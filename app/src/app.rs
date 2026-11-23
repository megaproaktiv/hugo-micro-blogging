//! The main window: top bar with tag search, main tags on the left, a scrollable list of
//! 10 entries per page (title + abstract, max. 3 lines) and the Markdown editor with preview.

mod editor;
mod images;
mod panels;
mod widgets;
mod windows;

use crate::store::{self, Article, Config, Ontology, Snippet, TxEntry};
use crate::sync;
use crate::tray::{self, Tray, TrayCmd};
use eframe::egui::{self, Color32, Key};
use egui_commonmark::CommonMarkCache;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::SystemTime;

pub(crate) const PAGE: usize = 10;

pub(crate) const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
pub(crate) const ACCENT_SOFT: Color32 = Color32::from_rgb(219, 234, 254);
pub(crate) const INK: Color32 = Color32::from_rgb(17, 24, 39);
pub(crate) const MUTED: Color32 = Color32::from_rgb(107, 114, 128);
pub(crate) const LINE: Color32 = Color32::from_rgb(229, 231, 235);
pub(crate) const SIDEBAR: Color32 = Color32::from_rgb(243, 244, 246);
pub(crate) const LISTBG: Color32 = Color32::from_rgb(249, 250, 251);
pub(crate) const TAGBG: Color32 = Color32::from_rgb(238, 242, 255);
pub(crate) const TAGFG: Color32 = Color32::from_rgb(67, 56, 202);
pub(crate) const DANGER: Color32 = Color32::from_rgb(220, 38, 38);
pub(crate) const OK: Color32 = Color32::from_rgb(22, 163, 74);
pub(crate) const WARN: Color32 = Color32::from_rgb(217, 119, 6);

#[derive(PartialEq, Clone, Copy)]
enum ViewMode {
    Edit,
    Split,
    Preview,
}

struct Editor {
    a: Article,
    saved: Article,
    is_new: bool,
    date_text: String,
    tag_input: String,
    /// selected character range in the body editor
    cursor: Option<(usize, usize)>,
    last_change: f64,
    /// section path for new articles (sections layout)
    section: String,
}

impl Editor {
    fn open(a: Article, is_new: bool) -> Self {
        Editor {
            date_text: a.date.format("%Y-%m-%d %H:%M").to_string(),
            saved: a.clone(),
            a,
            is_new,
            tag_input: String::new(),
            cursor: None,
            last_change: 0.0,
            section: String::new(),
        }
    }

