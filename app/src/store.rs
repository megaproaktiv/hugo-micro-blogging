//! Articles, frontmatter, transaction logs, snippets, tag ontology and settings.
//!
//! Layout of the data folder (it is also the Git repository that gets synced):
//!
//! ```text
//! <data>/content/2026/11/22/<slug>/index.md          one article (Hugo page bundle)
//! <data>/transactions/2026/11/2026-11-22_<client>.json   what changed that day on that client
//! <data>/snippets.json                                snippets collected via right-click
//! <data>/tags.json                                    tag ontology (child -> parent)
//! ```

use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const CONTENT_DIR: &str = "content";
pub const TX_DIR: &str = "transactions";

// ---------------------------------------------------------------- settings

/// How articles are arranged below the content folder.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// `JJJJ/MM/TT/<name>/index.md`, abstract in the frontmatter
    #[default]
    Dated,
    /// `<bereich>/<unterbereich>/<name>/index.md` (Hugo sections), abstract before `<!--more-->`
    Sections,
}

/// How an added image is written into the Markdown.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ImageStyle {
    /// `{{< figure src="bild.png" width="600" alt="bild" >}}`
    #[default]
    Figure,
    /// `![bild](bild.png)`
    Markdown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub data_dir: PathBuf,
    /// Content folder inside the data folder ("" = the data folder itself).
    pub content_subdir: String,
    pub layout: Layout,
    /// Section for new articles in the sections layout, e.g. `aws/mock`.
    pub default_section: String,
    pub image_style: ImageStyle,
    pub image_width: u32,
    /// Written as `author:` into new articles when not empty.
    pub author: String,
    /// Second local folder to sync with (e.g. the content folder of the Hugo blog).
    pub sync_dir: PathBuf,
    /// Stamp `YYYY-MM-dd-HH-mm` of the last processed transaction file …
    pub last_tx: String,
    /// … and the files with exactly that stamp that were already processed.
    pub last_tx_files: Vec<String>,
    pub client: String,
    /// RFC 3339 time of the last successful sync.
    pub last_sync: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let docs = dirs::document_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Config {
            data_dir: docs.join("hmb"),
            content_subdir: CONTENT_DIR.into(),
            layout: Layout::Dated,
            default_section: String::new(),
            image_style: ImageStyle::Figure,
            image_width: 600,
            author: String::new(),
            sync_dir: PathBuf::new(),
            last_tx: String::new(),
            last_tx_files: Vec::new(),
            client: default_client_name(),
            last_sync: None,
        }
    }
}

impl Config {
    /// Folder that holds the articles.
    pub fn content_root(&self) -> PathBuf {
        let sub = self.content_subdir.trim().trim_matches('/');
        if sub.is_empty() {
            self.data_dir.clone()
        } else {
            self.data_dir.join(sub)
        }
    }

    /// Folder next to the content folder: holds `transactions/`, `snippets.json`, `tags.json`.
    /// Never inside the content folder, so nothing of it is mistaken for content.
    pub fn meta_dir(&self) -> PathBuf {
        let sub = self.content_subdir.trim().trim_matches('/');
        if sub.is_empty() {
            meta_of(&self.data_dir)
        } else {
            self.data_dir.clone()
        }
    }

    /// Article folder relative to the content folder (`content/aws/x` -> `aws/x`).
    pub fn content_rel(&self, rel_dir: &str) -> String {
        let d = rel_dir.trim_matches('/');
        d.strip_prefix(&self.root_prefix()).unwrap_or(d).to_string()
    }

    /// Content folder relative to the data folder, with a trailing `/` (or empty).
    pub fn root_prefix(&self) -> String {
        let sub = self.content_subdir.trim().trim_matches('/');
        if sub.is_empty() {
            String::new()
        } else {
            format!("{sub}/")
        }
    }
}

