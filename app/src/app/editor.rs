//! The editor: title, frontmatter (date, tags, draft), abstract, Markdown body with preview.

use super::widgets::{byte_idx, set_body_cursor, short};
use super::*;
use eframe::egui::{CornerRadius, FontId, Margin, RichText, Stroke, TextStyle, Vec2};
use egui_commonmark::CommonMarkViewer;

type Fmt = (&'static str, &'static str, &'static str);

impl HmbApp {
    /// Wraps the current selection in the body with `before`/`after`
    /// (or inserts `placeholder` between them when nothing is selected).
    pub(super) fn insert_text(&mut self, ctx: &egui::Context, before: &str, after: &str, placeholder: &str) {
        let Some(ed) = &mut self.editor else { return };
        let body = &ed.a.body;
        let n = body.chars().count();
        let (s, e) = ed.cursor.unwrap_or((n, n));
        let (s, e) = (s.min(n), e.min(n));
        let (s, e) = (s.min(e), s.max(e));
        let bs = byte_idx(body, s);
        let be = byte_idx(body, e);
        let selected = &body[bs..be];
        let inner = if selected.is_empty() { placeholder } else { selected };
        let new_body = format!("{}{}{}{}{}", &body[..bs], before, inner, after, &body[be..]);
        let sel_start = s + before.chars().count();
        let sel_end = sel_start + inner.chars().count();
        ed.a.body = new_body;
        ed.cursor = Some((sel_start, sel_end));
        ed.last_change = ctx.input(|i| i.time);
        set_body_cursor(ctx, sel_start, sel_end);
        self.focus_body = true;
    }

    /// Puts `prefix` at the start of the line where the cursor is.
    fn insert_line_prefix(&mut self, ctx: &egui::Context, prefix: &str) {
        let Some(ed) = &mut self.editor else { return };
        let body = &ed.a.body;
        let n = body.chars().count();
        let s = ed.cursor.map(|c| c.0).unwrap_or(n).min(n);
        let bs = byte_idx(body, s);
        let line_start = body[..bs].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let new_body = format!("{}{}{}", &body[..line_start], prefix, &body[line_start..]);
        let c = s + prefix.chars().count();
        ed.a.body = new_body;
        ed.cursor = Some((c, c));
        ed.last_change = ctx.input(|i| i.time);
        set_body_cursor(ctx, c, c);
        self.focus_body = true;
    }

    pub(super) fn insert_snippet(&mut self, ctx: &egui::Context, text: &str) {
        if self.editor.is_none() {
            self.new_entry();
        }
        let t = format!("{text}\n");
        self.insert_text(ctx, &t, "", "");
        self.set_status("Snippet eingefügt.", true);
    }

    pub(super) fn editor_panel(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::WHITE).inner_margin(Margin::symmetric(18, 14)))
            .show(ctx, |ui| {
                if self.editor.is_none() {
                    ui.add_space(ui.available_height() * 0.35);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Kein Eintrag geöffnet").size(18.0).color(INK));
                        ui.label(
                            RichText::new("Wähle links einen Eintrag oder lege einen neuen an.").color(MUTED),
                        );
                        ui.add_space(10.0);
                        let b = egui::Button::new(RichText::new("➕  Neuer Eintrag").color(Color32::WHITE))
                            .fill(ACCENT);
                        if ui.add(b).clicked() {
                            self.new_entry();
                        }
                    });
                    return;
                }
                self.editor_ui(ui, ctx);
            });
    }

    fn editor_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let all_tags = self.all_tags();
        let snippets = self.snippets.clone();
        let mut changed = false;
        let mut action_save = false;
        let mut action_delete = false;
        let mut fmt: Option<Fmt> = None;
        let mut prefix: Option<&'static str> = None;
        let mut snippet_insert: Option<String> = None;
        let mut pick_image = false;
        let layout = self.cfg.layout;
        let sections = store::known_sections(&self.cfg, &self.articles);
        let data = self.data.clone();
        let cfg_section = |a: &Article| a.section(&self.cfg);
        let current_section = self.editor.as_ref().map(|e| cfg_section(&e.a)).unwrap_or_default();

        // bottom bar of the editor
        egui::TopBottomPanel::bottom("editor_bottom")
            .frame(egui::Frame::new().inner_margin(Margin { left: 0, right: 0, top: 8, bottom: 0 }))
            .show_separator_line(false)
            .show_inside(ui, |ui| {
                let ed = self.editor.as_ref().unwrap();
                ui.horizontal(|ui| {
                    let save = egui::Button::new(RichText::new("💾  Speichern").color(Color32::WHITE)).fill(ACCENT);
                    if ui.add(save).on_hover_text("⌘S – wird auch automatisch gespeichert").clicked() {
                        action_save = true;
                    }
                    if ui.button(RichText::new("🗑  Löschen").color(DANGER)).clicked() {
                        action_delete = true;
                    }
                    if ed.dirty() {
                        ui.label(RichText::new("● Ungespeichert").small().color(WARN));
                    } else {
                        ui.label(RichText::new("✔ Gespeichert").small().color(OK));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let path = if ed.is_new {
                            "Ordner wird beim ersten Speichern angelegt".to_string()
                        } else {
                            format!("{}/index.md", ed.a.rel_dir)
                        };
                        ui.add(egui::Label::new(RichText::new(path).small().monospace().color(MUTED)).truncate());
                    });
                });
            });

        let ed = self.editor.as_mut().unwrap();

        // title
        let r = ui.add(
            egui::TextEdit::singleline(&mut ed.a.title)
                .hint_text("Titel")
                .font(FontId::proportional(22.0))
                .frame(false)
                .desired_width(f32::INFINITY),
        );
        changed |= r.changed();
        ui.add(egui::Separator::default().spacing(6.0));

        // frontmatter
        egui::Grid::new("fm").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label(RichText::new("Datum").color(MUTED));
            ui.horizontal(|ui| {
                let valid = store::parse_date(&ed.date_text).is_some();
                let r = ui.add(
                    egui::TextEdit::singleline(&mut ed.date_text)
                        .desired_width(140.0)
                        .text_color(if valid { INK } else { DANGER })
                        .hint_text("JJJJ-MM-TT HH:MM"),
                );
                if r.changed() {
                    if let Some(d) = store::parse_date(&ed.date_text) {
                        ed.a.date = d;
                        changed = true;
                    }
                }
                if ui.small_button("Jetzt").clicked() {
                    ed.a.date = chrono::Local::now().naive_local();
                    ed.date_text = ed.a.date.format("%Y-%m-%d %H:%M").to_string();
                    changed = true;
                }
                ui.add_space(12.0);
                changed |= ui.checkbox(&mut ed.a.draft, "Entwurf").changed();
                ui.add_space(12.0);
                let id = if ed.is_new { "ID: (neu)".to_string() } else { format!("ID: {}", ed.a.id) };
                ui.label(RichText::new(id).small().monospace().color(MUTED));
            });
            ui.end_row();

            if layout == store::Layout::Sections {
                ui.label(RichText::new("Bereich").color(MUTED));
                ui.horizontal(|ui| {
                    if ed.is_new {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut ed.section)
                                .hint_text("z. B. aws/mock")
                                .desired_width(200.0),
                        );
                        changed |= r.changed();
                        egui::ComboBox::from_id_salt("section_pick")
                            .selected_text("vorhandene …")
                            .width(160.0)
                            .show_ui(ui, |ui| {
                                for s in &sections {
                                    if ui.selectable_label(ed.section == *s, s).clicked() {
                                        ed.section = s.clone();
                                    }
                                }
                            });
                        ui.label(RichText::new("Ordner: Bereich/Titel").small().color(MUTED));
                    } else {
                        let s = if current_section.is_empty() { "—".to_string() } else { current_section.clone() };
                        ui.label(RichText::new(s).monospace().color(INK));
                    }
                });
                ui.end_row();
            }

            ui.label(RichText::new("Tags").color(MUTED));
            ui.horizontal_wrapped(|ui| {
                let mut remove = None;
                for (i, t) in ed.a.tags.iter().enumerate() {
                    let b = egui::Button::new(RichText::new(format!("#{t}  ✖")).small().color(TAGFG))
                        .fill(TAGBG)
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(9));
                    if ui.add(b).on_hover_text("Tag entfernen").clicked() {
                        remove = Some(i);
                    }
                }
                if let Some(i) = remove {
                    ed.a.tags.remove(i);
                    changed = true;
                }
                let r = ui.add(
                    egui::TextEdit::singleline(&mut ed.tag_input)
                        .hint_text("Tag hinzufügen …")
                        .desired_width(130.0),
                );
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                if enter || ed.tag_input.ends_with(',') {
                    for t in ed.tag_input.split(',') {
                        let t = store::normalize_tag(t);
                        if !t.is_empty() && !ed.a.tags.contains(&t) {
                            ed.a.tags.push(t);
                            changed = true;
                        }
                    }
                    ed.tag_input.clear();
                    if enter {
                        r.request_focus();
                    }
                }
                let q = store::normalize_tag(&ed.tag_input);
                if !q.is_empty() {
                    let sugg: Vec<String> = all_tags
                        .iter()
                        .filter(|t| t.contains(q.as_str()) && !ed.a.tags.contains(t))
                        .take(5)
                        .cloned()
                        .collect();
                    for t in sugg {
                        let b = egui::Button::new(RichText::new(format!("+ #{t}")).small().color(MUTED)).frame(false);
                        if ui.add(b).clicked() {
                            ed.a.tags.push(t);
                            ed.tag_input.clear();
                            changed = true;
                        }
                    }
                }
            });
            ui.end_row();
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Abstract").color(MUTED));
            ui.label(RichText::new("– erscheint in der Liste (höchstens 3 Zeilen)").small().color(MUTED));
        });
        let r = ui.add(
            egui::TextEdit::multiline(&mut ed.a.abstract_)
                .hint_text("Kurze Zusammenfassung …")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        changed |= r.changed();
        ui.add_space(8.0);

        // toolbar
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let tb = |ui: &mut egui::Ui, label: RichText, tip: &str| -> bool {
                ui.add(egui::Button::new(label).min_size(Vec2::new(30.0, 24.0)))
                    .on_hover_text(tip)
                    .clicked()
            };
            if tb(ui, RichText::new("B").strong(), "Fett") {
                fmt = Some(("**", "**", "fett"));
            }
            if tb(ui, RichText::new("I").italics(), "Kursiv") {
                fmt = Some(("*", "*", "kursiv"));
            }
            if tb(ui, RichText::new("S").strikethrough(), "Durchgestrichen") {
                fmt = Some(("~~", "~~", "Text"));
            }
            if tb(ui, RichText::new("H2"), "Überschrift") {
                prefix = Some("## ");
            }
            if tb(ui, RichText::new("•"), "Liste") {
                prefix = Some("- ");
            }
            if tb(ui, RichText::new("1."), "Nummerierte Liste") {
                prefix = Some("1. ");
            }
            if tb(ui, RichText::new("“ ”"), "Zitat") {
                prefix = Some("> ");
            }
            if tb(ui, RichText::new("</>").monospace(), "Code") {
                fmt = Some(("`", "`", "code"));
            }
            if tb(ui, RichText::new("🔗"), "Link") {
                fmt = Some(("[", "](https://)", "Linktext"));
            }
            if ui
                .add(egui::Button::new(RichText::new("🖼 Bild")).min_size(Vec2::new(30.0, 24.0)))
                .on_hover_text("Bild hinzufügen – oder einfach ins Fenster ziehen")
                .clicked()
            {
                pick_image = true;
            }
            ui.add_space(6.0);
            ui.menu_button("📋", |ui| {
                if snippets.is_empty() {
                    ui.label(RichText::new("Noch keine Snippets").color(MUTED));
                }
                for s in snippets.iter().take(15) {
                    if ui.button(short(&s.text, 48)).clicked() {
                        snippet_insert = Some(s.text.clone());
                        ui.close();
                    }
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.selectable_value(&mut self.view, ViewMode::Preview, "Vorschau");
                ui.selectable_value(&mut self.view, ViewMode::Split, "Geteilt");
                ui.selectable_value(&mut self.view, ViewMode::Edit, "Bearbeiten");
            });
        });
        ui.add_space(4.0);

        // body editor + preview
        let ed = self.editor.as_mut().unwrap();
        let view = self.view;
        let h = ui.available_height();
        let raw_md = format!(
            "# {}\n\n{}\n\n---\n\n{}",
            if ed.a.title.trim().is_empty() { "Ohne Titel" } else { ed.a.title.trim() },
            ed.a.abstract_.trim(),
            ed.a.body
        );
        let preview_md = store::preview_markdown(&raw_md, &ed.a.dir(&data));
        let md_cache = &mut self.md_cache;
        let focus_body = std::mem::take(&mut self.focus_body);

        let mut body_editor = |ui: &mut egui::Ui, ed: &mut Editor| {
            egui::Frame::new()
                .fill(Color32::from_rgb(252, 252, 253))
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(6))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("bodyscroll")
                        .auto_shrink([false, false])
                        .max_height(h - 14.0)
                        .show(ui, |ui| {
                            let out = egui::TextEdit::multiline(&mut ed.a.body)
                                .id(egui::Id::new("body_editor"))
                                .font(TextStyle::Monospace)
                                .hint_text("Artikel in Markdown schreiben …  (Rechtsklick: Snippets & Formatierung)")
                                .frame(false)
                                .desired_width(f32::INFINITY)
                                .min_size(Vec2::new(0.0, h - 28.0))
                                .show(ui);
                            if let Some(cr) = out.cursor_range {
                                let (a, b) = (cr.primary.index, cr.secondary.index);
                                ed.cursor = Some((a.min(b), a.max(b)));
                            }
                            if focus_body {
                                out.response.request_focus();
                            }
                            if out.response.changed() {
                                changed = true;
                            }
                            out.response.context_menu(|ui| {
                                ui.label(RichText::new("Snippet einfügen").small().strong().color(MUTED));
                                if snippets.is_empty() {
                                    ui.label(RichText::new("Noch keine Snippets").color(MUTED));
                                }
                                for s in snippets.iter().take(12) {
                                    if ui.button(short(&s.text, 48)).clicked() {
                                        snippet_insert = Some(s.text.clone());
                                        ui.close();
                                    }
                                }
                                ui.separator();
                                ui.label(RichText::new("Formatieren").small().strong().color(MUTED));
                                let items: [(&str, Option<Fmt>, Option<&'static str>); 5] = [
                                    ("Fett", Some(("**", "**", "fett")), None),
                                    ("Kursiv", Some(("*", "*", "kursiv")), None),
                                    ("Link", Some(("[", "](https://)", "Linktext")), None),
                                    ("Überschrift", None, Some("## ")),
                                    ("Zitat", None, Some("> ")),
                                ];
                                for (label, f, p) in items {
                                    if ui.button(label).clicked() {
                                        if f.is_some() {
                                            fmt = f;
                                        }
                                        if p.is_some() {
                                            prefix = p;
                                        }
                                        ui.close();
                                    }
                                }
                            });
                        });
                });
        };
        let preview = |ui: &mut egui::Ui, cache: &mut CommonMarkCache| {
            egui::Frame::new()
                .fill(Color32::WHITE)
                .stroke(Stroke::new(1.0, LINE))
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("previewscroll")
                        .auto_shrink([false, false])
                        .max_height(h - 26.0)
                        .show(ui, |ui| {
                            let w = ui.available_width().max(100.0) as usize;
                            CommonMarkViewer::new().max_image_width(Some(w)).show(ui, cache, &preview_md);
                        });
                });
        };

        match view {
            ViewMode::Edit => body_editor(ui, ed),
            ViewMode::Preview => preview(ui, md_cache),
            ViewMode::Split => {
                ui.columns(2, |cols| {
                    body_editor(&mut cols[0], ed);
                    preview(&mut cols[1], md_cache);
                });
            }
        }

        if changed {
            ed.last_change = now;
        }
        if let Some((b, a, p)) = fmt {
            self.insert_text(ctx, b, a, p);
        }
        if let Some(p) = prefix {
            self.insert_line_prefix(ctx, p);
        }
        if let Some(s) = snippet_insert {
            self.insert_snippet(ctx, &s);
        }
        if action_save {
            self.save_current();
        }
        if pick_image {
            self.pick_images(ctx);
        }
        if action_delete {
            self.confirm_delete = true;
        }
    }
}
