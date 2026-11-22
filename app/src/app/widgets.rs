//! Small drawing helpers: tag rows, tag chips, entry cards, style.

use super::*;
use eframe::egui::{CornerRadius, FontId, Margin, RichText, Sense, Stroke, TextStyle, Vec2};

/// One row in the tag sidebar. Returns (row clicked, arrow clicked).
pub(super) fn tag_row(
    ui: &mut egui::Ui,
    label: &str,
    count: usize,
    selected: bool,
    depth: usize,
    expandable: Option<bool>,
) -> (bool, bool) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, CornerRadius::same(6), ACCENT_SOFT);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(229, 231, 235));
    }
    let indent = 4.0 + depth as f32 * 14.0;
    let mut toggled = false;
    let arrow_rect = egui::Rect::from_min_size(rect.min + Vec2::new(indent, 4.0), Vec2::splat(18.0));
    if let Some(open) = expandable {
        let c = arrow_rect.center();
        let pts = if open {
            vec![c + Vec2::new(-4.0, -2.0), c + Vec2::new(4.0, -2.0), c + Vec2::new(0.0, 3.0)]
        } else {
            vec![c + Vec2::new(-2.0, -4.0), c + Vec2::new(3.0, 0.0), c + Vec2::new(-2.0, 4.0)]
        };
        p.add(egui::Shape::convex_polygon(pts, MUTED, Stroke::NONE));
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                toggled = arrow_rect.expand(3.0).contains(pos);
            }
        }
    }
    let color = if selected { ACCENT } else { INK };
    p.text(
        egui::pos2(rect.min.x + indent + 20.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        color,
    );
    p.text(
        egui::pos2(rect.max.x - 8.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        count.to_string(),
        FontId::proportional(12.0),
        MUTED,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    (resp.clicked() && !toggled, toggled)
}

pub(super) fn tag_chip(ui: &mut egui::Ui, t: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(format!("#{t}")).small().color(TAGFG))
            .fill(TAGBG)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(9)),
    )
}

/// A card in the list: title, date + tags, abstract (max. 3 lines).
/// Returns (card clicked, tag clicked).
/// `dup`: another folder holds an article with the same ID (shown as a warning).
pub(super) fn entry_card(ui: &mut egui::Ui, a: &Article, selected: bool, dup: bool) -> (bool, Option<String>) {
    let mut tag_clicked = None;
    let stroke = if selected { Stroke::new(1.5, ACCENT) } else { Stroke::new(1.0, LINE) };
    let inner = egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let title = if a.title.trim().is_empty() { "Ohne Titel" } else { a.title.trim() };
            ui.add(egui::Label::new(RichText::new(title).size(15.5).strong().color(INK)).wrap());
            if a.draft || dup {
                ui.horizontal(|ui| {
                    if a.draft {
                        ui.label(RichText::new("Entwurf").small().color(WARN));
                    }
                    if dup {
                        ui.label(RichText::new("⚠ doppelt").small().color(DANGER)).on_hover_text(format!(
                            "Diese ID gibt es mehrfach. Dieser Ordner: {}\nEinstellungen (⚙) → „Doppelte Ordner reparieren“",
                            a.rel_dir
                        ));
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(RichText::new(a.date.format("%d.%m.%Y · %H:%M").to_string()).small().color(MUTED));
                ui.add_space(4.0);
                for t in &a.tags {
                    if tag_chip(ui, t).clicked() {
                        tag_clicked = Some(t.clone());
                    }
                }
            });
            let text = if a.abstract_.trim().is_empty() {
                first_words(&store::plain_text(&a.body), 300)
            } else {
                store::plain_text(&a.abstract_)
            };
            if !text.is_empty() {
                ui.add_space(2.0);
                let mut job = egui::text::LayoutJob::single_section(
                    text,
                    egui::TextFormat {
                        font_id: FontId::proportional(13.5),
                        color: Color32::from_rgb(55, 65, 81),
                        ..Default::default()
                    },
                );
                job.wrap = egui::text::TextWrapping {
                    max_width: ui.available_width(),
                    max_rows: 3,
                    break_anywhere: false,
                    overflow_character: Some('…'),
                };
                let galley = ui.fonts(|f| f.layout_job(job));
                ui.label(galley);
            }
        });
    let resp = ui.interact(inner.response.rect, ui.id().with(("card", &a.rel_dir)), Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    (resp.clicked() && tag_clicked.is_none(), tag_clicked)
}

pub(super) fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::light();
    v.panel_fill = Color32::WHITE;
    v.window_fill = Color32::WHITE;
    v.extreme_bg_color = Color32::WHITE;
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    v.window_corner_radius = CornerRadius::same(10);
    v.widgets.inactive.corner_radius = CornerRadius::same(6);
    v.widgets.hovered.corner_radius = CornerRadius::same(6);
    v.widgets.active.corner_radius = CornerRadius::same(6);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(243, 244, 246);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(10.0, 5.0);
        s.text_styles.insert(TextStyle::Body, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(11.5));
        s.text_styles.insert(TextStyle::Monospace, FontId::monospace(13.5));
    });
}

pub(super) fn native_view(cc: &eframe::CreationContext<'_>) -> usize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle() {
        Ok(h) => match h.as_raw() {
            RawWindowHandle::AppKit(a) => a.ns_view.as_ptr() as usize,
            _ => 0,
        },
        Err(_) => 0,
    }
}

pub(super) fn set_body_cursor(ctx: &egui::Context, start: usize, end: usize) {
    let id = egui::Id::new("body_editor");
    if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
        state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
        state.store(ctx, id);
    }
}

pub(super) fn byte_idx(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

pub(super) fn short(s: &str, n: usize) -> String {
    let one = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > n {
        format!("{}…", one.chars().take(n).collect::<String>())
    } else {
        one
    }
}

pub(super) fn first_words(body: &str, n: usize) -> String {
    let text: String = body
        .lines()
        .filter(|l| !l.trim_start().starts_with("```") && !l.trim().is_empty())
        .map(|l| l.trim_start_matches(['#', '>', '-', '*', ' ']).trim())
        .collect::<Vec<_>>()
        .join(" ");
    short(&text, n)
}

pub(super) fn mtime(p: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(p).ok().and_then(|m| m.modified().ok())
}
