//! Git for the open profile: pull, commit, push in the repository of the Hugo blog.
//! Uses the `git` command of the computer (its login / SSH keys); never asks interactively.

use crate::store::{self, Config};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

/// Runs git in `repo`. Ok/Err carry everything git printed (for the output pane).
pub fn run(repo: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "core.quotepath=false"])
        .args(args)
        .current_dir(repo)
        .env("GIT_TERMINAL_PROMPT", "0");
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        // fail instead of waiting for a password prompt nobody can see
        cmd.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    }
    let out = cmd.output().map_err(|e| format!("git konnte nicht gestartet werden: {e}"))?;
    let mut text = String::from_utf8_lossy(&out.stdout).trim_end().to_string();
    let err = String::from_utf8_lossy(&out.stderr).trim_end().to_string();
    if !err.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    if out.status.success() {
        Ok(text)
    } else {
        Err(if text.is_empty() { format!("git {} ist fehlgeschlagen", args.join(" ")) } else { text })
    }
}

/// Top folder of the git repository that contains `dir`.
pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }
    run(dir, &["rev-parse", "--show-toplevel"]).ok().map(|s| PathBuf::from(s.trim()))
}

/// The repository for a profile: the configured one, else the one holding the sync folder,
/// else the one holding the data folder.
pub fn repo_for(cfg: &Config) -> Result<PathBuf, String> {
    if !cfg.git_repo.as_os_str().is_empty() {
        return repo_root(&cfg.git_repo)
            .ok_or_else(|| format!("{} ist kein Git-Repository.", cfg.git_repo.display()));
    }
    if !cfg.sync_dir.as_os_str().is_empty() {
        if let Some(r) = repo_root(&cfg.sync_dir) {
            return Ok(r);
        }
    }
    repo_root(&cfg.data_dir).ok_or_else(|| {
        "Kein Git-Repository gefunden. Der Sync-Ordner (der content-Ordner deines Hugo-Blogs) sollte in einem Git-Repository liegen – oder in den Einstellungen ein Git-Repository eintragen.".into()
    })
}