    fn dirty(&self) -> bool {
        if self.is_new {
            !(self.a.title.trim().is_empty()
                && self.a.body.trim().is_empty()
                && self.a.abstract_.trim().is_empty())
        } else {
            self.a != self.saved
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum PickKind {
    Images,
    DataDir,
    SyncDir,
}

pub struct HmbApp {
    cfg: Config,
    data: PathBuf,
    /// screenshot with example data: never write settings
    demo: bool,
    articles: Vec<Article>,
    onto: Ontology,
    snippets: Vec<Snippet>,
    snippets_mtime: Option<SystemTime>,
    last_watch: f64,

    search: String,
    selected_tag: Option<String>,
    expanded: BTreeSet<String>,
    page: usize,
    filter_key: String,

    editor: Option<Editor>,
    view: ViewMode,
    md_cache: CommonMarkCache,
    focus_body: bool,

    status: String,
    status_ok: bool,
    sync_rx: Option<Receiver<Result<sync::SyncResult, String>>>,
    news: Option<Vec<TxEntry>>,

    show_settings: bool,
    settings_draft: Config,
    show_onto: bool,
    show_snippets: bool,
    new_snippet: String,
    confirm_delete: bool,
    /// exact copies one level deeper (left behind by an earlier sync)
    dupes: Vec<PathBuf>,
    confirm_cleanup: bool,
    startup_note: Option<String>,
    /// file dialog running outside the event handler (see `start_pick`)
    pending_pick: Option<(PickKind, std::sync::mpsc::Receiver<Vec<PathBuf>>)>,

    tray: Option<Tray>,
    quitting: bool,
    hint_minimized: bool,

    screenshot: Option<PathBuf>,
    /// `--doc-view <name>`: state shown for a screenshot in doc/ (main, settings, snippets, editor, sync)
    doc_view: Option<String>,
    frames: u32,
    shot_requested: bool,
}

impl HmbApp {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        widgets::setup_style(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let mut cfg = store::load_config();
        let mut demo = false;
        let mut data = cfg.data_dir.clone();
        if screenshot.is_some() && store::load_articles(&cfg).is_empty() {
            // show the app with example entries instead of an empty list
            data = std::env::temp_dir().join(format!("hmb-demo-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&data);
            cfg.data_dir = data.clone();
            if cfg.content_subdir.trim().is_empty() {
                cfg.content_subdir = store::CONTENT_DIR.into();
            }
            store::write_demo(&cfg);
            demo = true;
        }

        let tray = if screenshot.is_none() {
            tray::create(cc.egui_ctx.clone(), widgets::native_view(cc))
        } else {
            None
        };

        let mut app = HmbApp {
            pending_pick: None,
            settings_draft: cfg.clone(),
            cfg,
            data,
            demo,
            articles: vec![],
            onto: Ontology::default(),
            snippets: vec![],
            snippets_mtime: None,
            last_watch: 0.0,
            search: String::new(),
            selected_tag: None,
            expanded: BTreeSet::new(),
            page: 0,
            filter_key: String::new(),
            editor: None,
            view: ViewMode::Split,
            md_cache: CommonMarkCache::default(),
            focus_body: false,
            status: String::new(),
            status_ok: true,
            sync_rx: None,
            news: None,
            show_settings: false,
            show_onto: false,
            show_snippets: false,
            new_snippet: String::new(),
            confirm_delete: false,
            dupes: Vec::new(),
            confirm_cleanup: false,
            startup_note: None,
            tray,
            quitting: false,
            hint_minimized: false,
            screenshot,
            doc_view: None,
            frames: 0,
            shot_requested: false,
        };
        if app.screenshot.is_none() && !app.demo {
            // move files of older versions out of the content folder (transactions, snippets, tags)
            let moved = sync::migrate_layout(&app.cfg);
            if !moved.is_empty() {
                app.startup_note = Some(moved.join(" "));
            }
        }
        app.reload();
        app.expanded = app.onto.parents.values().cloned().collect();
        if app.screenshot.is_some() && !app.articles.is_empty() {
            // HMB_OPEN=<id> picks the article shown in a screenshot (for testing)
            let want = std::env::var("HMB_OPEN").ok();
            let idx = want
                .and_then(|w| app.articles.iter().position(|a| a.id == w || a.rel_dir.ends_with(&w)))
                .unwrap_or(0);
            app.open_article(idx);
        }
        let args: Vec<String> = std::env::args().collect();
        app.doc_view = args.iter().position(|a| a == "--doc-view").and_then(|i| args.get(i + 1)).cloned();
        match app.doc_view.as_deref() {
            Some("settings") => {
                app.settings_draft = app.cfg.clone();
                app.show_settings = true;
            }
            Some("snippets") => app.show_snippets = true,
            Some("editor") => {
                app.view = ViewMode::Split;
                if let Some(ed) = &mut app.editor {
                    ed.tag_input = "le".into(); // shows the tag suggestions while typing
                }
            }
            Some("main") => app.view = ViewMode::Preview,
            _ => {}
        }
        let msg = format!("{} Einträge geladen aus {}", app.articles.len(), widgets::show_path(&app.data));
        app.set_status(msg, true);
        if let Some(note) = app.startup_note.take() {
            app.set_status(note, true);
        }
        app.dupes = sync::nested_copies(&app.cfg);
        if !app.dupes.is_empty() {
            let msg = format!(
                "{} Artikel-Ordner liegen doppelt (von einem früheren Abgleich) – unter Einstellungen (⚙) reparieren.",
                app.dupes.len()
            );
            app.set_status(msg, false);
        }
        app
    }

    // ------------------------------------------------------------ data

    fn reload(&mut self) {
        self.articles = store::load_articles(&self.cfg);
        self.onto = Ontology::load(&self.cfg.meta_dir());
        self.snippets = store::load_snippets(&self.cfg.meta_dir());
        self.snippets_mtime = widgets::mtime(&store::snippets_path(&self.cfg.meta_dir()));
        // refresh an open, unchanged article from disk (e.g. after a sync)
        if let Some(ed) = &mut self.editor {
            if !ed.dirty() && !ed.is_new {
                if let Some(a) = self.articles.iter().find(|a| a.rel_dir == ed.a.rel_dir) {
                    *ed = Editor::open(a.clone(), false);
                }
            }
        }
    }

    fn set_status(&mut self, s: impl Into<String>, ok: bool) {
        self.status = s.into();
        self.status_ok = ok;
    }

    fn ancestors(&self, tag: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut cur = tag.to_string();
        for _ in 0..32 {
            match self.onto.parent(&cur) {
                Some(p) if p != tag && !out.iter().any(|o| o == p) => {
                    out.push(p.to_string());
                    cur = p.to_string();
                }
                _ => break,
            }
        }
        out
    }

    /// The article's tags plus all their parent tags.
    fn expanded_tags(&self, a: &Article) -> BTreeSet<String> {
        let mut s = BTreeSet::new();
        for t in &a.tags {
            s.insert(t.clone());
            s.extend(self.ancestors(t));
        }
        s
    }

    fn all_tags(&self) -> BTreeSet<String> {
        let mut s: BTreeSet<String> = self.articles.iter().flat_map(|a| a.tags.clone()).collect();
        for (c, p) in &self.onto.parents {
            s.insert(c.clone());
            s.insert(p.clone());
        }
        s
    }

    fn tag_counts(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for a in &self.articles {
            for t in self.expanded_tags(a) {
                *m.entry(t).or_insert(0) += 1;
            }
        }
        m
    }

    fn search_terms(&self) -> Vec<String> {
        self.search
            .split(|c: char| c.is_whitespace() || c == ',')
            .map(store::normalize_tag)
            .filter(|t| !t.is_empty())
            .collect()
    }

    fn filtered(&self) -> Vec<usize> {
        let terms = self.search_terms();
        self.articles
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                let tags = self.expanded_tags(a);
                if let Some(sel) = &self.selected_tag {
                    if !tags.contains(sel) {
                        return false;
                    }
                }
                terms.iter().all(|term| tags.iter().any(|t| t.contains(term.as_str())))
            })
            .map(|(i, _)| i)
            .collect()
    }

    // ------------------------------------------------------------ editing

    fn open_article(&mut self, idx: usize) {
        self.save_current();
        if let Some(a) = self.articles.get(idx).cloned() {
            self.editor = Some(Editor::open(a, false));
            self.confirm_delete = false;
        }
    }

    fn open_by_id(&mut self, id: &str) {
        if let Some(i) = self.articles.iter().position(|a| a.id == id) {
            self.open_article(i);
        }
    }

    fn new_entry(&mut self) {
        self.save_current();
        let section = self.cfg.default_section.clone();
        let mut a = store::new_article(&self.cfg, "", &section);
        if let Some(t) = &self.selected_tag {
            a.tags.push(t.clone());
        }
        let mut ed = Editor::open(a, true);
        ed.section = section;
        self.editor = Some(ed);
        self.confirm_delete = false;
        self.set_status("Neuer Eintrag – Titel eingeben, dann schreiben.", true);
    }

    fn save_current(&mut self) {
        self.save_impl(false);
    }

    /// Saves the open article; with `force` even when it is new and still empty.
    fn save_impl(&mut self, force: bool) {
        let Some(ed) = &mut self.editor else { return };
        if !force && !ed.dirty() {
            return;
        }
        if ed.is_new {
            // the folder name (and so the ID) comes from the title at the first save
            let fresh = store::new_article(&self.cfg, &ed.a.title, &ed.section);
            ed.a.rel_dir = fresh.rel_dir;
            ed.a.id = fresh.id;
            for (k, v) in fresh.extra {
                ed.a.extra.entry(k).or_insert(v);
            }
        }
        ed.a.tags = store::normalize_tags(&ed.a.tags);
        let action = if ed.is_new { "created" } else { "updated" };
        match store::save_article(&self.cfg, &ed.a) {
            Ok(()) => {
                let _ = store::log_tx(&self.cfg, &ed.a, action);
                ed.is_new = false;
                ed.saved = ed.a.clone();
                let a = ed.a.clone();
                if let Some(slot) = self.articles.iter_mut().find(|x| x.rel_dir == a.rel_dir) {
                    *slot = a.clone();
                } else {
                    self.articles.push(a.clone());
                }
                self.articles.sort_by(|x, y| y.date.cmp(&x.date).then(y.id.cmp(&x.id)));
                self.set_status(format!("Gespeichert: {}", a.id), true);
            }
            Err(e) => self.set_status(format!("Speichern fehlgeschlagen: {e}"), false),
        }
    }

    fn delete_current(&mut self) {
        let Some(ed) = self.editor.take() else { return };
        if ed.is_new {
            self.set_status("Entwurf verworfen.", true);
            return;
        }
        match store::delete_article(&self.cfg, &ed.saved) {
            Ok(()) => {
                let _ = store::log_tx(&self.cfg, &ed.saved, "deleted");
                self.articles.retain(|a| a.rel_dir != ed.saved.rel_dir);
                self.set_status(format!("Gelöscht: {}", ed.saved.id), true);
            }
            Err(e) => {
                self.set_status(format!("Löschen fehlgeschlagen: {e}"), false);
                self.editor = Some(ed);
            }
        }
    }

    // ------------------------------------------------------------ sync

    /// Normal sync (transactions only) or, with `full`, a complete comparison of both folders.
    fn start_sync(&mut self, ctx: &egui::Context, full: bool) {
        if self.sync_rx.is_some() {
            return;
        }
        if self.demo {
            self.set_status("Beispieldaten werden nicht synchronisiert.", false);
            return;
        }
        self.save_current();
        let (tx, rx) = channel();
        let cfg = self.cfg.clone();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let r = if full { sync::full_sync(&cfg) } else { sync::sync(&cfg) };
            let _ = tx.send(r);
            ctx2.request_repaint();
        });
        self.sync_rx = Some(rx);
        let what = if full { "Vollsynchronisierung läuft" } else { "Synchronisiere mit dem Sync-Ordner" };
        self.set_status(format!("{what} …"), true);
    }

    fn poll_sync(&mut self) {
        let Some(rx) = &self.sync_rx else { return };
        let Ok(res) = rx.try_recv() else { return };
        self.sync_rx = None;
        match res {
            Ok(r) => {
                self.reload();
                self.cfg.last_tx = r.last_tx.clone();
                self.cfg.last_tx_files = r.last_tx_files.clone();
                self.settings_draft.last_tx = r.last_tx;
                self.settings_draft.last_tx_files = r.last_tx_files;
                self.cfg.last_sync =
                    Some(chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false));
                if !self.demo {
                    let _ = store::save_config(&self.cfg);
                }
                self.set_status(format!("Synchronisiert. {}", r.log.join(" ")), true);
                let mut news = r.imported;
                news.truncate(300);
                self.news = Some(news);
            }
            Err(e) => self.set_status(format!("Synchronisieren fehlgeschlagen: {e}"), false),
        }
    }

