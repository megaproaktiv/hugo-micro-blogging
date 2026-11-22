//! Top bar, status bar, tag sidebar and the list of entries.

use super::widgets::{entry_card, tag_chip, tag_row};
use super::*;
use eframe::egui::{CornerRadius, FontId, Margin, RichText, Sense, Stroke, Vec2};

impl HmbApp {
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top")
            .frame(
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(Margin::symmetric(14, 10))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (r, p) = ui.allocate_painter(Vec2::splat(28.0), Sense::hover());
                    let rect = r.rect;
                    p.rect_filled(rect, CornerRadius::same(7), ACCENT);
                    p.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "µ",
                        FontId::proportional(19.0),
                        Color32::WHITE,
                    );
                    ui.label(RichText::new("hmb").size(19.0).strong().color(INK));
                    ui.label(RichText::new("Hugo Microblogger").small().color(MUTED));
                    ui.add_space(24.0);

                    let search = ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("🔍  Nach Tags suchen …  (z. B. rust web)")
                            .desired_width(360.0)
                            .margin(Margin::symmetric(8, 5)),
                    );
                    if search.changed() {
                        self.page = 0;
                    }
                    if !self.search.is_empty()
                        && ui.small_button("✖").on_hover_text("Suche leeren").clicked()
                    {
                        self.search.clear();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(RichText::new("⚙").size(16.0)).frame(false))
                            .on_hover_text("Einstellungen")
                            .clicked()
                        {
                            self.settings_draft = self.cfg.clone();
                            self.dupes = sync::nested_copies(&self.cfg);
                            self.confirm_cleanup = false;
                            self.show_settings = true;
                        }
                        let syncing = self.sync_rx.is_some();
                        let label = if syncing { "Synchronisiere …" } else { "🔄  Synchronisieren" };
                        if ui
                            .add_enabled(!syncing, egui::Button::new(label))
                            .on_hover_text(
                                "Neue Transaktionen aus dem Sync-Ordner holen und eigene Änderungen hineinschreiben",
                            )
                            .clicked()
                        {
                            self.start_sync(ctx, false);
                        }
                        if syncing {
                            ui.spinner();
                        }
                        if ui.add(egui::Button::new(RichText::new("📋  Snippets").color(INK))).clicked() {
                            self.show_snippets = !self.show_snippets;
                        }
                        if ui
                            .add(
                                egui::Button::new(RichText::new("➕  Neu").color(Color32::WHITE).strong())
                                    .fill(ACCENT),
                            )
                            .on_hover_text("Neuer Eintrag (⌘N)")
                            .clicked()
                        {
                            self.new_entry();
                        }
                    });
                });
            });
    }

    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(Color32::WHITE)
                    .inner_margin(Margin::symmetric(12, 5))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let c = if self.status_ok { MUTED } else { DANGER };
                    ui.label(RichText::new(&self.status).small().color(c));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let last = self
                            .cfg
                            .last_sync
                            .as_deref()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                            .map(|d| d.format("%d.%m.%Y %H:%M").to_string())
                            .unwrap_or_else(|| "noch nie".into());
                        ui.label(RichText::new(format!("Zuletzt synchronisiert: {last}")).small().color(MUTED));
                        ui.label(RichText::new("·").small().color(MUTED));
                        ui.label(RichText::new(format!("Rechner: {}", self.cfg.client)).small().color(MUTED));
                    });
                });
            });
    }

    pub(super) fn tag_panel(&mut self, ctx: &egui::Context) {
        let counts = self.tag_counts();
        let mut roots: Vec<String> = self
            .all_tags()
            .into_iter()
            .filter(|t| self.onto.parent(t).is_none())
            .collect();
        roots.sort_by(|a, b| {
            counts
                .get(b)
                .unwrap_or(&0)
                .cmp(counts.get(a).unwrap_or(&0))
                .then(a.cmp(b))
        });

        egui::SidePanel::left("tags")
            .resizable(false)
            .exact_width(200.0)
            .frame(
                egui::Frame::new()
                    .fill(SIDEBAR)
                    .inner_margin(Margin::symmetric(10, 12))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ctx, |ui| {
                ui.label(RichText::new("HAUPTTAGS").small().strong().color(MUTED));
                ui.add_space(6.0);
                let all_sel = self.selected_tag.is_none();
                if tag_row(ui, "Alle Einträge", self.articles.len(), all_sel, 0, None).0 {
                    self.selected_tag = None;
                    self.page = 0;
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .id_salt("tagscroll")
                    .auto_shrink([false, false])
                    .max_height(ui.available_height() - 44.0)
                    .show(ui, |ui| {
                        for t in &roots {
                            self.tag_tree(ui, t, 0, &counts);
                        }
                        if roots.is_empty() {
                            ui.label(RichText::new("Noch keine Tags.").small().color(MUTED));
                        }
                    });
                ui.separator();
                if ui
                    .add(egui::Button::new(RichText::new("⚙  Tags ordnen …").color(INK)).frame(false))
                    .on_hover_text("Über- und Unterbegriffe der Tags festlegen")
                    .clicked()
                {
                    self.show_onto = true;
                }
            });
    }

    fn tag_tree(&mut self, ui: &mut egui::Ui, tag: &str, depth: usize, counts: &BTreeMap<String, usize>) {
        let mut children = self.onto.children(tag);
        children.sort();
        let open = self.expanded.contains(tag);
        let sel = self.selected_tag.as_deref() == Some(tag);
        let (clicked, toggled) = tag_row(
            ui,
            &format!("#{tag}"),
            *counts.get(tag).unwrap_or(&0),
            sel,
            depth,
            if children.is_empty() { None } else { Some(open) },
        );
        if clicked {
            self.selected_tag = if sel { None } else { Some(tag.to_string()) };
            self.page = 0;
        }
        if toggled {
            if open {
                self.expanded.remove(tag);
            } else {
                self.expanded.insert(tag.to_string());
            }
        }
        if open && depth < 8 {
            for c in children {
                self.tag_tree(ui, &c, depth + 1, counts);
            }
        }
    }

    pub(super) fn list_panel(&mut self, ctx: &egui::Context) {
        let items = self.filtered();
        let key = format!("{:?}|{}", self.selected_tag, self.search);
        if key != self.filter_key {
            self.filter_key = key;
            self.page = 0;
        }
        let pages = items.len().div_ceil(PAGE).max(1);
        self.page = self.page.min(pages - 1);
        let current_dir = self.editor.as_ref().map(|e| e.a.rel_dir.clone());
        let mut id_counts: BTreeMap<String, usize> = BTreeMap::new();
        for a in &self.articles {
            *id_counts.entry(a.id.clone()).or_insert(0) += 1;
        }

        egui::SidePanel::left("list")
            .resizable(true)
            .default_width(400.0)
            .width_range(320.0..=560.0)
            .frame(
                egui::Frame::new()
                    .fill(LISTBG)
                    .inner_margin(Margin::symmetric(12, 12))
                    .stroke(Stroke::new(1.0, LINE)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let title = match &self.selected_tag {
                        Some(t) => format!("#{t}"),
                        None => "Alle Einträge".into(),
                    };
                    ui.label(RichText::new(title).size(16.0).strong().color(INK));
                    ui.label(RichText::new(format!("{} Treffer", items.len())).small().color(MUTED));
                });
                let terms = self.search_terms();
                if !terms.is_empty() {
                    let matches: Vec<String> = self
                        .all_tags()
                        .into_iter()
                        .filter(|t| terms.iter().any(|q| t.contains(q.as_str())))
                        .take(8)
                        .collect();
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Passende Tags:").small().color(MUTED));
                        for t in matches {
                            if tag_chip(ui, &t).clicked() {
                                self.selected_tag = Some(t.clone());
                                self.search.clear();
                            }
                        }
                    });
                }
                ui.add_space(6.0);

                let footer_h = 34.0;
                egui::ScrollArea::vertical()
                    .id_salt(("listscroll", self.page, self.filter_key.clone()))
                    .auto_shrink([false, false])
                    .max_height(ui.available_height() - footer_h)
                    .show(ui, |ui| {
                        if items.is_empty() {
                            ui.add_space(30.0);
                            ui.vertical_centered(|ui| {
                                if self.articles.is_empty() {
                                    ui.label(RichText::new("Noch keine Einträge").size(15.0).color(INK));
                                    ui.label(
                                        RichText::new("Klicke oben auf „➕ Neu“, um den ersten Eintrag zu schreiben.")
                                            .color(MUTED),
                                    );
                                    ui.add_space(8.0);
                                    if ui.button("Beispieleinträge anlegen").clicked() {
                                        store::write_demo(&self.cfg);
                                        self.reload();
                                    }
                                } else {
                                    ui.label(RichText::new("Keine Einträge mit diesen Tags.").color(MUTED));
                                }
                            });
                        }
                        let start = self.page * PAGE;
                        let mut open = None;
                        let mut chip = None;
                        for &i in items.iter().skip(start).take(PAGE) {
                            let a = &self.articles[i];
                            let sel = current_dir.as_deref() == Some(a.rel_dir.as_str());
                            let dup = id_counts.get(a.id.as_str()).copied().unwrap_or(0) > 1;
                            let (c, t) = entry_card(ui, a, sel, dup);
                            if c {
                                open = Some(i);
                            }
                            if t.is_some() {
                                chip = t;
                            }
                            ui.add_space(8.0);
                        }
                        if let Some(t) = chip {
                            self.selected_tag = Some(t);
                        } else if let Some(i) = open {
                            self.open_article(i);
                        }
                    });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.page > 0, egui::Button::new("‹ Neuere")).clicked() {
                        self.page -= 1;
                    }
                    ui.label(
                        RichText::new(format!("Seite {} von {}", self.page + 1, pages))
                            .small()
                            .color(MUTED),
                    );
                    if ui
                        .add_enabled(self.page + 1 < pages, egui::Button::new("Ältere ›"))
                        .clicked()
                    {
                        self.page += 1;
                    }
                });
            });
    }
}