/// Paths that hold blog content for this profile, relative to `repo`
/// (content folder, transactions, snippets.json, tags.json). Empty = whole repository.
pub fn scope(cfg: &Config, repo: &Path) -> Vec<String> {
    if !cfg.git_blog_only {
        return vec![];
    }
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let repo = canon(repo);
    let (content, meta) = if !cfg.sync_dir.as_os_str().is_empty() && canon(&cfg.sync_dir).starts_with(&repo) {
        (cfg.sync_dir.clone(), store::meta_of(&cfg.sync_dir))
    } else {
        (cfg.content_root(), cfg.meta_dir())
    };
    let mut out = Vec::new();
    for p in [
        content,
        meta.join(store::TX_DIR),
        store::snippets_path(&meta),
        meta.join("tags.json"),
    ] {
        let abs = canon(&p);
        if let Ok(rel) = abs.strip_prefix(&repo) {
            let rel = rel.to_string_lossy().to_string();
            if p.exists() && !out.contains(&rel) {
                out.push(if rel.is_empty() { ".".into() } else { rel });
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Created,
    Changed,
    Deleted,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Created => "Neu",
            Kind::Changed => "Geändert",
            Kind::Deleted => "Gelöscht",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Change {
    pub kind: Kind,
    /// article folder, relative to the repository
    pub dir: String,
    pub title: String,
    pub modified: Option<SystemTime>,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    /// `main...origin/main [ahead 1]`
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub changes: Vec<Change>,
    /// changed files that are not articles (transactions, snippets.json …)
    pub other: Vec<String>,
}

impl Status {
    pub fn is_clean(&self) -> bool {
        self.changes.is_empty() && self.other.is_empty()
    }
}

fn title_of(text: &str, dir: &str) -> String {
    let a = store::parse_article(text, dir, &Config::default());
    if a.title.trim().is_empty() {
        dir.rsplit('/').next().unwrap_or(dir).to_string()
    } else {
        a.title.trim().to_string()
    }
}

/// What changed since the last commit, grouped by article.
pub fn status(repo: &Path, scope: &[String]) -> Result<Status, String> {
    let mut st = Status::default();
    if let Ok(head) = run(repo, &["status", "-sb", "--porcelain=v1"]) {
        let first = head.lines().next().unwrap_or("").trim_start_matches("## ").to_string();
        let num = |key: &str| -> u32 {
            first
                .split(key)
                .nth(1)
                .and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next())
                .and_then(|n| n.parse().ok())
                .unwrap_or(0)
        };
        st.ahead = num("ahead ");
        st.behind = num("behind ");
        st.branch = first;
    }
    let mut args = vec!["status", "--porcelain=v1", "-z", "--untracked-files=all"];
    if !scope.is_empty() {
        args.push("--");
        args.extend(scope.iter().map(|s| s.as_str()));
    }
    let raw = run(repo, &args)?;
    // file -> (index of status char: '?', 'A', 'D' or 'M')
    let mut files: Vec<(String, char)> = Vec::new();
    let mut parts = raw.split('\0');
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let (x, y) = (entry.as_bytes()[0] as char, entry.as_bytes()[1] as char);
        let path = entry[3..].to_string();
        if x == 'R' || x == 'C' {
            parts.next(); // source path of a rename
        }
        let kind = if x == '?' || x == 'A' {
            'A'
        } else if x == 'D' || y == 'D' {
            'D'
        } else {
            'M'
        };
        files.push((path, kind));
    }

    let in_head = |p: &str| run(repo, &["cat-file", "-e", &format!("HEAD:{p}")]).is_ok();
    let mut dirs: BTreeMap<String, Kind> = BTreeMap::new();
    for (path, k) in &files {
        let (dir, name) = match path.rfind('/') {
            Some(i) => (path[..i].to_string(), &path[i + 1..]),
            None => (String::new(), path.as_str()),
        };
        if name == "index.md" {
            let kind = match k {
                'A' => Kind::Created,
                'D' => Kind::Deleted,
                _ => Kind::Changed,
            };
            dirs.insert(dir, kind);
        } else if !dir.is_empty()
            && (repo.join(&dir).join("index.md").is_file() || in_head(&format!("{dir}/index.md")))
        {
            dirs.entry(dir).or_insert(Kind::Changed);
        } else {
            st.other.push(path.clone());
        }
    }
    for (dir, kind) in dirs {
        let file = repo.join(&dir).join("index.md");
        let (title, modified) = if kind == Kind::Deleted {
            let old = run(repo, &["show", &format!("HEAD:{dir}/index.md")]).unwrap_or_default();
            (title_of(&old, &dir), None)
        } else {
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            (title_of(&text, &dir), std::fs::metadata(&file).and_then(|m| m.modified()).ok())
        };
        st.changes.push(Change { kind, dir, title, modified });
    }
    // newest first
    st.changes.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(st)
}

/// Commit message: the title of the last changed entry, then what was created, changed, deleted.
pub fn commit_message(st: &Status) -> String {
    let last = st
        .changes
        .iter()
        .filter(|c| c.kind != Kind::Deleted)
        .max_by_key(|c| c.modified)
        .or_else(|| st.changes.first());
    let mut subject = match last {
        Some(c) => c.title.clone(),
        None if !st.other.is_empty() => "hmb: Transaktionen und Einstellungen".to_string(),
        None => String::new(),
    };
    if st.changes.len() > 1 {
        subject.push_str(&format!(" (+{} weitere)", st.changes.len() - 1));
    }
    let mut body = String::new();
    for kind in [Kind::Created, Kind::Changed, Kind::Deleted] {
        let list: Vec<&Change> = st.changes.iter().filter(|c| c.kind == kind).collect();
        if list.is_empty() {
            continue;
        }
        body.push_str(&format!("\n{}:\n", kind.label()));
        for c in list {
            body.push_str(&format!("- {} ({})\n", c.title, c.dir));
        }
    }
    if !st.other.is_empty() {
        body.push_str(&format!("\nSonstiges: {} Datei(en) (Transaktionen, Snippets, Tags)\n", st.other.len()));
    }
    if body.is_empty() {
        subject
    } else {
        format!("{subject}\n{body}").trim_end().to_string()
    }
}

/// Pull with rebase; local uncommitted changes are put aside and restored.
pub fn pull(repo: &Path) -> Result<String, String> {
    run(repo, &["pull", "--rebase", "--autostash"])
}

/// Stages the blog content (or everything) and commits with `message`.
pub fn commit(repo: &Path, scope: &[String], message: &str) -> Result<String, String> {
    let mut add = vec!["add", "-A"];
    if !scope.is_empty() {
        add.push("--");
        add.extend(scope.iter().map(|s| s.as_str()));
    }
    let added = run(repo, &add)?;
    let out = run(repo, &["commit", "-m", message])?;
    Ok(if added.is_empty() { out } else { format!("{added}\n{out}") })
}

/// Push; sets the upstream on the first push of a branch.
pub fn push(repo: &Path) -> Result<String, String> {
    if run(repo, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).is_ok() {
        run(repo, &["push"])
    } else {
        run(repo, &["push", "-u", "origin", "HEAD"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn status_message_commit_push_with_local_remote() {
        let t = std::env::temp_dir().join(format!("hmb-git-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t);
        let (remote, repo) = (t.join("remote.git"), t.join("blog"));
        fs::create_dir_all(&t).unwrap();
        run(&t, &["init", "-q", "--bare", remote.to_str().unwrap()]).unwrap();
        run(&t, &["init", "-q", repo.to_str().unwrap()]).unwrap();
        for (k, v) in [("user.name", "Test"), ("user.email", "t@example.org")] {
            run(&repo, &["config", k, v]).unwrap();
        }
        run(&repo, &["remote", "add", "origin", remote.to_str().unwrap()]).unwrap();
        let w = |p: &str, s: &str| {
            let f = repo.join(p);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, s).unwrap();
        };
        w("content/a/alt/index.md", "---\ntitle: Alter Artikel\n---\nx\n");
        w("content/a/weg/index.md", "---\ntitle: Wird gelöscht\n---\nx\n");
        w("public/index.html", "build output");
        run(&repo, &["add", "-A"]).unwrap();
        run(&repo, &["commit", "-q", "-m", "start"]).unwrap();
        // changes: one new (with image), one changed, one deleted, one transaction, one non-blog file
        w("content/b/neu/index.md", "---\ntitle: Ganz neu\n---\nx\n");
        w("content/b/neu/bild.png", "png");
        std::thread::sleep(std::time::Duration::from_millis(20));
        w("content/a/alt/index.md", "---\ntitle: Alter Artikel\n---\ny\n");
        fs::remove_dir_all(repo.join("content/a/weg")).unwrap();
        w("transactions/transaction-2026-11-27-10-00.json", "{}");
        w("public/index.html", "changed build output");

        let cfg = Config { sync_dir: repo.join("content"), git_blog_only: true, ..Default::default() };
        let sc = scope(&cfg, &repo);
        assert_eq!(sc, vec!["content".to_string(), "transactions".to_string()]);
        let st = status(&repo, &sc).unwrap();
        let kinds: Vec<(Kind, &str)> = st.changes.iter().map(|c| (c.kind, c.title.as_str())).collect();
        println!("{kinds:?} other={:?}", st.other);
        assert!(kinds.contains(&(Kind::Created, "Ganz neu")));
        assert!(kinds.contains(&(Kind::Changed, "Alter Artikel")));
        assert!(kinds.contains(&(Kind::Deleted, "Wird gelöscht")));
        assert_eq!(st.other, vec!["transactions/transaction-2026-11-27-10-00.json".to_string()]);
        let msg = commit_message(&st);
        println!("{msg}");
        assert!(msg.starts_with("Alter Artikel (+2 weitere)"), "last changed entry first: {msg}");
        assert!(msg.contains("Neu:\n- Ganz neu (content/b/neu)"));
        assert!(msg.contains("Gelöscht:\n- Wird gelöscht (content/a/weg)"));

        commit(&repo, &sc, &msg).unwrap();
        let after = status(&repo, &sc).unwrap();
        assert!(after.is_clean());
        // the non-blog file stays uncommitted
        assert!(run(&repo, &["status", "--porcelain"]).unwrap().contains("public/index.html"));
        let pushed = push(&repo).unwrap();
        println!("{pushed}");
        assert!(run(&remote, &["log", "--oneline", "-1"]).unwrap().contains("Alter Artikel"));
        assert!(pull(&repo).is_ok());
        let _ = fs::remove_dir_all(&t);
    }
}
