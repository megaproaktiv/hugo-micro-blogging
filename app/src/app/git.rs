//! Git window (button "Git" in the top bar): Pull, then Commit with a prepared message
//! (title of the last changed entry + what was created, changed, deleted), then Push.
//! Git runs in a thread; its output is shown in the window.

use super::*;
use crate::gitops::{self, Kind};
use eframe::egui::{CornerRadius, Margin, RichText, Stroke, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    Pull,
    Commit,
    Push,
}

impl Step {
    fn label(self) -> &'static str {
        match self {
            Step::Pull => "git pull",
            Step::Commit => "git commit",
            Step::Push => "git push",
        }
    }
}

pub struct LogLine {
    pub cmd: String,
    pub out: String,
    pub ok: bool,
}

#[derive(Default)]
pub struct GitState {
    pub open: bool,
    pub repo: Option<PathBuf>,
    pub repo_err: Option<String>,
    pub status: gitops::Status,
    pub message: String,
    pub message_edited: bool,
    pub log: Vec<LogLine>,
    pub running: Option<(Step, Receiver<Result<String, String>>)>,
    pub pulled: bool,
}

impl GitState {
    pub fn busy(&self) -> bool {
        self.running.is_some()
    }
}

impl HmbApp {
    pub(super) fn open_git(&mut self) {
        self.save_current();
        self.git.open = true;
        self.git_refresh();
    }

    /// Reads repository, branch and changes again; prepares the commit message.
    fn git_refresh(&mut self) {
        match gitops::repo_for(&self.cfg) {
            Ok(repo) => {
                let scope = gitops::scope(&self.cfg, &repo);
                match gitops::status(&repo, &scope) {
                    Ok(st) => {
                        if !self.git.message_edited {
                            self.git.message = gitops::commit_message(&st);
                        }
                        self.git.status = st;
                        self.git.repo_err = None;
                    }
                    Err(e) => self.git.repo_err = Some(e),
                }
                self.git.repo = Some(repo);
            }
            Err(e) => {
                self.git.repo = None;
                self.git.repo_err = Some(e);
            }
        }
    }