fn default_client_name() -> String {
    let host = std::env::var("HOSTNAME")
        .ok()
        .or_else(|| {
            std::process::Command::new("hostname")
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "client".into());
    let s = slugify(host.split('.').next().unwrap_or("client"), 24);
    if s.is_empty() {
        "client".into()
    } else {
        s
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hmb")
        .join("config.json")
}

/// Settings of the earlier versions (before the rename to hmb).
fn old_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Microblog") // settings folder of versions before the rename to hmb
        .join("config.json")
}

pub fn load_config() -> Config {
    fs::read_to_string(config_path())
        .or_else(|_| fs::read_to_string(old_config_path()))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_config(cfg: &Config) -> Result<(), String> {
    let p = config_path();
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let s = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(p, s).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- articles

pub const MORE: &str = "<!--more-->";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FrontMatter {
    pub title: String,
    pub date: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub id: String,
    pub draft: bool,
    pub tags: Vec<String>,
    #[serde(rename = "abstract", skip_serializing_if = "String::is_empty")]
    pub abstract_: String,
    /// every other frontmatter field (categories, author, toc, weight …) is kept as it is
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Article {
    pub id: String,
    /// Folder relative to the data folder, e.g. `content/2026/11/22/hallo-welt`.
    pub rel_dir: String,
    pub title: String,
    /// Local time, edited as `YYYY-MM-DD HH:MM`.
    pub date: NaiveDateTime,
    pub tags: Vec<String>,
    pub abstract_: String,
    pub body: String,
    pub draft: bool,
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

impl Article {
    pub fn dir(&self, data: &Path) -> PathBuf {
        data.join(&self.rel_dir)
    }

    pub fn file(&self, data: &Path) -> PathBuf {
        self.dir(data).join("index.md")
    }

    pub fn to_markdown(&self, layout: Layout) -> String {
        let in_front = layout == Layout::Dated;
        let fm = FrontMatter {
            title: self.title.clone(),
            date: format_date_rfc3339(&self.date),
            id: self.id.clone(),
            draft: self.draft,
            tags: self.tags.clone(),
            abstract_: if in_front { self.abstract_.trim().to_string() } else { String::new() },
            extra: self.extra.clone(),
        };
        let yaml = serde_yaml::to_string(&fm).unwrap_or_default();
        let mut out = String::from("---\n");
        out.push_str(&yaml);
        if !yaml.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("---\n\n");
        if !in_front && !self.abstract_.trim().is_empty() {
            // Hugo: everything before <!--more--> is the summary
            out.push_str(self.abstract_.trim());
            out.push_str("\n\n");
            out.push_str(MORE);
            out.push_str("\n\n");
        }
        out.push_str(self.body.trim_end());
        out.push('\n');
        out
    }

    /// Section path for the sections layout, e.g. `aws/mock` for `aws/mock/overcast`.
    pub fn section(&self, cfg: &Config) -> String {
        let rel = self.rel_dir.strip_prefix(&cfg.root_prefix()).unwrap_or(&self.rel_dir);
        match rel.rfind('/') {
            Some(p) => rel[..p].to_string(),
            None => String::new(),
        }
    }
}

pub fn format_date_rfc3339(d: &NaiveDateTime) -> String {
    match Local.from_local_datetime(d).single() {
        Some(dt) => dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        None => d.format("%Y-%m-%dT%H:%M:%S").to_string(),
    }
}

pub fn parse_date(s: &str) -> Option<NaiveDateTime> {
    let s = s.trim().trim_matches('"').trim_matches('\'');
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Local).naive_local());
    }
    for f in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
        if let Ok(d) = NaiveDateTime::parse_from_str(s, f) {
            return Some(d);
        }
    }
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
}

pub fn parse_article(text: &str, rel_dir: &str, cfg: &Config) -> Article {
    let text = text.trim_start_matches('\u{feff}');
    let (fm, body) = split_frontmatter(text);
    let fm: FrontMatter = fm
        .and_then(|y| serde_yaml::from_str(y).ok())
        .unwrap_or_default();
    let date = parse_date(&fm.date)
        .or_else(|| date_from_dir(rel_dir))
        .unwrap_or_else(|| Local::now().naive_local());
    let id = if fm.id.trim().is_empty() {
        make_id(cfg, rel_dir, &date)
    } else {
        fm.id.trim().to_string()
    };
    let mut body = body.trim_start_matches(['\n', '\r']).to_string();
    let mut abstract_ = fm.abstract_.clone();
    if abstract_.trim().is_empty() {
        if let Some(p) = body.find(MORE) {
            abstract_ = body[..p].trim().to_string();
            body = body[p + MORE.len()..].trim_start_matches(['\n', '\r']).to_string();
        }
    }
    Article {
        id,
        rel_dir: rel_dir.to_string(),
        title: fm.title,
        date,
        tags: normalize_tags(&fm.tags),
        abstract_,
        body,
        draft: fm.draft,
        extra: fm.extra,
    }
}

fn split_frontmatter(text: &str) -> (Option<&str>, &str) {
    let t = text.strip_prefix("---\r\n").or_else(|| text.strip_prefix("---\n"));
    if let Some(rest) = t {
        for marker in ["\n---\n", "\n---\r\n"] {
            if let Some(pos) = rest.find(marker) {
                return (Some(&rest[..pos + 1]), &rest[pos + marker.len()..]);
            }
        }
        if let Some(stripped) = rest.strip_suffix("\n---") {
            return (Some(stripped), "");
        }
    }
    (None, text)
}

/// The article ID: year, month, day plus the folder path.
/// Dated: `content/2026/11/22/slug` -> `2026-11-22-slug`.
/// Sections: `aws/mock/overcast` (date 2026-10-11) -> `2026-10-11-aws-mock-overcast`.
pub fn make_id(cfg: &Config, rel_dir: &str, date: &NaiveDateTime) -> String {
    let rel = rel_dir.strip_prefix(&cfg.root_prefix()).unwrap_or(rel_dir);
    let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
    let n = parts.len();
    let dated = n >= 4
        && parts[n - 4].len() == 4
        && parts[n - 4].chars().all(|c| c.is_ascii_digit())
        && parts[n - 3].chars().all(|c| c.is_ascii_digit())
        && parts[n - 2].chars().all(|c| c.is_ascii_digit());
    if dated {
        format!("{}-{}-{}-{}", parts[n - 4], parts[n - 3], parts[n - 2], parts[n - 1])
    } else {
        format!("{}-{}", date.format("%Y-%m-%d"), parts.join("-"))
    }
}

fn date_from_dir(rel_dir: &str) -> Option<NaiveDateTime> {
    let parts: Vec<&str> = rel_dir.split('/').collect();
    let n = parts.len();
    if n < 4 {
        return None;
    }
    let s = format!("{}-{}-{}", parts[n - 4], parts[n - 3], parts[n - 2]);
    parse_date(&s)
}

/// Reads every `index.md` below the content folder.
pub fn load_articles(cfg: &Config) -> Vec<Article> {
    let data = &cfg.data_dir;
    let mut out = Vec::new();
    walk(&cfg.content_root(), &mut |p| {
        if p.file_name().map(|n| n == "index.md").unwrap_or(false) {
            if let (Ok(text), Some(dir)) = (fs::read_to_string(p), p.parent()) {
                if let Ok(rel) = dir.strip_prefix(data) {
                    let rel = rel.to_string_lossy().replace('\\', "/");
                    out.push(parse_article(&text, &rel, cfg));
                }
            }
        }
    });
    out.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
    out
}

fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let hidden = p
            .file_name()
            .map(|n| n.to_string_lossy().starts_with('.'))
            .unwrap_or(false);
        if hidden {
            continue;
        }
        if p.is_dir() {
            walk(&p, f);
        } else {
            f(&p);
        }
    }
}

