//! Small windows: delete confirmation, "what's new" after sync, settings, tag ontology, snippets.

use super::widgets::{mtime, short};
use super::*;
use crate::service;
use eframe::egui::{CornerRadius, Margin, RichText, Stroke, Vec2};

impl HmbApp {
    pub(super) fn windows(&mut self, ctx: &egui::Context) {
        if self.confirm_delete {
            self.delete_window(ctx);
        }
        if self.news.is_some() {
            self.news_window(ctx);
        }
        if self.show_settings {
            self.settings_window(ctx);
        }
        if self.show_onto {
            self.onto_window(ctx);
        }
        if self.show_snippets {
            self.snippets_window(ctx);
        }
    }

    fn delete_window(&mut self, ctx: &egui::Context) {
        let title = self.editor.as_ref().map(|e| e.a.title.clone()).unwrap_or_default();
        let mut close = false;
        let mut del = false;
        egui::Window::new("Eintrag löschen?")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(format!("„{title}“ wird gelöscht. Das lässt sich nicht rückgängig machen."));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let b = egui::Button::new(RichText::new("Löschen").color(Color32::WHITE)).fill(DANGER);
                    if ui.add(b).clicked() {
                        del = true;
                    }
                    if ui.button("Abbrechen").clicked() {
                        close = true;
                    }
                });
            });
        if del {
            self.confirm_delete = false;
            self.delete_current();
        } else if close {
            self.confirm_delete = false;
        }
    }

    fn news_window(&mut self, ctx: &egui::Context) {
        let news = self.news.clone().unwrap_or_default();
        let mut open = true;
        let mut goto = None;
        egui::Window::new("Neu seit der letzten Synchronisierung")
            .open(&mut open)
            .default_width(460.0)
            .collapsible(false)
            .show(ctx, |ui| {
                if news.is_empty() {
                    ui.label(RichText::new("Von anderen Rechnern kam nichts Neues.").color(MUTED));
                }
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    for e in &news {
                        ui.horizontal(|ui| {
                            let (icon, c) = match e.action.as_str() {
                                "created" => ("➕ neu", OK),
                                "deleted" => ("🗑 gelöscht", DANGER),
                                _ => ("✏ geändert", ACCENT),
                            };
                            ui.label(RichText::new(icon).small().color(c));
                            let t = if e.title.is_empty() { e.id.clone() } else { e.title.clone() };
                            if e.action != "deleted" {
                                if ui.link(t).clicked() {
                                    goto = Some(e.id.clone());
                                }
                            } else {
                                ui.label(t);
                            }
                            let when = e.time.get(..16).unwrap_or(&e.time).replace('T', " ");
                            ui.label(RichText::new(format!("{} · {}", e.client, when)).small().color(MUTED));
                        });
                    }
                });
            });
        if let Some(id) = goto {
            self.open_by_id(&id);
        }
        if !open {
            self.news = None;
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let mut apply = false;
        let mut install = false;
        let mut reveal = false;
        let mut full_sync = false;
        let mut cleanup = false;
        egui::Window::new("Einstellungen")
            .open(&mut open)
            .default_width(520.0)
            .collapsible(false)
            .show(ctx, |ui| {
                let d = &mut self.settings_draft;
                egui::Grid::new("settings").num_columns(2).spacing([12.0, 10.0]).show(ui, |ui| {
                    ui.label("Datenordner");
                    ui.horizontal(|ui| {
                        let mut s = d.data_dir.display().to_string();
                        if ui.add(egui::TextEdit::singleline(&mut s).desired_width(300.0)).changed() {
                            d.data_dir = PathBuf::from(s);
                        }
                        if ui.button("Wählen …").clicked() {
                            if let Some(p) = rfd::FileDialog::new().set_directory(&d.data_dir).pick_folder() {
                                d.data_dir = p;
                            }
                        }
                    });
                    ui.end_row();
                    ui.label("Content-Ordner");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut d.content_subdir)
                                .hint_text("leer = Datenordner selbst")
                                .desired_width(160.0),
                        );
                        ui.label(RichText::new("innerhalb des Datenordners").small().color(MUTED));
                    });
                    ui.end_row();

                    ui.label("Ordnerstruktur");
                    ui.vertical(|ui| {
                        ui.radio_value(&mut d.layout, store::Layout::Dated, "Nach Datum:  JJJJ/MM/TT/<name>/index.md");
                        ui.radio_value(
                            &mut d.layout,
                            store::Layout::Sections,
                            "Nach Bereichen (Hugo):  <bereich>/<unterbereich>/<name>/index.md",
                        );
                        let hint = match d.layout {
                            store::Layout::Dated => "Abstract steht im Frontmatter (abstract:).",
                            store::Layout::Sections => {
                                "Abstract steht vor <!--more-->, weitere Frontmatter-Felder (categories, author …) bleiben erhalten."
                            }
                        };
                        ui.label(RichText::new(hint).small().color(MUTED));
                    });
                    ui.end_row();

                    if d.layout == store::Layout::Sections {
                        ui.label("Bereich für neue Einträge");
                        ui.add(
                            egui::TextEdit::singleline(&mut d.default_section)
                                .hint_text("z. B. methodik/writing")
                                .desired_width(220.0),
                        );
                        ui.end_row();
                    }

                    ui.label("Bilder einfügen als");
                    ui.vertical(|ui| {
                        ui.radio_value(
                            &mut d.image_style,
                            store::ImageStyle::Figure,
                            "Hugo-figure:  {{< figure src=\"bild.png\" width=\"600\" alt=\"…\" >}}",
                        );
                        ui.radio_value(&mut d.image_style, store::ImageStyle::Markdown, "Markdown:  ![…](bild.png)");
                        if d.image_style == store::ImageStyle::Figure {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Breite").small().color(MUTED));
                                ui.add(egui::DragValue::new(&mut d.image_width).range(50..=2000).suffix(" px"));
                            });
                        }
                    });
                    ui.end_row();

                    ui.label("Autor (optional)");
                    ui.add(egui::TextEdit::singleline(&mut d.author).desired_width(220.0));
                    ui.end_row();

                    ui.label("Sync-Ordner");
                    ui.horizontal(|ui| {
                        let mut s = d.sync_dir.display().to_string();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut s)
                                    .hint_text("z. B. der content-Ordner des Hugo-Blogs")
                                    .desired_width(300.0),
                            )
                            .changed()
                        {
                            d.sync_dir = PathBuf::from(s);
                        }
                        if ui.button("Wählen …").clicked() {
                            let mut dlg = rfd::FileDialog::new();
                            if d.sync_dir.is_dir() {
                                dlg = dlg.set_directory(&d.sync_dir);
                            }
                            if let Some(p) = dlg.pick_folder() {
                                d.sync_dir = p;
                            }
                        }
                    });
                    ui.end_row();
                    if !d.sync_dir.as_os_str().is_empty() && d.sync_dir.is_dir() {
                        ui.label("");
                        match store::hugo_site_of(&d.sync_dir) {
                            Some((site, file)) => ui.label(
                                RichText::new(format!(
                                    "✔ Gehört zur Hugo-Seite {} ({file}) – Artikel landen direkt im Blog, Git und Hugo erledigen den Rest.",
                                    site.display()
                                ))
                                .small()
                                .color(OK),
                            ),
                            None => ui.label(
                                RichText::new("Keine Hugo-Seite darüber gefunden (hugo.toml). Für den Blog den content-Ordner der Hugo-Seite wählen.")
                                    .small()
                                    .color(MUTED),
                            ),
                        };
                        ui.end_row();
                    }
                    ui.label("Name dieses Rechners");
                    ui.add(egui::TextEdit::singleline(&mut d.client).desired_width(160.0));
                    ui.end_row();
                    ui.label("");
                    ui.label(
                        RichText::new("Steht in den Transaktionsdateien, damit mehrere Rechner sauber abgleichen.")
                            .small()
                            .color(MUTED),
                    );
                    ui.end_row();
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new(
                        "„Synchronisieren“ liest nur die neuen Transaktionsdateien (transaction-JJJJ-MM-TT-HH-mm.json) im Ordner „transactions“ neben dem Sync-Ordner – nie darin, damit Hugo sie nicht für Inhalt hält –, übernimmt die dort genannten Ordner und schreibt die eigenen Änderungen samt neuer Transaktionsdatei hinein. Git & Co. kümmern sich um den Sync-Ordner selbst.",
                    )
                    .small()
                    .color(MUTED),
                );
                let last = if self.cfg.last_tx.is_empty() {
                    "noch keine".to_string()
                } else {
                    format!("{} ({} Datei(en))", self.cfg.last_tx, self.cfg.last_tx_files.len())
                };
                ui.label(RichText::new(format!("Zuletzt verarbeitete Transaktion: {last}")).small().color(MUTED));
                ui.horizontal(|ui| {
                    if ui
                        .button("⟲  Vollsynchronisierung")
                        .on_hover_text(
                            "Vergleicht alle Artikel in beiden Ordnern: Fehlendes wird geholt bzw. geschrieben, bei Unterschieden gewinnt die neuere Fassung.",
                        )
                        .clicked()
                    {
                        full_sync = true;
                    }
                    ui.label(RichText::new("durchsucht alle Artikel in beiden Ordnern").small().color(MUTED));
                });
                // problems with the chosen folders, shown for what is typed right now
                let mut check = self.settings_draft.clone();
                check.last_tx.clear();
                if !check.sync_dir.as_os_str().is_empty() {
                    if let Some(problem) = sync::folder_problem(&check) {
                        ui.add_space(4.0);
                        egui::Frame::new()
                            .fill(Color32::from_rgb(254, 242, 242))
                            .stroke(Stroke::new(1.0, Color32::from_rgb(254, 202, 202)))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                ui.label(RichText::new(format!("⚠ {problem}")).color(DANGER));
                                let own = store::Config::default().data_dir;
                                let label = format!("Eigenen Datenordner verwenden ({})", own.display());
                                if ui.button(label).clicked() {
                                    self.settings_draft.data_dir = own;
                                    self.settings_draft.content_subdir = store::CONTENT_DIR.into();
                                }
                                ui.label(
                                    RichText::new("Danach „Speichern“ und „Vollsynchronisierung“: alle Blog-Artikel kommen in die App und umgekehrt.")
                                        .small()
                                        .color(MUTED),
                                );
                            });
                    }
                }
                if !self.dupes.is_empty() {
                    ui.add_space(4.0);
                    egui::Frame::new()
                        .fill(Color32::from_rgb(255, 251, 235))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(253, 230, 138)))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(Margin::same(8))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "⚠ {} Artikel-Ordner liegen eine Ebene zu tief (content/content/…) – eine frühere Version hat beim Abgleich Ordner in sich selbst kopiert. Daher erscheinen Einträge doppelt.",
                                    self.dupes.len()
                                ))
                                .color(WARN),
                            );
                            egui::CollapsingHeader::new("Welche Ordner?").id_salt("dupes").show(ui, |ui| {
                                for d in self.dupes.iter().take(60) {
                                    ui.label(RichText::new(d.display().to_string()).small().monospace().color(MUTED));
                                }
                            });
                            if !self.confirm_cleanup {
                                if ui.button("🧹  Doppelte Ordner reparieren …").clicked() {
                                    self.confirm_cleanup = true;
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    let b = egui::Button::new(RichText::new("Jetzt reparieren").color(Color32::WHITE)).fill(DANGER);
                                    if ui.add(b).clicked() {
                                        cleanup = true;
                                    }
                                    if ui.button("Abbrechen").clicked() {
                                        self.confirm_cleanup = false;
                                    }
                                });
                                ui.label(RichText::new("Exakte Kopien werden gelöscht. Fehlt das Original, wird die Kopie an die richtige Stelle zurückgelegt. Abweichende Fassungen bleiben unangetastet.").small().color(MUTED));
                            }
                        });
                }
                ui.separator();
                ui.label(RichText::new("Rechtsklick-Menü (macOS)").strong());
                let txt = format!(
                    "Richtet den Dienst „{}“ ein: Text irgendwo markieren → Rechtsklick → Dienste.",
                    service::SERVICE_NAME
                );
                ui.label(RichText::new(txt).small().color(MUTED));
                let (state, color) = match service::status() {
                    service::Status::Ok => ("✔ eingerichtet", OK),
                    service::Status::OtherProgram => ("⚠ zeigt auf eine andere Programmdatei – bitte neu einrichten", WARN),
                    service::Status::NotInstalled => ("noch nicht eingerichtet", MUTED),
                };
                ui.horizontal(|ui| {
                    let label = if service::status() == service::Status::NotInstalled {
                        "Rechtsklick-Dienst einrichten"
                    } else {
                        "Neu einrichten"
                    };
                    if ui.button(label).clicked() {
                        install = true;
                    }
                    ui.label(RichText::new(state).small().color(color));
                });
                let old = service::old_entries();
                if old > 0 {
                    ui.label(
                        RichText::new(format!(
                            "⚠ {old} alter Eintrag „Als Microblog-Snippet speichern“ (vor der Umbenennung) funktioniert nicht mehr – „Neu einrichten“ entfernt ihn."
                        ))
                        .small()
                        .color(WARN),
                    );
                }
                ui.separator();
                ui.horizontal(|ui| {
                    let b = egui::Button::new(RichText::new("Speichern").color(Color32::WHITE)).fill(ACCENT);
                    if ui.add(b).clicked() {
                        apply = true;
                    }
                    if ui.button("Datenordner im Finder zeigen").clicked() {
                        reveal = true;
                    }
                });
            });
        if reveal {
            let _ = std::fs::create_dir_all(&self.data);
            ctx.open_url(egui::OpenUrl::same_tab(format!("file://{}", self.data.display())));
        }
        if install {
            match service::install() {
                Ok(p) => self.set_status(format!("Rechtsklick-Dienst eingerichtet: {}", p.display()), true),
                Err(e) => self.set_status(format!("Dienst nicht eingerichtet: {e}"), false),
            }
        }
        if apply {
            self.apply_settings();
            open = false;
        }
        if cleanup {
            self.confirm_cleanup = false;
            match sync::repair_nested(&self.cfg) {
                Ok(r) => {
                    self.editor = None;
                    self.reload();
                    let mut msg = format!(
                        "Repariert: {} Kopien entfernt, {} Artikel an ihren Platz zurückgelegt.",
                        r.removed,
                        r.restored.len()
                    );
                    if !r.conflicts.is_empty() {
                        msg.push_str(&format!(" {} abweichende Fassung(en) bitte selbst prüfen: {}", r.conflicts.len(), r.conflicts.join(", ")));
                    }
                    self.set_status(msg, r.conflicts.is_empty());
                }
                Err(e) => self.set_status(format!("Aufräumen fehlgeschlagen: {e}"), false),
            }
            self.dupes = sync::nested_copies(&self.cfg);
        }
        if full_sync {
            // use what is typed in the settings, then compare everything
            self.apply_settings();
            self.start_sync(ctx, true);
        }
        self.show_settings = open;
    }

    fn onto_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let tags: Vec<String> = self.all_tags().into_iter().collect();
        let mut change: Option<(String, Option<String>)> = None;
        egui::Window::new("Tags ordnen")
            .open(&mut open)
            .default_width(420.0)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Lege für jeden Tag einen Oberbegriff fest. Tags ohne Oberbegriff erscheinen links als Haupttags; die Suche nach einem Oberbegriff findet auch alle Unterbegriffe.",
                    )
                    .small()
                    .color(MUTED),
                );
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    egui::Grid::new("onto")
                        .num_columns(2)
                        .striped(true)
                        .spacing([16.0, 6.0])
                        .show(ui, |ui| {
                            for t in &tags {
                                ui.label(RichText::new(format!("#{t}")).color(TAGFG));
                                let cur = self.onto.parent(t).map(|s| s.to_string());
                                let label = cur
                                    .clone()
                                    .map(|p| format!("unter #{p}"))
                                    .unwrap_or_else(|| "— Haupttag —".into());
                                egui::ComboBox::from_id_salt(("parent", t.as_str()))
                                    .selected_text(label)
                                    .width(180.0)
                                    .show_ui(ui, |ui| {
                                        if ui.selectable_label(cur.is_none(), "— Haupttag —").clicked() {
                                            change = Some((t.clone(), None));
                                        }
                                        for p in &tags {
                                            if p == t || self.onto.would_cycle(t, p) {
                                                continue;
                                            }
                                            let sel = cur.as_deref() == Some(p.as_str());
                                            if ui.selectable_label(sel, format!("#{p}")).clicked() {
                                                change = Some((t.clone(), Some(p.clone())));
                                            }
                                        }
                                    });
                                ui.end_row();
                            }
                        });
                });
            });
        if let Some((t, p)) = change {
            match p {
                Some(p) => {
                    self.onto.parents.insert(t, p.clone());
                    self.expanded.insert(p);
                }
                None => {
                    self.onto.parents.remove(&t);
                }
            }
            match self.onto.save(&self.cfg.meta_dir()) {
                Ok(()) => self.set_status("Tag-Ordnung gespeichert.", true),
                Err(e) => self.set_status(format!("Tag-Ordnung nicht gespeichert: {e}"), false),
            }
        }
        self.show_onto = open;
    }

    fn snippets_window(&mut self, ctx: &egui::Context) {
        let mut open = true;
        let mut insert = None;
        let mut remove = None;
        let mut add = false;
        egui::Window::new("Snippets")
            .open(&mut open)
            .default_width(420.0)
            .default_pos(ctx.screen_rect().right_top() + Vec2::new(-460.0, 70.0))
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Kleine Textbausteine. Neue kommen über Rechtsklick → Dienste → „Als hmb-Snippet speichern“ oder hier.",
                    )
                    .small()
                    .color(MUTED),
                );
                ui.add(
                    egui::TextEdit::multiline(&mut self.new_snippet)
                        .hint_text("Neues Snippet …")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                if ui.button("➕ Hinzufügen").clicked() {
                    add = true;
                }
                ui.separator();
                egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                    if self.snippets.is_empty() {
                        ui.label(RichText::new("Noch keine Snippets.").color(MUTED));
                    }
                    for (i, s) in self.snippets.iter().enumerate() {
                        egui::Frame::new()
                            .fill(LISTBG)
                            .stroke(Stroke::new(1.0, LINE))
                            .corner_radius(CornerRadius::same(6))
                            .inner_margin(Margin::same(8))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.label(short(&s.text, 220));
                                ui.horizontal(|ui| {
                                    let when = s.created.get(..16).unwrap_or(&s.created).replace('T', " ");
                                    ui.label(RichText::new(format!("{} · {}", s.source, when)).small().color(MUTED));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("🗑").on_hover_text("Löschen").clicked() {
                                            remove = Some(i);
                                        }
                                        if ui.small_button("Kopieren").clicked() {
                                            ui.ctx().copy_text(s.text.clone());
                                        }
                                        if ui.small_button("Einfügen").clicked() {
                                            insert = Some(s.text.clone());
                                        }
                                    });
                                });
                            });
                        ui.add_space(4.0);
                    }
                });
            });
        if add && !self.new_snippet.trim().is_empty() {
            match store::add_snippet(&self.cfg.meta_dir(), &self.new_snippet, "App") {
                Ok(()) => {
                    self.new_snippet.clear();
                    self.snippets = store::load_snippets(&self.cfg.meta_dir());
                    self.snippets_mtime = mtime(&store::snippets_path(&self.cfg.meta_dir()));
                }
                Err(e) => self.set_status(format!("Snippet nicht gespeichert: {e}"), false),
            }
        }
        if let Some(i) = remove {
            self.snippets.remove(i);
            let _ = store::save_snippets(&self.cfg.meta_dir(), &self.snippets);
            self.snippets_mtime = mtime(&store::snippets_path(&self.cfg.meta_dir()));
        }
        if let Some(t) = insert {
            self.insert_snippet(ctx, &t);
        }
        self.show_snippets = open;
    }
}
