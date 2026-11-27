//! Sync between the app's content folder and a second local folder (e.g. the content folder of
//! the Hugo blog, which the person keeps in Git themselves). No network, no Git here.
//!
//! Both sides are treated as *content folders*: an article `aws/mock/overcast` lives in
//! `<app content>/aws/mock/overcast/` and `<sync folder>/aws/mock/overcast/`. Transaction files
//! are kept in `<sync folder>/transactions/transaction-YYYY-MM-dd-HH-mm.json`.
//!
//! Normal sync:
//!   1. read the transaction files starting at the last processed stamp (several files can share
//!      one stamp) and copy the folders listed there into the app (new ones are imported,
//!      existing ones overwritten);
//!   2. copy the local changes (pending list) into the sync folder and describe them in a new
//!      transaction file;
//!   3. remember the last processed transaction file.
//!
//! Full sync: compares every article folder on both sides, copies what is missing on either side,
//! newer wins where both differ, writes a transaction file for everything written to the sync
//! folder and marks all existing transaction files as processed.

use crate::store::{self, Config, Ontology, Snippet, TxEntry};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub struct SyncResult {
    pub log: Vec<String>,
    /// changes that came from the sync folder (shown as "what's new")
    pub imported: Vec<TxEntry>,
    pub last_tx: String,
    pub last_tx_files: Vec<String>,
}

fn canon(p: &Path) -> PathBuf {
    fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Is something wrong with the two folders? (same folder, one inside the other …)
pub fn folder_problem(cfg: &Config) -> Option<String> {
    let sync = &cfg.sync_dir;
    if sync.as_os_str().is_empty() {
        return Some("Bitte zuerst in den Einstellungen einen Sync-Ordner wählen.".into());
    }
    if !sync.is_dir() {
        return Some(format!("Sync-Ordner nicht gefunden: {}", sync.display()));
    }
    let local = canon(&cfg.content_root());
    let remote = canon(sync);
    if local == remote {
        return Some(
            "Der Sync-Ordner ist derselbe Ordner wie der Content-Ordner der App – die App arbeitet dann schon direkt im Blog. Wähle als Datenordner einen eigenen Ordner (z. B. Dokumente/hmb)."
                .into(),
        );
    }
    if local.starts_with(&remote) || remote.starts_with(&local) {
        return Some(
            "Content-Ordner der App und Sync-Ordner liegen ineinander. Wähle zwei getrennte Ordner, z. B. Dokumente/hmb als Datenordner und den content-Ordner des Blogs als Sync-Ordner."
                .into(),
        );
    }
    let data = canon(&cfg.data_dir);
    if remote.starts_with(&data) {
        return Some("Der Sync-Ordner liegt im Datenordner der App. Wähle zwei getrennte Ordner.".into());
    }
    None
}

fn check(cfg: &Config) -> Result<(PathBuf, PathBuf), String> {
    if let Some(p) = folder_problem(cfg) {
        return Err(p);
    }
    let local = cfg.content_root();
    fs::create_dir_all(&local).map_err(|e| e.to_string())?;
    Ok((local, cfg.sync_dir.clone()))
}

/// Replaces `dst` with a copy of `src` (all files of the article folder, incl. images).
fn mirror(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        fs::remove_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    }
    copy_dir(src, dst)
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    for e in fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))?.flatten() {
        let name = e.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &dst.join(&name))?;
        } else {
            fs::copy(&p, dst.join(&name)).map_err(|e| format!("{}: {e}", p.display()))?;
        }
    }
    Ok(())
}

fn remove_dir(root: &Path, rel: &str) -> Result<bool, String> {
    if rel.trim_matches('/').is_empty() {
        return Ok(false);
    }
    let d = root.join(rel);
    if d.is_dir() {
        fs::remove_dir_all(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        prune_empty(&d, root);
        return Ok(true);
    }
    Ok(false)
}

/// Removes folders above `start` that became empty, up to (not including) `stop`.
fn prune_empty(start: &Path, stop: &Path) {
    let mut p = start.parent().map(|p| p.to_path_buf());
    while let Some(d) = p {
        if d == stop || !d.starts_with(stop) {
            break;
        }
        let _ = fs::remove_file(d.join(".DS_Store"));
        if fs::remove_dir(&d).is_err() {
            break;
        }
        p = d.parent().map(|p| p.to_path_buf());
    }
}

/// Files of a folder (recursively, without hidden files), with contents.
fn dir_files(d: &Path) -> BTreeMap<String, Vec<u8>> {
    fn go(d: &Path, prefix: &str, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(rd) = fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with('.') {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                go(&p, &format!("{prefix}{n}/"), out);
            } else {
                out.insert(format!("{prefix}{n}"), fs::read(&p).unwrap_or_default());
            }
        }
    }
    let mut out = BTreeMap::new();
    go(d, "", &mut out);
    out
}