/// Clean section path: `AWS / Mock` -> `aws/mock`.
pub fn clean_section(s: &str) -> String {
    s.split('/')
        .map(|p| slugify(p, 40))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

/// Plans the folder for a new article (unique), depending on the layout.
pub fn new_article(cfg: &Config, title: &str, section: &str) -> Article {
    let now = Local::now().naive_local();
    let parent = match cfg.layout {
        Layout::Dated => now.format("%Y/%m/%d").to_string(),
        Layout::Sections => clean_section(section),
    };
    let base_dir = if parent.is_empty() { cfg.content_root() } else { cfg.content_root().join(&parent) };
    let mut base = slugify(title, 40);
    if base.is_empty() {
        base = format!("eintrag-{}", now.format("%Y%m%d-%H%M"));
    }
    let mut slug = base.clone();
    let mut n = 2;
    while base_dir.join(&slug).exists() {
        slug = format!("{base}-{n}");
        n += 1;
    }
    let rel_dir = if parent.is_empty() {
        format!("{}{slug}", cfg.root_prefix())
    } else {
        format!("{}{parent}/{slug}", cfg.root_prefix())
    };
    let mut extra = BTreeMap::new();
    if cfg.layout == Layout::Sections {
        if let Some(first) = parent.split('/').next().filter(|s| !s.is_empty()) {
            extra.insert(
                "categories".into(),
                serde_yaml::Value::Sequence(vec![serde_yaml::Value::String(first.to_string())]),
            );
        }
    }
    if !cfg.author.trim().is_empty() {
        extra.insert("author".into(), serde_yaml::Value::String(cfg.author.trim().to_string()));
    }
    Article {
        id: make_id(cfg, &rel_dir, &now),
        rel_dir,
        title: title.to_string(),
        date: now,
        tags: vec![],
        abstract_: String::new(),
        body: String::new(),
        draft: false,
        extra,
    }
}

pub fn save_article(cfg: &Config, a: &Article) -> Result<(), String> {
    let f = a.file(&cfg.data_dir);
    if let Some(d) = f.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    fs::write(&f, a.to_markdown(cfg.layout)).map_err(|e| e.to_string())
}

pub fn delete_article(cfg: &Config, a: &Article) -> Result<(), String> {
    let dir = a.dir(&cfg.data_dir);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    }
    // tidy up folders that became empty
    let stop = cfg.content_root();
    let mut p = dir.parent().map(|p| p.to_path_buf());
    while let Some(d) = p {
        if d == stop || !d.starts_with(&stop) {
            break;
        }
        let _ = fs::remove_file(d.join(".DS_Store"));
        if fs::remove_dir(&d).is_err() {
            break;
        }
        p = d.parent().map(|p| p.to_path_buf());
    }
    Ok(())
}

