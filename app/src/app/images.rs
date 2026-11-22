//! Images: pick via button or drag & drop into the window. The file is copied into the
//! article's own folder (Hugo page bundle) and the right Markdown / figure tag is inserted.

use super::*;
use eframe::egui::{FontId, Id, LayerId, Order};
use std::path::Path;

impl HmbApp {
    /// Adds the given image files to the open article (creates one if none is open).
    pub(super) fn add_images(&mut self, ctx: &egui::Context, files: &[PathBuf]) {
        let (images, others): (Vec<&PathBuf>, Vec<&PathBuf>) =
            files.iter().partition(|p| store::is_image(p.as_path()));
        if images.is_empty() {
            if !others.is_empty() {
                self.set_status("Nur Bilder (png, jpg, gif, webp, svg) können eingefügt werden.", false);
            }
            return;
        }
        if self.editor.is_none() {
            self.new_entry();
        }
        // the article needs its folder on disk before images can be stored next to it
        if self.editor.as_ref().map(|e| e.is_new).unwrap_or(false) {
            self.save_impl(true);
        }
        let Some(ed) = &self.editor else { return };
        let article = ed.a.clone();
        let mut tags = Vec::new();
        let mut names = Vec::new();
        for p in images {
            match store::add_image(&self.cfg, &article, Path::new(p)) {
                Ok(name) => {
                    tags.push(store::image_markup(&self.cfg, &name));
                    names.push(name);
                }
                Err(e) => {
                    self.set_status(format!("Bild nicht gespeichert: {e}"), false);
                    return;
                }
            }
        }
        let text = format!("\n{}\n", tags.join("\n\n"));
        self.insert_text(ctx, &text, "", "");
        self.save_impl(true);
        self.set_status(
            format!("Bild gespeichert in {}: {}", article.rel_dir, names.join(", ")),
            true,
        );
    }

    pub(super) fn pick_images(&mut self, ctx: &egui::Context) {
        let files = rfd::FileDialog::new()
            .set_title("Bild hinzufügen")
            .add_filter("Bilder", &store::IMAGE_EXT)
            .pick_files()
            .unwrap_or_default();
        if !files.is_empty() {
            self.add_images(ctx, &files);
        }
    }

    pub(super) fn handle_drops(&mut self, ctx: &egui::Context) {
        let files: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !files.is_empty() {
            self.add_images(ctx, &files);
        }
    }

    /// Shows a hint while files are dragged over the window.
    pub(super) fn drop_overlay(&self, ctx: &egui::Context) {
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if !hovering {
            return;
        }
        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("drop_overlay")));
        let rect = ctx.screen_rect();
        painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(37, 99, 235, 60));
        painter.rect_stroke(
            rect.shrink(12.0),
            12.0,
            egui::Stroke::new(3.0, ACCENT),
            egui::StrokeKind::Inside,
        );
        let target = match &self.editor {
            Some(e) if !e.is_new => format!("„{}“", e.a.title),
            _ => "einen neuen Eintrag".to_string(),
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("🖼  Loslassen, um das Bild in {target} einzufügen"),
            FontId::proportional(24.0),
            INK,
        );
    }
}