/// Same files with the same contents?
fn same_dir(a: &Path, b: &Path) -> bool {
    dir_files(a) == dir_files(b)
}

fn modified(dir: &Path) -> std::time::SystemTime {
    fs::metadata(dir.join("index.md"))
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH)
}

fn entry_for(root: &Path, rel: &str, action: &str, cfg: &Config, client: &str) -> TxEntry {
    let (id, title) = fs::read_to_string(root.join(rel).join("index.md"))
        .map(|t| {
            let a = store::parse_article(&t, &format!("{}{rel}", cfg.root_prefix()), cfg);
            (a.id, a.title)
        })
        .unwrap_or_else(|_| (rel.replace('/', "-"), String::new()));
    TxEntry {
        time: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        id,
        action: action.into(),
        path: rel.into(),
        title,
        client: client.into(),
    }
}

/// Tag ontology and snippets are merged in both directions (local wins on differences).
fn merge_extras(cfg: &Config) {
    let (data, sync) = (&cfg.meta_dir(), &store::meta_of(&cfg.sync_dir));
    if canon(data) == canon(sync) {
        return;
    }
    let mut local = Ontology::load(data);
    let remote = Ontology::load(sync);
    for (c, p) in remote.parents {
        local.parents.entry(c).or_insert(p);
    }
    if !local.parents.is_empty() {
        let _ = local.save(data);
        let _ = local.save(sync);
    }
    let mut all: Vec<Snippet> = store::load_snippets(data);
    let seen: BTreeSet<(String, String)> = all.iter().map(|s| (s.created.clone(), s.text.clone())).collect();
    for s in store::load_snippets(sync) {
        if !seen.contains(&(s.created.clone(), s.text.clone())) {
            all.push(s);
        }
    }
    if !all.is_empty() {
        all.sort_by(|a, b| b.created.cmp(&a.created));
        let _ = store::save_snippets(data, &all);
        let _ = store::save_snippets(sync, &all);
    }
}

/// Local pending changes (paths relative to the content folder), with safety rules:
/// changes to nested copies (`content/…`) are never passed on, and a deletion is ignored
/// when the article exists here again.
fn pending(cfg: &Config) -> Vec<TxEntry> {
    let names = nest_names(cfg);
    let local = cfg.content_root();
    store::load_pending(&cfg.meta_dir())
        .into_iter()
        .map(|mut e| {
            e.path = e.dir();
            e
        })
        .filter(|e| !e.path.is_empty() && strip_nest(&e.path, &names).is_none())
        .filter(|e| !(e.action == "deleted" && local.join(&e.path).join("index.md").exists()))
        .collect()
}

fn mark(stamp: &str, name: &str, last_tx: &mut String, last_files: &mut Vec<String>) {
    if stamp > last_tx.as_str() {
        *last_tx = stamp.to_string();
        last_files.clear();
    }
    if stamp == last_tx.as_str() && !last_files.iter().any(|f| f == name) {
        last_files.push(name.to_string());
    }
}