/// All section paths that already hold articles (for the sections layout).
pub fn known_sections(cfg: &Config, articles: &[Article]) -> Vec<String> {
    let set: BTreeSet<String> = articles
        .iter()
        .map(|a| a.section(cfg))
        .filter(|s| !s.is_empty() && !s.chars().next().unwrap_or('x').is_ascii_digit())
        .collect();
    set.into_iter().collect()
}

// ---------------------------------------------------------------- images

pub const IMAGE_EXT: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "svg"];

pub fn is_image(p: &Path) -> bool {
    p.extension()
        .map(|e| IMAGE_EXT.contains(&e.to_string_lossy().to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Copies an image into the article's folder (page bundle) and returns the file name used.
pub fn add_image(cfg: &Config, a: &Article, src: &Path) -> Result<String, String> {
    if !is_image(src) {
        return Err(format!("{} ist kein Bild (png, jpg, gif, webp, svg)", src.display()));
    }
    let dir = a.dir(&cfg.data_dir);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| "png".into());
    let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut base = slugify(&stem, 50);
    if base.is_empty() {
        base = "bild".into();
    }
    let mut name = format!("{base}.{ext}");
    let mut n = 2;
    while dir.join(&name).exists() {
        // the same picture again? then reuse it
        if fs::read(dir.join(&name)).ok() == fs::read(src).ok() {
            return Ok(name);
        }
        name = format!("{base}-{n}.{ext}");
        n += 1;
    }
    fs::copy(src, dir.join(&name)).map_err(|e| e.to_string())?;
    Ok(name)
}

/// The Markdown (or Hugo shortcode) for an image file in the article's folder.
pub fn image_markup(cfg: &Config, file: &str) -> String {
    let alt = Path::new(file)
        .file_stem()
        .map(|s| s.to_string_lossy().replace(['-', '_'], " "))
        .unwrap_or_default();
    match cfg.image_style {
        ImageStyle::Figure => format!(
            "{{{{< figure src=\"{file}\" width=\"{}\" alt=\"{alt}\" >}}}}",
            cfg.image_width.max(50)
        ),
        ImageStyle::Markdown => format!("![{alt}]({file})"),
    }
}

/// Makes images visible in the preview: Hugo `figure` shortcodes become Markdown images and
/// relative image paths point into the article's folder.
pub fn preview_markdown(md: &str, dir: &Path) -> String {
    let abs = |src: &str| -> String {
        if src.contains("://") || src.starts_with('/') {
            src.to_string()
        } else {
            format!("file://{}", dir.join(src).display())
        }
    };
    // 1) shortcodes {{< figure src="…" alt="…" >}}
    let mut out = String::new();
    let mut rest = md;
    while let Some(start) = rest.find("{{<") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(end) = after.find(">}}") else {
            out.push_str(after);
            rest = "";
            break;
        };
        let code = &after[3..end];
        let name = code.split_whitespace().next().unwrap_or("");
        if name == "figure" || name == "img" || name == "image" {
            let src = attr(code, "src").unwrap_or_default();
            let alt = attr(code, "alt").or_else(|| attr(code, "caption")).unwrap_or_default();
            out.push_str(&format!("![{alt}](<{}>)", abs(&src)));
        } else {
            out.push_str(&after[..end + 3]);
        }
        rest = &after[end + 3..];
    }
    out.push_str(rest);
    // 2) relative Markdown images ![alt](file.png)
    let mut res = String::new();
    let mut rest = out.as_str();
    while let Some(start) = rest.find("![") {
        res.push_str(&rest[..start]);
        let after = &rest[start..];
        let (Some(mid), Some(close)) = (after.find("]("), after.find(')')) else {
            res.push_str(after);
            rest = "";
            break;
        };
        if mid > close || after[..mid].contains('\n') {
            res.push_str("![");
            rest = &after[2..];
            continue;
        }
        let url = after[mid + 2..close].trim();
        if url.starts_with('<') {
            res.push_str(&after[..close + 1]);
        } else {
            res.push_str(&format!("{}](<{}>)", &after[..mid], abs(url)));
        }
        rest = &after[close + 1..];
    }
    res.push_str(rest);
    res
}

fn attr(code: &str, key: &str) -> Option<String> {
    let pat = format!("{key}=");
    let p = code.find(&pat)? + pat.len();
    let v = &code[p..];
    let q = v.chars().next()?;
    if q == '"' || q == '\'' {
        let end = v[1..].find(q)? + 1;
        Some(v[1..end].to_string())
    } else {
        Some(v.split_whitespace().next()?.to_string())
    }
}

/// Plain text for the list: without shortcodes, images and link syntax.
pub fn plain_text(md: &str) -> String {
    let mut out = String::new();
    let mut rest = md;
    while let Some(s) = rest.find("{{<") {
        out.push_str(&rest[..s]);
        match rest[s..].find(">}}") {
            Some(e) => rest = &rest[s + e + 3..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    let mut res = String::new();
    let chars: Vec<char> = out.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '!' && chars.get(i + 1) == Some(&'[') {
            // drop images completely
            if let Some(close) = chars[i..].iter().position(|&x| x == ')') {
                i += close + 1;
                continue;
            }
        }
        if c == '[' {
            if let Some(mid) = chars[i..].iter().position(|&x| x == ']') {
                if chars.get(i + mid + 1) == Some(&'(') {
                    if let Some(close) = chars[i + mid..].iter().position(|&x| x == ')') {
                        res.extend(&chars[i + 1..i + mid]);
                        i += mid + close + 1;
                        continue;
                    }
                }
            }
        }
        if c != '*' && c != '`' && c != '#' {
            res.push(if c == '\n' { ' ' } else { c });
        }
        i += 1;
    }
    res.split_whitespace().collect::<Vec<_>>().join(" ")
}
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for t in tags {
        let t = normalize_tag(t);
        if !t.is_empty() && seen.insert(t.clone()) {
            out.push(t);
        }
    }
    out
}

pub fn normalize_tag(t: &str) -> String {
    t.trim()
        .trim_start_matches('#')
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

pub fn slugify(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.to_lowercase().chars() {
        let rep: &str = match c {
            'ä' => "ae",
            'ö' => "oe",
            'ü' => "ue",
            'ß' => "ss",
            'é' | 'è' | 'ê' => "e",
            'á' | 'à' | 'â' => "a",
            'ó' | 'ò' | 'ô' => "o",
            'í' | 'ì' | 'î' => "i",
            'ú' | 'ù' | 'û' => "u",
            _ => "",
        };
        if !rep.is_empty() {
            out.push_str(rep);
            dash = false;
        } else if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
        if out.len() >= max {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

// ---------------------------------------------------------------- transactions
//
// Local changes are collected in `<data>/transactions/pending.json` until the next sync.
// On sync they are copied into the sync folder and described there in
// `<sync>/transactions/transaction-YYYY-MM-dd-HH-mm.json` (one file per sync run).

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TxEntry {
    pub time: String,
    pub id: String,
    /// created | updated | deleted
    pub action: String,
    /// article folder relative to the data / sync folder, e.g. `aws/mock/overcast`
    pub path: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub client: String,
}

impl TxEntry {
    /// Folder of the article (older files stored `…/index.md`).
    pub fn dir(&self) -> String {
        self.path.trim_end_matches("/index.md").trim_matches('/').to_string()
    }
}

/// Contents of one `transaction-YYYY-MM-dd-HH-mm.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct TxFile {
    pub created: String,
    pub client: String,
    pub entries: Vec<TxEntry>,
}

/// The level of a content folder: its parent (transactions etc. live there).
pub fn meta_of(content: &Path) -> PathBuf {
    content
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| content.to_path_buf())
}

/// The Hugo website a content folder belongs to (the folder above it with a Hugo config file).
pub fn hugo_site_of(content: &Path) -> Option<(PathBuf, String)> {
    let site = meta_of(content);
    ["hugo.toml", "hugo.yaml", "hugo.yml", "hugo.json", "config.toml", "config.yaml", "config.json"]
        .iter()
        .find(|f| site.join(f).is_file())
        .map(|f| (site.clone(), f.to_string()))
}

/// Local changes not yet synced (app-internal, next to the content folder).
pub fn pending_path(meta: &Path) -> PathBuf {
    meta.join(".hmb-pending.json")
}

pub fn load_pending(data: &Path) -> Vec<TxEntry> {
    fs::read_to_string(pending_path(data))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_pending(data: &Path, p: &[TxEntry]) -> Result<(), String> {
    if p.is_empty() {
        let _ = fs::remove_file(pending_path(data));
        return Ok(());
    }
    write_json(&pending_path(data), &p)
}

/// Notes a local change for the next sync. Several changes of one article are folded into one.
pub fn log_tx(cfg: &Config, a: &Article, action: &str) -> Result<(), String> {
    let data = &cfg.meta_dir();
    let client = cfg.client.as_str();
    let path = cfg.content_rel(&a.rel_dir);
    let mut pending = load_pending(data);
    let entry = TxEntry {
        time: Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        id: a.id.clone(),
        action: action.to_string(),
        path: path.clone(),
        title: a.title.clone(),
        client: client.to_string(),
    };
    match pending.iter_mut().find(|e| e.dir() == path) {
        Some(old) => {
            let action = match (old.action.as_str(), action) {
                // created and changed again before syncing: still "created"
                ("created", "updated") => "created",
                _ => action,
            };
            *old = TxEntry { action: action.to_string(), ..entry };
        }
        None => pending.push(entry),
    }
    save_pending(data, &pending)
}

/// `transaction-2026-11-23-09-41.json` (or `…-09-41-<client>.json`) -> `2026-11-23-09-41`
pub fn tx_stamp(file_name: &str) -> Option<String> {
    let rest = file_name.strip_prefix("transaction-")?.strip_suffix(".json")?;
    let stamp = rest.get(..16)?;
    let ok = stamp.chars().enumerate().all(|(i, c)| {
        if [4, 7, 10, 13].contains(&i) {
            c == '-'
        } else {
            c.is_ascii_digit()
        }
    });
    ok.then(|| stamp.to_string())
}

/// All transaction files in `<meta>/transactions/` (flat, no sub folders), sorted by (stamp, name).
pub fn list_tx_files(base: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = fs::read_dir(base.join(TX_DIR))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    tx_stamp(&name).map(|s| (s, name))
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

pub fn read_tx_file(base: &Path, name: &str) -> Option<TxFile> {
    fs::read_to_string(base.join(TX_DIR).join(name))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

/// Writes a new transaction file into `base/transactions/` and returns its name.
pub fn write_tx_file(base: &Path, client: &str, entries: &[TxEntry]) -> Result<String, String> {
    let now = Local::now();
    let stamp = now.format("%Y-%m-%d-%H-%M").to_string();
    let dir = base.join(TX_DIR);
    let mut name = format!("transaction-{stamp}.json");
    let mut n = 2;
    while dir.join(&name).exists() {
        name = if n == 2 {
            format!("transaction-{stamp}-{client}.json")
        } else {
            format!("transaction-{stamp}-{client}-{n}.json")
        };
        n += 1;
    }
    let f = TxFile {
        created: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        client: client.to_string(),
        entries: entries.to_vec(),
    };
    write_json(&dir.join(&name), &f)?;
    Ok(name)
}

fn write_json<T: Serialize>(f: &Path, v: &T) -> Result<(), String> {
    if let Some(d) = f.parent() {
        fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let s = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    fs::write(f, s).map_err(|e| e.to_string())
}

/// Article folders (holding an `index.md`) below `root`, relative to `root`.
pub fn article_dirs(root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    walk(root, &mut |p| {
        if p.file_name().map(|n| n == "index.md").unwrap_or(false) {
            if let Some(rel) = p.parent().and_then(|d| d.strip_prefix(root).ok()) {
                let rel = rel.to_string_lossy().replace('\\', "/");
                if !rel.is_empty() {
                    out.insert(rel);
                }
            }
        }
    });
    out
}

// ---------------------------------------------------------------- snippets

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snippet {
    pub created: String,
    pub text: String,
    #[serde(default)]
    pub source: String,
}

pub fn snippets_path(data: &Path) -> PathBuf {
    data.join("snippets.json")
}

pub fn load_snippets(data: &Path) -> Vec<Snippet> {
    fs::read_to_string(snippets_path(data))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_snippets(data: &Path, s: &[Snippet]) -> Result<(), String> {
    write_json(&snippets_path(data), &s)
}

pub fn add_snippet(data: &Path, text: &str, source: &str) -> Result<(), String> {
    let text = text.trim_end();
    if text.trim().is_empty() {
        return Err("leerer Text".into());
    }
    let mut all = load_snippets(data);
    all.insert(
        0,
        Snippet {
            created: Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
            text: text.to_string(),
            source: source.to_string(),
        },
    );
    save_snippets(data, &all)
}

// ---------------------------------------------------------------- tag ontology

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Ontology {
    /// child tag -> parent tag
    pub parents: BTreeMap<String, String>,
}

impl Ontology {
    pub fn load(data: &Path) -> Self {
        fs::read_to_string(data.join("tags.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, data: &Path) -> Result<(), String> {
        write_json(&data.join("tags.json"), self)
    }
    pub fn parent(&self, t: &str) -> Option<&str> {
        self.parents.get(t).map(|s| s.as_str())
    }
    pub fn children(&self, t: &str) -> Vec<String> {
        self.parents
            .iter()
            .filter(|(_, p)| p.as_str() == t)
            .map(|(c, _)| c.clone())
            .collect()
    }
    /// `t` and everything below it.
    pub fn descendants(&self, t: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut stack = vec![t.to_string()];
        while let Some(x) = stack.pop() {
            if out.insert(x.clone()) {
                stack.extend(self.children(&x));
            }
        }
        out
    }
    /// Would making `parent` the parent of `child` create a loop?
    pub fn would_cycle(&self, child: &str, parent: &str) -> bool {
        self.descendants(child).contains(parent)
    }
}

// ---------------------------------------------------------------- demo data

/// Writes a few example articles (used for the screenshot when there is no data yet).
pub fn write_demo(cfg: &Config) {
    let data = &cfg.meta_dir();
    let samples: [(&str, &str, &[&str], &str, &str); 6] = [
        (
            "Willkommen bei hmb",
            "2026-11-22 09:15",
            &["meta", "notizen"],
            "Kurze Einträge in Markdown schreiben, mit Tags ordnen und per Knopfdruck mit dem Blog-Ordner abgleichen. Jeder Eintrag liegt als eigene Datei im Ordner Content – mit seinen Bildern direkt daneben.",
            "## Schön, dass du da bist\n\nDieser Eintrag zeigt, was **Markdown** kann:\n\n- *kursiv* und **fett**\n- [Links](https://gohugo.io)\n- `Code` im Text\n\n> Ein Zitat sieht so aus.\n\n### Bilder\n\nEinfach ein Bild ins Fenster ziehen oder oben auf **🖼 Bild** klicken – es wird neben den Artikel kopiert und hier eingefügt.\n\n### Nächste Schritte\n\n1. Oben auf **Neu** klicken\n2. Tags vergeben\n3. Synchronisieren\n",
        ),
        (
            "Hugo-Seite vorbereitet",
            "2026-11-21 18:40",
            &["hugo", "web"],
            "Die Ordnerstruktur passt schon zu Hugo: jeder Eintrag ist ein Page Bundle mit index.md, Frontmatter enthält Titel, Datum, Tags und Abstract.",
            "Die Seite kommt in Stufe 2.\n",
        ),
        (
            "Rust und egui ausprobiert",
            "2026-11-20 21:05",
            &["rust", "programmieren"],
            "Ein kleines Fenster mit Seitenleiste, Liste und Editor. Erstaunlich wenig Code für eine Oberfläche, die sich schnell anfühlt und überall läuft.",
            "```rust\nfn main() {\n    println!(\"Hallo\");\n}\n```\n",
        ),
        (
            "Lesenotiz: Gute Tags",
            "2026-11-19 07:30",
            &["notizen", "lesen"],
            "Tags sind frei vergebbar. Später lassen sie sich ordnen: \"rust\" unter \"programmieren\", \"hugo\" unter \"web\". So bleibt die Seitenleiste übersichtlich und die Suche trotzdem genau.",
            "Ontologie folgt.\n",
        ),
        (
            "Wochenmarkt",
            "2026-11-18 12:10",
            &["alltag"],
            "Äpfel, Kürbis und frisches Brot. Der Stand mit dem Bergkäse hatte wieder die längste Schlange.",
            "",
        ),
        (
            "Synchronisieren mit mehreren Rechnern",
            "2026-11-17 16:55",
            &["meta", "git"],
            "Jeder Abgleich schreibt eine Transaktionsdatei mit Zeitstempel. Beim nächsten Mal werden nur die neuen Transaktionen gelesen – zuerst holen, dann schreiben.",
            "",
        ),
    ];
    let mut onto = Ontology::default();
    onto.parents.insert("rust".into(), "programmieren".into());
    onto.parents.insert("hugo".into(), "web".into());
    onto.parents.insert("git".into(), "programmieren".into());
    let _ = onto.save(data);
    for (title, date, tags, abs, body) in samples {
        let d = parse_date(date).unwrap();
        let parent = match cfg.layout {
            Layout::Dated => d.format("%Y/%m/%d").to_string(),
            Layout::Sections => tags[0].to_string(),
        };
        let rel_dir = format!("{}{parent}/{}", cfg.root_prefix(), slugify(title, 40));
        let a = Article {
            id: make_id(cfg, &rel_dir, &d),
            rel_dir,
            title: title.into(),
            date: d,
            tags: tags.iter().map(|s| s.to_string()).collect(),
            abstract_: abs.into(),
            body: body.into(),
            draft: false,
            extra: BTreeMap::new(),
        };
        let _ = save_article(cfg, &a);
        let _ = log_tx(cfg, &a, "created");
    }
    let _ = add_snippet(data, "Gefunden: Hugo kann Page Bundles mit Bildern direkt neben index.md.", "Beispiel");
    let _ = add_snippet(data, "> Weniger, aber besser.", "Beispiel");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round trip over a real content folder: `HMB_FIXTURE_CONTENT=/path/to/content cargo test`.
    #[test]
    fn sections_round_trip_keeps_everything() {
        let Ok(dir) = std::env::var("HMB_FIXTURE_CONTENT") else { return };
        let cfg = Config {
            data_dir: PathBuf::from(&dir),
            content_subdir: String::new(),
            layout: Layout::Sections,
            ..Default::default()
        };
        let arts = load_articles(&cfg);
        assert!(!arts.is_empty());
        for a in &arts {
            let md = a.to_markdown(Layout::Sections);
            let b = parse_article(&md, &a.rel_dir, &cfg);
            assert_eq!(a.title, b.title, "{}", a.rel_dir);
            assert_eq!(a.tags, b.tags, "{}", a.rel_dir);
            assert_eq!(a.abstract_.trim(), b.abstract_.trim(), "{}", a.rel_dir);
            assert_eq!(a.body.trim(), b.body.trim(), "{}", a.rel_dir);
            assert_eq!(a.extra, b.extra, "{}", a.rel_dir);
            assert_eq!(a.date, b.date, "{}", a.rel_dir);
        }
        let first = arts.iter().find(|a| a.extra.contains_key("categories")).expect("categories kept");
        println!("{} Artikel geprüft, z. B. {}:\n{}", arts.len(), first.rel_dir,
            first.to_markdown(Layout::Sections).lines().take(14).collect::<Vec<_>>().join("\n"));
    }

    #[test]
    fn figure_markup_and_preview() {
        let cfg = Config::default();
        assert_eq!(image_markup(&cfg, "mein-bild.png"), "{{< figure src=\"mein-bild.png\" width=\"600\" alt=\"mein bild\" >}}");
        let p = preview_markdown("a {{< figure src=\"x.png\" alt=\"X\" >}} b ![y](y.jpg)", Path::new("/d"));
        assert_eq!(p, "a ![X](<file:///d/x.png>) b ![y](<file:///d/y.jpg>)");
    }
}

#[cfg(test)]
mod hugo_tests {
    use super::*;

    #[test]
    fn finds_hugo_site_above_content() {
        let t = std::env::temp_dir().join(format!("hmb-hugo-{}", std::process::id()));
        fs::create_dir_all(t.join("site/content")).unwrap();
        assert!(hugo_site_of(&t.join("site/content")).is_none());
        fs::write(t.join("site/hugo.toml"), "title = 'x'").unwrap();
        assert_eq!(hugo_site_of(&t.join("site/content")).unwrap().1, "hugo.toml");
        let _ = fs::remove_dir_all(&t);
    }
}