    fn git_start(&mut self, ctx: &egui::Context, step: Step) {
        let Some(repo) = self.git.repo.clone() else { return };
        if self.git.busy() {
            return;
        }
        let scope = gitops::scope(&self.cfg, &repo);
        let message = self.git.message.trim().to_string();
        let (tx, rx) = channel();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let r = match step {
                Step::Pull => gitops::pull(&repo),
                Step::Commit => gitops::commit(&repo, &scope, &message),
                Step::Push => gitops::push(&repo),
            };
            let _ = tx.send(r);
            ctx2.request_repaint();
        });
        self.git.running = Some((step, rx));
        self.set_status(format!("{} läuft …", step.label()), true);
    }

    /// Called every frame: takes the result of a finished git command.
    pub(super) fn poll_git(&mut self) {
        let Some((step, rx)) = &self.git.running else { return };
        let step = *step;
        let res = match rx.try_recv() {
            Ok(r) => r,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(_) => Err("abgebrochen".into()),
        };
        self.git.running = None;
        let ok = res.is_ok();
        let out = match &res {
            Ok(s) if s.trim().is_empty() => "fertig".to_string(),
            Ok(s) | Err(s) => s.clone(),
        };
        self.git.log.push(LogLine { cmd: step.label().into(), out: out.clone(), ok });
        match (step, ok) {
            (Step::Pull, true) => {
                self.git.pulled = true;
                self.set_status("git pull fertig – jetzt synchronisieren, um neue Artikel in die App zu holen.", true);
            }
            (Step::Commit, true) => {
                self.git.message_edited = false;
                self.set_status("Commit gespeichert – jetzt pushen.", true);
            }
            (Step::Push, true) => {
                let last = out.lines().last().unwrap_or("").trim().to_string();
                self.set_status(format!("Gepusht. {last}"), true);
            }
            (_, false) => {
                let first = out.lines().next().unwrap_or("").to_string();
                self.set_status(format!("{} fehlgeschlagen: {first}", step.label()), false);
            }
        }
        self.git_refresh();
    }

    pub(super) fn git_window(&mut self, ctx: &egui::Context) {
        if !self.git.open {
            return;
        }
        let mut open = true;
        let mut action: Option<Step> = None;
        let mut refresh = false;
        let mut sync_now = false;
        let mut reset_msg = false;
        let mut scope_changed = false;
        let busy = self.git.busy();
        let running = self.git.running.as_ref().map(|(s, _)| *s);
        let title = format!("Git – {}", self.cfg.title);
        egui::Window::new(title)
            .id(egui::Id::new("git_window"))
            .open(&mut open)
            .default_width(640.0)
            .default_height(560.0)
            .collapsible(false)
            .show(ctx, |ui| {
                // repository and branch
                match &self.git.repo {
                    Some(repo) => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Repository").color(MUTED));
                            ui.label(RichText::new(widgets::show_path(repo)).monospace().color(INK));
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Branch").color(MUTED));
                            ui.label(RichText::new(&self.git.status.branch).monospace().color(INK));
                            if ui.small_button("⟳").on_hover_text("Neu einlesen").clicked() {
                                refresh = true;
                            }
                        });
                        if ui
                            .checkbox(&mut self.cfg.git_blog_only, "Nur Blog-Inhalte (content, transactions, Snippets, Tags)")
                            .on_hover_text("Ausgeschaltet: alle Änderungen im Repository werden committet")
                            .changed()
                        {
                            scope_changed = true;
                        }
                    }
                    None => {
                        ui.label(
                            RichText::new(self.git.repo_err.clone().unwrap_or_default()).color(DANGER),
                        );
                        return;
                    }
                }
                if let Some(e) = &self.git.repo_err {
                    ui.label(RichText::new(e).small().color(DANGER));
                }
                ui.separator();

                // 1) pull
                ui.horizontal(|ui| {
                    let b = egui::Button::new(RichText::new("⬇  Pull").strong()).min_size(Vec2::new(110.0, 28.0));
                    if ui.add_enabled(!busy, b).on_hover_text("git pull --rebase --autostash").clicked() {
                        action = Some(Step::Pull);
                    }
                    if running == Some(Step::Pull) {
                        ui.spinner();
                    }
                    let behind = self.git.status.behind;
                    let hint = if behind > 0 {
                        format!("{behind} neue Commit(s) auf dem Server")
                    } else {
                        "Neues vom Server holen".into()
                    };
                    ui.label(RichText::new(hint).small().color(MUTED));
                    if self.git.pulled {
                        if ui
                            .add_enabled(self.sync_rx.is_none(), egui::Button::new("🔄  Jetzt synchronisieren"))
                            .on_hover_text("Neue Artikel aus dem Blog in die App holen")
                            .clicked()
                        {
                            sync_now = true;
                        }
                    }
                });
                ui.add_space(6.0);

                // 2) changes + commit
                let st = &self.git.status;
                let n = st.changes.len() + st.other.len();
                ui.label(
                    RichText::new(format!("Änderungen seit dem letzten Commit ({})", st.changes.len()))
                        .strong()
                        .color(INK),
                );
                egui::Frame::new()
                    .fill(LISTBG)
                    .stroke(Stroke::new(1.0, LINE))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        egui::ScrollArea::vertical().id_salt("git_changes").max_height(140.0).show(ui, |ui| {
                            if st.is_clean() {
                                ui.label(RichText::new("Nichts zu committen – alles gespeichert.").color(MUTED));
                            }
                            for c in &st.changes {
                                ui.horizontal(|ui| {
                                    let (txt, col) = match c.kind {
                                        Kind::Created => ("+ neu", OK),
                                        Kind::Changed => ("✏ geändert", ACCENT),
                                        Kind::Deleted => ("✖ gelöscht", DANGER),
                                    };
                                    ui.label(RichText::new(txt).small().color(col));
                                    ui.label(RichText::new(&c.title).color(INK));
                                    ui.label(RichText::new(&c.dir).small().monospace().color(MUTED));
                                });
                            }
                            if !st.other.is_empty() {
                                ui.label(
                                    RichText::new(format!(
                                        "+ {} weitere Datei(en): Transaktionen, Snippets, Tags",
                                        st.other.len()
                                    ))
                                    .small()
                                    .color(MUTED),
                                );
                            }
                        });
                    });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Commit-Nachricht").color(MUTED));
                    if self.git.message_edited && ui.small_button("Vorschlag wiederherstellen").clicked() {
                        reset_msg = true;
                    }
                });
                let r = ui.add(
                    egui::TextEdit::multiline(&mut self.git.message)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(5)
                        .desired_width(f32::INFINITY),
                );
                if r.changed() {
                    self.git.message_edited = true;
                }
                ui.horizontal(|ui| {
                    let can = !busy && n > 0 && !self.git.message.trim().is_empty();
                    let b = egui::Button::new(RichText::new("✔  Commit").strong().color(Color32::WHITE))
                        .fill(if can { ACCENT } else { MUTED })
                        .min_size(Vec2::new(110.0, 28.0));
                    if ui.add_enabled(can, b).clicked() {
                        action = Some(Step::Commit);
                    }
                    if running == Some(Step::Commit) {
                        ui.spinner();
                    }
                    ui.label(
                        RichText::new("Erste Zeile: zuletzt geänderter Eintrag; darunter neu / geändert / gelöscht")
                            .small()
                            .color(MUTED),
                    );
                });
                ui.add_space(6.0);

                // 3) push
                ui.horizontal(|ui| {
                    let ahead = self.git.status.ahead;
                    let b = egui::Button::new(RichText::new("⬆  Push").strong()).min_size(Vec2::new(110.0, 28.0));
                    if ui.add_enabled(!busy, b).on_hover_text("git push").clicked() {
                        action = Some(Step::Push);
                    }
                    if running == Some(Step::Push) {
                        ui.spinner();
                    }
                    let hint = if ahead > 0 {
                        format!("{ahead} Commit(s) noch nicht auf dem Server")
                    } else {
                        "Alles auf dem Server".into()
                    };
                    ui.label(RichText::new(hint).small().color(if ahead > 0 { WARN } else { MUTED }));
                });
                ui.separator();

                // output of git
                ui.label(RichText::new("Ausgabe").color(MUTED));
                egui::ScrollArea::vertical()
                    .id_salt("git_log")
                    .max_height(160.0)
                    .stick_to_bottom(true)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if self.git.log.is_empty() {
                            ui.label(RichText::new("Noch nichts ausgeführt.").small().color(MUTED));
                        }
                        for l in &self.git.log {
                            let (mark, col) = if l.ok { ("✔", OK) } else { ("✖", DANGER) };
                            ui.label(RichText::new(format!("{mark} $ {}", l.cmd)).monospace().color(col));
                            ui.label(RichText::new(&l.out).small().monospace().color(if l.ok { INK } else { DANGER }));
                            ui.add_space(4.0);
                        }
                    });
            });
        if scope_changed {
            let _ = self.persist();
            refresh = true;
        }
        if reset_msg {
            self.git.message_edited = false;
            refresh = true;
        }
        if refresh {
            self.git_refresh();
        }
        if let Some(step) = action {
            self.git_start(ctx, step);
        }
        if sync_now {
            self.git.pulled = false;
            self.start_sync(ctx, false);
        }
        if !open {
            self.git.open = false;
        }
    }
}