pub fn sync(cfg: &Config) -> Result<SyncResult, String> {
    let (local, remote) = check(cfg)?;
    let mut log = migrate_layout(cfg);
    let pending = pending(cfg);
    let pending_dirs: BTreeSet<String> = pending.iter().map(|e| e.path.clone()).collect();

    // 1) read new transaction files of the sync folder
    let rmeta = store::meta_of(&remote);
    let files = store::list_tx_files(&rmeta);
    let todo: Vec<&(String, String)> = files
        .iter()
        .filter(|(stamp, name)| {
            stamp.as_str() > cfg.last_tx.as_str()
                || (*stamp == cfg.last_tx && !cfg.last_tx_files.contains(name))
        })
        .collect();
    // newest action per folder wins
    let mut changes: BTreeMap<String, TxEntry> = BTreeMap::new();
    for (_, name) in &todo {
        if let Some(f) = store::read_tx_file(&rmeta, name) {
            for mut e in f.entries {
                if e.client.is_empty() {
                    e.client = f.client.clone();
                }
                let dir = e.dir();
                if !dir.is_empty() && strip_nest(&dir, &nest_names(cfg)).is_none() {
                    changes.insert(dir, e);
                }
            }
        }
    }
    let mut imported = Vec::new();
    let (mut n_new, mut n_upd, mut n_del, mut n_conf) = (0, 0, 0, 0);
    for (dir, e) in &changes {
        if pending_dirs.contains(dir) {
            n_conf += 1; // changed here too: the local version wins and is written below
            continue;
        }
        if e.action == "deleted" {
            if remove_dir(&local, dir)? {
                n_del += 1;
                imported.push(e.clone());
            }
            continue;
        }
        let src = remote.join(dir);
        if !src.join("index.md").exists() {
            continue;
        }
        let dst = local.join(dir);
        let existed = dst.exists();
        if existed && same_dir(&src, &dst) {
            continue;
        }
        mirror(&src, &dst)?;
        if existed {
            n_upd += 1;
        } else {
            n_new += 1;
        }
        imported.push(e.clone());
    }
    log.push(format!(
        "{} Transaktionsdatei(en) gelesen: {n_new} neu, {n_upd} aktualisiert, {n_del} gelöscht.",
        todo.len()
    ));
    if n_conf > 0 {
        log.push(format!("{n_conf} Eintrag/Einträge auch hier geändert – die lokale Fassung gewinnt."));
    }

    // 2) write local changes into the sync folder
    let mut written = Vec::new();
    for e in &pending {
        if e.action == "deleted" {
            remove_dir(&remote, &e.path)?;
            written.push(e.clone());
        } else if local.join(&e.path).join("index.md").exists() {
            mirror(&local.join(&e.path), &remote.join(&e.path))?;
            written.push(e.clone());
        }
    }
    let mut last_tx = cfg.last_tx.clone();
    let mut last_files = cfg.last_tx_files.clone();
    for (stamp, name) in &todo {
        mark(stamp, name, &mut last_tx, &mut last_files);
    }
    if !written.is_empty() {
        let name = store::write_tx_file(&rmeta, &cfg.client, &written)?;
        if let Some(stamp) = store::tx_stamp(&name) {
            mark(&stamp, &name, &mut last_tx, &mut last_files);
        }
        log.push(format!("{} lokale Änderung(en) geschrieben ({name}).", written.len()));
    } else {
        log.push("Keine lokalen Änderungen.".into());
    }
    store::save_pending(&cfg.meta_dir(), &[])?;
    merge_extras(cfg);

    Ok(SyncResult { log, imported, last_tx, last_tx_files: last_files })
}

pub fn full_sync(cfg: &Config) -> Result<SyncResult, String> {
    let (local, remote) = check(cfg)?;
    let moved = migrate_layout(cfg);
    let pending = pending(cfg);
    let pending_dirs: BTreeSet<String> = pending.iter().map(|e| e.path.clone()).collect();

    // deletions made here go out first
    let mut written = Vec::new();
    for e in pending.iter().filter(|e| e.action == "deleted") {
        remove_dir(&remote, &e.path)?;
        written.push(e.clone());
    }

    let here = store::article_dirs(&local);
    let there = store::article_dirs(&remote);
    let mut imported = Vec::new();
    let (mut n_in, mut n_out) = (0, 0);
    for dir in here.union(&there) {
        let (l, r) = (local.join(dir), remote.join(dir));
        match (here.contains(dir), there.contains(dir)) {
            (false, true) => {
                mirror(&r, &l)?;
                imported.push(entry_for(&remote, dir, "created", cfg, "Sync-Ordner"));
                n_in += 1;
            }
            (true, false) => {
                mirror(&l, &r)?;
                written.push(entry_for(&local, dir, "created", cfg, &cfg.client));
                n_out += 1;
            }
            _ => {
                if same_dir(&l, &r) {
                    continue;
                }
                if pending_dirs.contains(dir) || modified(&l) >= modified(&r) {
                    mirror(&l, &r)?;
                    written.push(entry_for(&local, dir, "updated", cfg, &cfg.client));
                    n_out += 1;
                } else {
                    mirror(&r, &l)?;
                    imported.push(entry_for(&remote, dir, "updated", cfg, "Sync-Ordner"));
                    n_in += 1;
                }
            }
        }
    }

    let mut log = moved;
    log.push(format!(
        "Vollsynchronisierung: {} Artikel in der App, {} im Sync-Ordner – {n_in} geholt, {n_out} geschrieben.",
        here.len(),
        there.len()
    ));
    let rmeta = store::meta_of(&remote);
    let mut files = store::list_tx_files(&rmeta);
    if !written.is_empty() {
        let name = store::write_tx_file(&rmeta, &cfg.client, &written)?;
        log.push(format!("Transaktionsdatei geschrieben: {name}"));
        files = store::list_tx_files(&rmeta);
    }
    // everything that exists so far counts as processed
    let last_tx = files.last().map(|(s, _)| s.clone()).unwrap_or_default();
    let last_tx_files = files.iter().filter(|(s, _)| *s == last_tx).map(|(_, n)| n.clone()).collect();
    store::save_pending(&cfg.meta_dir(), &[])?;
    merge_extras(cfg);

    Ok(SyncResult { log, imported, last_tx, last_tx_files })
}