    fn apply_settings(&mut self) {
        self.save_current();
        let d = &self.settings_draft;
        let changed_dir = d.data_dir != self.cfg.data_dir
            || d.content_subdir != self.cfg.content_subdir
            || d.layout != self.cfg.layout;
        self.settings_draft.default_section = store::clean_section(&self.settings_draft.default_section);
        let c = store::slugify(&self.settings_draft.client, 24);
        self.settings_draft.client = if c.is_empty() { "client".into() } else { c };
        // sync bookkeeping always comes from the live settings
        self.settings_draft.last_tx = self.cfg.last_tx.clone();
        self.settings_draft.last_tx_files = self.cfg.last_tx_files.clone();
        self.settings_draft.last_sync = self.cfg.last_sync.clone();
        self.cfg = self.settings_draft.clone();
        if changed_dir {
            self.data = self.cfg.data_dir.clone();
            self.demo = false;
            self.editor = None;
            self.reload();
        }
        match store::save_config(&self.cfg) {
            Ok(()) => self.set_status("Einstellungen gespeichert.", true),
            Err(e) => self.set_status(format!("Einstellungen nicht gespeichert: {e}"), false),
        }
    }

    // ------------------------------------------------------------ per-frame helpers

    fn handle_tray(&mut self, ctx: &egui::Context) {
        let cmds: Vec<TrayCmd> = match &self.tray {
            Some(t) => t.rx.try_iter().collect(),
            None => return,
        };
        for c in cmds {
            match c {
                TrayCmd::Show => {}
                TrayCmd::New => self.new_entry(),
                TrayCmd::Snippets => self.show_snippets = true,
                TrayCmd::Sync => self.start_sync(ctx, false),
                TrayCmd::Quit => {
                    self.save_current();
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn watch_files(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if now - self.last_watch < 1.0 {
            return;
        }
        self.last_watch = now;
        let m = widgets::mtime(&store::snippets_path(&self.cfg.meta_dir()));
        if m != self.snippets_mtime {
            self.snippets_mtime = m;
            let before = self.snippets.len();
            self.snippets = store::load_snippets(&self.cfg.meta_dir());
            if self.snippets.len() > before {
                self.set_status("Neues Snippet empfangen.", true);
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let (save, new) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, Key::S),
                i.consume_key(egui::Modifiers::COMMAND, Key::N),
            )
        });
        if save {
            self.save_current();
        }
        if new {
            self.new_entry();
        }
    }

    fn close_handling(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) {
            self.save_current();
            if self.tray.is_some() && !self.quitting {
                // keep running in the menu bar
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                if !self.hint_minimized {
                    self.hint_minimized = true;
                    self.set_status(
                        "hmb läuft weiter – über das Symbol in der Menüleiste wieder öffnen oder beenden.",
                        true,
                    );
                }
            }
        }
    }

    fn autosave(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let due = match &self.editor {
            Some(ed) if ed.dirty() => {
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
                now - ed.last_change > 1.5 && !ed.a.title.trim().is_empty()
            }
            _ => false,
        };
        if due {
            self.save_current();
        }
    }

    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        self.frames += 1;
        ctx.request_repaint();
        if self.frames == 3 && self.doc_view.as_deref() == Some("sync") {
            self.start_sync(ctx, false);
        }
        if self.frames >= 120 && !self.shot_requested {
            self.shot_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let [w, h] = img.size;
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(d) = path.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let saved = if self.doc_view.is_some() && w > 1600 {
                // pictures for the guide: smaller files
                let full = image::RgbaImage::from_raw(w as u32, h as u32, bytes.clone()).unwrap_or_default();
                let nh = (h as f32 * 1600.0 / w as f32) as u32;
                image::imageops::resize(&full, 1600, nh, image::imageops::FilterType::Lanczos3).save(&path)
            } else {
                image::save_buffer(&path, &bytes, w as u32, h as u32, image::ColorType::Rgba8)
            };
            match saved {
                Ok(()) => println!("Screenshot gespeichert: {}", path.display()),
                Err(e) => eprintln!("Screenshot fehlgeschlagen: {e}"),
            }
            if self.demo {
                let _ = std::fs::remove_dir_all(&self.data);
            }
            std::process::exit(0);
        }
        if self.frames > 600 {
            eprintln!("Screenshot kam nicht an");
            std::process::exit(1);
        }
    }
}

impl eframe::App for HmbApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_tray(ctx);
        self.handle_drops(ctx);
        self.poll_pick(ctx);
        self.poll_sync();
        self.watch_files(ctx);
        self.shortcuts(ctx);
        self.close_handling(ctx);
        self.top_bar(ctx);
        self.status_bar(ctx);
        self.tag_panel(ctx);
        self.list_panel(ctx);
        self.editor_panel(ctx);
        self.windows(ctx);
        self.drop_overlay(ctx);
        self.autosave(ctx);
        self.screenshot_step(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_current();
    }
}