// ---------------------------------------------------------------- tidying up older layouts

fn is_old_day_log(name: &str) -> bool {
    // `2026-11-22_mac.json` (one file per day in year/month folders, first version)
    let b = name.as_bytes();
    name.ends_with(".json")
        && b.len() > 16
        && b[4] == b'-'
        && b[7] == b'-'
        && b[10] == b'_'
        && name[..4].chars().all(|c| c.is_ascii_digit())
}

fn merge_pending_from(file: &Path, meta: &Path) {
    let extra: Vec<TxEntry> = fs::read_to_string(file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    if !extra.is_empty() {
        let mut p = store::load_pending(meta);
        for e in extra {
            if !p.iter().any(|x| x.dir() == e.dir()) {
                p.push(e);
            }
        }
        let _ = store::save_pending(meta, &p);
    }
    let _ = fs::remove_file(file);
}

/// Moves the transaction files of `old` (flat, from any sub folder) into `dest`, removes old
/// day logs and folders that became empty. Returns the number of moved files.
fn move_tx_folder(old: &Path, dest: &Path, local_meta: Option<&Path>) -> usize {
    let mut moved = 0;
    let mut files = Vec::new();
    fn collect(d: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    collect(&p, out);
                } else {
                    out.push(p);
                }
            }
        }
    }
    collect(old, &mut files);
    for f in files {
        let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if store::tx_stamp(&name).is_some() {
            let _ = fs::create_dir_all(dest);
            let mut target = dest.join(&name);
            if target.exists() {
                if fs::read(&target).ok() == fs::read(&f).ok() {
                    let _ = fs::remove_file(&f);
                    continue;
                }
                target = dest.join(name.replace(".json", "-alt.json"));
            }
            if fs::rename(&f, &target).is_ok() || (fs::copy(&f, &target).is_ok() && fs::remove_file(&f).is_ok()) {
                moved += 1;
            }
        } else if name == "pending.json" {
            match local_meta {
                Some(m) => merge_pending_from(&f, m),
                None => {
                    let _ = fs::remove_file(&f);
                }
            }
        } else if is_old_day_log(&name) || name == ".DS_Store" {
            let _ = fs::remove_file(&f);
        }
    }
    remove_empty_dirs(old);
    moved
}

/// Removes `d` and its sub folders if they hold no files (bottom-up).
fn remove_empty_dirs(d: &Path) {
    if let Ok(rd) = fs::read_dir(d) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                remove_empty_dirs(&e.path());
            }
        }
    }
    let _ = fs::remove_file(d.join(".DS_Store"));
    let _ = fs::remove_dir(d);
}

/// Older versions kept `transactions/` (with year/month/day folders), `snippets.json` and
/// `tags.json` inside the content folder, where Hugo could mistake them for content.
/// This moves them next to the content folder; transaction files end up flat in `transactions/`.
pub fn migrate_layout(cfg: &Config) -> Vec<String> {
    let mut log = Vec::new();
    let local_meta = cfg.meta_dir();
    let mut sides: Vec<(PathBuf, PathBuf, bool)> = vec![(cfg.content_root(), local_meta.clone(), true)];
    if !cfg.sync_dir.as_os_str().is_empty()
        && cfg.sync_dir.is_dir()
        && canon(&cfg.sync_dir) != canon(&cfg.content_root())
    {
        sides.push((cfg.sync_dir.clone(), store::meta_of(&cfg.sync_dir), false));
    }
    // folders between a content folder and its meta level (e.g. site/content for content/post)
    // held these files in earlier versions too
    let mut expanded: Vec<(PathBuf, PathBuf, bool)> = Vec::new();
    for (content, meta, is_local) in &sides {
        expanded.push((content.clone(), meta.clone(), *is_local));
        let mut cur = content.parent().map(|p| p.to_path_buf());
        while let Some(d) = cur {
            if canon(&d) == canon(meta) || !canon(&d).starts_with(canon(meta)) {
                break;
            }
            expanded.push((d.clone(), meta.clone(), *is_local));
            cur = d.parent().map(|p| p.to_path_buf());
        }
    }
    for (content, meta, is_local) in &expanded {
        if !content.is_dir() || canon(content) == canon(meta) {
            continue;
        }
        // transactions/ inside the content folder (only if it is not a real Hugo section)
        let old = content.join(store::TX_DIR);
        if old.is_dir() && !old.join("index.md").exists() && !old.join("_index.md").exists() {
            let n = move_tx_folder(&old, &meta.join(store::TX_DIR), is_local.then_some(local_meta.as_path()));
            log.push(format!(
                "Transaktionen aus {} nach {} verschoben ({n} Datei(en)).",
                old.display(),
                meta.join(store::TX_DIR).display()
            ));
        }
        // snippets.json / tags.json inside the content folder
        let old_snips = store::snippets_path(content);
        if old_snips.is_file() {
            let mut all = store::load_snippets(meta);
            let seen: BTreeSet<(String, String)> = all.iter().map(|s| (s.created.clone(), s.text.clone())).collect();
            for s in store::load_snippets(content) {
                if !seen.contains(&(s.created.clone(), s.text.clone())) {
                    all.push(s);
                }
            }
            all.sort_by(|a, b| b.created.cmp(&a.created));
            if store::save_snippets(meta, &all).is_ok() {
                let _ = fs::remove_file(&old_snips);
                log.push(format!("snippets.json nach {} verschoben.", meta.display()));
            }
        }
        if content.join("tags.json").is_file() {
            let mut onto = Ontology::load(meta);
            for (c, p) in Ontology::load(content).parents {
                onto.parents.entry(c).or_insert(p);
            }
            if onto.save(meta).is_ok() {
                let _ = fs::remove_file(content.join("tags.json"));
                log.push(format!("tags.json nach {} verschoben.", meta.display()));
            }
        }
    }
    // the transactions folder next to content holds only flat transaction files:
    // pull up files from year/month folders, move the old local pending list, drop old day logs
    let mut metas = vec![local_meta.clone()];
    if sides.len() > 1 {
        metas.push(sides[1].1.clone());
    }
    for meta in metas {
        let tx = meta.join(store::TX_DIR);
        let Ok(rd) = fs::read_dir(&tx) else { continue };
        let mut tidy = false;
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if e.path().is_dir() {
                let n = move_tx_folder(&e.path(), &tx, Some(local_meta.as_path()));
                if n > 0 {
                    log.push(format!("{n} Transaktionsdatei(en) aus Unterordnern nach {} geholt.", tx.display()));
                }
                tidy = true;
            } else if name == "pending.json" && meta == local_meta {
                merge_pending_from(&e.path(), &local_meta);
                tidy = true;
            } else if is_old_day_log(&name) {
                let _ = fs::remove_file(e.path());
                tidy = true;
            }
        }
        if tidy && log.is_empty() {
            log.push(format!("Alte Unterordner in {} aufgeräumt.", tx.display()));
        }
    }
    log
}

// ---------------------------------------------------------------- repairing nested copies

/// Roots to check: the app's content folder and (if different) the sync folder.
fn repair_roots(cfg: &Config) -> Vec<PathBuf> {
    let mut roots = vec![cfg.content_root()];
    if !cfg.sync_dir.as_os_str().is_empty() && cfg.sync_dir.is_dir() && canon(&cfg.sync_dir) != canon(&roots[0]) {
        roots.push(cfg.sync_dir.clone());
    }
    roots
}

/// Folder names that mark a nested copy (`content/…` inside the content folder).
fn nest_names(cfg: &Config) -> BTreeSet<String> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    names.insert("content".into());
    if let Some(last) = cfg.content_subdir.trim_matches('/').rsplit('/').next() {
        if !last.is_empty() {
            names.insert(last.to_string());
        }
    }
    names
}

fn strip_nest<'a>(d: &'a str, names: &BTreeSet<String>) -> Option<&'a str> {
    names.iter().find_map(|n| d.strip_prefix(&format!("{n}/")))
}

/// Article folders that sit one level too deep, e.g. `content/aws/x` inside the content folder
/// (left behind by an earlier version that synced a folder into itself).
pub fn nested_copies(cfg: &Config) -> Vec<PathBuf> {
    let names = nest_names(cfg);
    let mut out = Vec::new();
    for root in repair_roots(cfg) {
        for d in store::article_dirs(&root) {
            if strip_nest(&d, &names).is_some() {
                out.push(root.join(d));
            }
        }
    }
    out.sort();
    out
}

pub struct Repair {
    pub removed: usize,
    pub restored: Vec<String>,
    pub conflicts: Vec<String>,
}

/// Repairs nested copies: identical copies are removed, copies whose original is missing are
/// moved back to the original place, differing ones are left alone and reported.
/// Noted local changes that only concern these copies are dropped.
pub fn repair_nested(cfg: &Config) -> Result<Repair, String> {
    let names = nest_names(cfg);
    let mut rep = Repair { removed: 0, restored: Vec::new(), conflicts: Vec::new() };
    for root in repair_roots(cfg) {
        for _round in 0..8 {
            let dirs = store::article_dirs(&root);
            let mut cands: Vec<&String> = dirs.iter().filter(|d| strip_nest(d, &names).is_some()).collect();
            // shallow copies first, so deeper copies can take their place afterwards
            cands.sort_by_key(|d| d.matches('/').count());
            let mut changed = false;
            for d in cands {
                let rest = strip_nest(d, &names).unwrap_or(d);
                let (src, dst) = (root.join(d), root.join(rest));
                if !src.is_dir() {
                    continue;
                }
                if dst.join("index.md").exists() {
                    if same_dir(&src, &dst) {
                        fs::remove_dir_all(&src).map_err(|e| format!("{}: {e}", src.display()))?;
                        prune_empty(&src, &root);
                        rep.removed += 1;
                        changed = true;
                    } else if !rep.conflicts.contains(d) {
                        rep.conflicts.push(d.clone());
                    }
                } else {
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    if dst.exists() {
                        fs::remove_dir_all(&dst).map_err(|e| e.to_string())?;
                    }
                    fs::rename(&src, &dst).map_err(|e| format!("{}: {e}", src.display()))?;
                    prune_empty(&src, &root);
                    if !rep.restored.iter().any(|r| r == rest) {
                        rep.restored.push(rest.to_string());
                    }
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
    // intermediate moves (copy of a copy) are not worth reporting
    rep.restored.retain(|r| strip_nest(r, &names).is_none());
    // forget noted changes that were only about the copies
    let pending: Vec<_> = store::load_pending(&cfg.meta_dir())
        .into_iter()
        .filter(|e| {
            let p = e.dir();
            let about_copy = strip_nest(&p, &names).is_some();
            let restored_delete = e.action == "deleted" && rep.restored.iter().any(|r| *r == p);
            !about_copy && !restored_delete
        })
        .collect();
    store::save_pending(&cfg.meta_dir(), &pending)?;
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `HMB_FIXTURE=<copy of the blog's data folder> cargo test` – never point it at the real blog.
    #[test]
    fn cleanup_and_full_sync_on_copy() {
        let Ok(dir) = std::env::var("HMB_FIXTURE") else { return };
        let base = PathBuf::from(&dir);
        let broken = Config {
            data_dir: base.clone(),
            content_subdir: "content".into(),
            layout: store::Layout::Sections,
            sync_dir: base.join("content"),
            ..Default::default()
        };
        assert!(folder_problem(&broken).is_some(), "same folder must be refused");
        assert!(sync(&broken).is_err());
        let before = store::article_dirs(&base.join("content")).len();
        let copies = nested_copies(&broken);
        println!("{before} Artikelordner, davon {} zu tief", copies.len());
        let rep = repair_nested(&broken).unwrap();
        let after = store::article_dirs(&base.join("content"));
        println!("{} entfernt, {} zurückgelegt {:?}, Konflikte {:?}, übrig: {}", rep.removed, rep.restored.len(), rep.restored, rep.conflicts, after.len());
        for d in &after { println!("  {d}"); }
        println!("vorgemerkt: {:?}", store::load_pending(&base).iter().map(|e| format!("{} {}", e.action, e.path)).collect::<Vec<_>>());
        assert!(after.iter().all(|d| !d.starts_with("content/")));
        assert!(nested_copies(&broken).is_empty());

        // proper setup: own data folder for the app, blog content as sync folder
        let own = base.join("_hmb_own");
        let good = Config { data_dir: own.clone(), sync_dir: base.join("content"), client: "testmac".into(), ..broken.clone() };
        assert!(folder_problem(&good).is_none(), "{:?}", folder_problem(&good));
        let r = full_sync(&good).unwrap();
        println!("{}", r.log.join("\n"));
        let in_app = store::article_dirs(&own.join("content"));
        assert_eq!(in_app, after, "all blog articles must be in the app");
        let arts = store::load_articles(&good);
        assert_eq!(arts.len(), after.len());

        // a new local article goes into the blog with a transaction file
        let mut a = store::new_article(&good, "Neu aus der App", "shorts");
        a.body = "Hallo".into();
        store::save_article(&good, &a).unwrap();
        store::log_tx(&good, &a, "created").unwrap();
        let good2 = Config { last_tx: r.last_tx.clone(), last_tx_files: r.last_tx_files.clone(), ..good.clone() };
        let r2 = sync(&good2).unwrap();
        println!("{}", r2.log.join("\n"));
        assert!(base.join("content/shorts/neu-aus-der-app/index.md").exists());
        let tx = store::read_tx_file(&base, r2.last_tx_files.last().unwrap()).unwrap();
        assert_eq!(tx.entries[0].path, "shorts/neu-aus-der-app");
    }
}

#[cfg(test)]
mod safety_tests {
    use super::*;

    #[test]
    fn pending_never_deletes_originals() {
        let t = std::env::temp_dir().join(format!("hmb-safety-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t);
        let (data, blog) = (t.join("app"), t.join("blog"));
        for base in [data.join("content"), blog.clone()] {
            fs::create_dir_all(base.join("build/gplan")).unwrap();
            fs::write(base.join("build/gplan/index.md"), "---\ntitle: GPlan\n---\nText\n").unwrap();
        }
        let cfg = Config {
            data_dir: data.clone(),
            content_subdir: "content".into(),
            layout: store::Layout::Sections,
            sync_dir: blog.clone(),
            client: "t".into(),
            ..Default::default()
        };
        // a deletion that was about the nested copy, and one for an article that exists here
        let e = |p: &str| TxEntry {
            time: "2026-11-23T00:00:00+01:00".into(),
            id: "x".into(),
            action: "deleted".into(),
            path: p.into(),
            title: String::new(),
            client: "t".into(),
        };
        store::save_pending(&data, &[e("content/build/gplan"), e("build/gplan")]).unwrap();
        let r = sync(&cfg).unwrap();
        println!("{}", r.log.join("\n"));
        assert!(blog.join("build/gplan/index.md").exists(), "original in the blog must survive");
        assert!(data.join("content/build/gplan/index.md").exists());
        let _ = fs::remove_dir_all(&t);
    }
}

#[cfg(test)]
mod repair_tests {
    use super::*;

    #[test]
    fn repair_removes_copies_and_restores_missing_originals() {
        let t = std::env::temp_dir().join(format!("hmb-repair-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t);
        let c = t.join("content");
        let put = |rel: &str, txt: &str| {
            fs::create_dir_all(c.join(rel)).unwrap();
            fs::write(c.join(rel).join("index.md"), txt).unwrap();
        };
        put("aws/x", "A");
        put("content/aws/x", "A"); // identical copy
        put("content/content/aws/x", "A"); // copy of the copy
        put("content/genai/y", "Y"); // original deleted by mistake
        put("build/z", "Z1");
        put("content/build/z", "Z2"); // differs: keep, report
        let cfg = Config { data_dir: t.clone(), content_subdir: "content".into(), ..Default::default() };
        assert_eq!(nested_copies(&cfg).len(), 4);
        let r = repair_nested(&cfg).unwrap();
        let dirs = store::article_dirs(&c);
        println!("{} entfernt, zurück {:?}, Konflikte {:?}, übrig {:?}", r.removed, r.restored, r.conflicts, dirs);
        assert!(dirs.contains("aws/x") && dirs.contains("genai/y") && dirs.contains("build/z"));
        assert!(!dirs.contains("content/aws/x") && !dirs.contains("content/genai/y"));
        assert_eq!(r.conflicts, vec!["content/build/z".to_string()]);
        let _ = fs::remove_dir_all(&t);
    }
}

#[cfg(test)]
mod migrate_tests {
    use super::*;

    /// `HMB_MIGRATE=<copy of the blog's data folder> cargo test migrate -- --nocapture`
    #[test]
    fn migrate_moves_everything_next_to_content() {
        let Ok(dir) = std::env::var("HMB_MIGRATE") else { return };
        let base = PathBuf::from(dir);
        let cfg = Config {
            data_dir: base.clone(),
            content_subdir: "content".into(),
            layout: store::Layout::Sections,
            sync_dir: base.join("content"),
            ..Default::default()
        };
        let before = store::article_dirs(&base.join("content"));
        for l in migrate_layout(&cfg) {
            println!("{l}");
        }
        assert!(!base.join("content/transactions").exists());
        assert!(!base.join("content/snippets.json").exists());
        assert!(!base.join("content/tags.json").exists());
        let tx = base.join("transactions");
        for e in fs::read_dir(&tx).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            println!("  transactions/{n}");
            assert!(e.path().is_file() && store::tx_stamp(&n).is_some(), "only flat transaction files: {n}");
        }
        assert_eq!(store::article_dirs(&base.join("content")), before, "articles untouched");
        println!("vorgemerkt: {}", store::load_pending(&base).len());
        assert!(migrate_layout(&cfg).is_empty(), "second run has nothing to do");
    }
}

#[cfg(test)]
mod migrate_synthetic_tests {
    use super::*;

    #[test]
    fn old_nested_transactions_become_flat_files_next_to_content() {
        let t = std::env::temp_dir().join(format!("hmb-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t);
        let (app, blog) = (t.join("app"), t.join("site/content"));
        let w = |p: PathBuf, s: &str| {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, s).unwrap();
        };
        w(blog.join("aws/x/index.md"), "---\ntitle: X\n---\n");
        w(blog.join("transactions/2026/11/2026-11-22_mac.json"), "{}");
        w(blog.join("transactions/transaction-2026-11-23-00-05.json"), "{\"client\":\"mac\",\"entries\":[]}");
        w(blog.join("snippets.json"), "[]");
        w(app.join("content/b/y/index.md"), "---\ntitle: Y\n---\n");
        w(app.join("transactions/2026/11/2026-11-22_mac.json"), "{}");
        w(app.join("transactions/pending.json"), "[]");
        let cfg = Config {
            data_dir: app.clone(),
            content_subdir: "content".into(),
            sync_dir: blog.clone(),
            ..Default::default()
        };
        println!("{:?}", migrate_layout(&cfg));
        let site_tx: Vec<String> = fs::read_dir(t.join("site/transactions")).unwrap().flatten()
            .map(|e| e.file_name().to_string_lossy().to_string()).collect();
        assert_eq!(site_tx, vec!["transaction-2026-11-23-00-05.json".to_string()]);
        assert!(!blog.join("transactions").exists() && !blog.join("snippets.json").exists());
        assert!(t.join("site/snippets.json").exists());
        assert!(!app.join("transactions/2026").exists() && !app.join("transactions/pending.json").exists());
        assert!(blog.join("aws/x/index.md").exists());
        // a sync now writes its transaction file next to the blog's content folder
        let a = store::load_articles(&cfg).into_iter().next().unwrap();
        store::log_tx(&cfg, &a, "updated").unwrap();
        let r = sync(&cfg).unwrap();
        println!("{}", r.log.join("\n"));
        assert!(blog.join("b/y/index.md").exists());
        assert_eq!(fs::read_dir(t.join("site/transactions")).unwrap().count(), 2);
        let _ = fs::remove_dir_all(&t);
    }
}


#[cfg(test)]
mod nested_content_tests {
    use super::*;

    #[test]
    fn files_in_hugo_content_move_to_site_root_for_sub_folder_sync() {
        let t = std::env::temp_dir().join(format!("hmb-nested-{}", std::process::id()));
        let _ = fs::remove_dir_all(&t);
        let site = t.join("pearls");
        let w = |p: PathBuf, s: &str| {
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, s).unwrap();
        };
        w(site.join("hugo.yaml"), "title: x");
        w(site.join("content/post/a/index.md"), "---\ntitle: A\n---\n");
        w(site.join("content/transactions/transaction-2026-11-25-10-27.json"), "{\"entries\":[]}");
        w(site.join("transactions/transaction-2026-11-23-11-38.json"), "{\"entries\":[]}");
        w(site.join("content/snippets.json"), "[]");
        w(site.join("content/tags.json"), "{\"parents\":{}}");
        let cfg = Config {
            data_dir: t.join("app"),
            content_subdir: "post".into(),
            sync_dir: site.join("content/post"),
            ..Default::default()
        };
        fs::create_dir_all(t.join("app/post")).unwrap();
        println!("{:?}", migrate_layout(&cfg));
        assert!(!site.join("content/transactions").exists());
        assert!(!site.join("content/snippets.json").exists() && !site.join("content/tags.json").exists());
        assert!(site.join("transactions/transaction-2026-11-25-10-27.json").exists());
        assert!(site.join("transactions/transaction-2026-11-23-11-38.json").exists());
        assert!(site.join("content/post/a/index.md").exists());
        let _ = fs::remove_dir_all(&t);
    }
}
