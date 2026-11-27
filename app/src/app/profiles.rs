//! Profiles: one configuration per Hugo site (title up to 32 characters, own data folder,
//! own sync folder and sync state). Switched with the dropdown in the top bar.

use super::*;
use eframe::egui::RichText;

impl HmbApp {
    /// Writes the open profile into the profile list on disk (not in screenshot/demo mode).
    pub(super) fn persist(&mut self) -> Result<(), String> {
        if self.demo || self.screenshot.is_some() {
            return Ok(());
        }
        let i = self.profiles.active.min(self.profiles.profiles.len() - 1);
        self.profiles.profiles[i] = self.cfg.clone();
        store::save_profiles(&self.profiles)
    }

    fn busy(&self) -> bool {
        self.sync_rx.is_some() || self.git.busy()
    }

    /// Opens another profile: other articles, other sync folder, other Git repository.
    pub(super) fn switch_profile(&mut self, i: usize) {
        if i >= self.profiles.profiles.len() || i == self.profiles.active || self.busy() {
            return;
        }
        self.save_current();
        let _ = self.persist();
        self.profiles.active = i;
        self.reopen_profile();
        let msg = format!(
            "Profil „{}“: {} Einträge aus {}",
            self.cfg.title,
            self.articles.len(),
            widgets::show_path(&self.data)
        );
        self.set_status(msg, true);
    }

    /// Adds a profile with its own data folder and opens its settings.
    pub(super) fn new_profile(&mut self) {
        if self.busy() {
            return;
        }
        let title = self.profiles.unique_title("Neuer Blog", None);
        let mut cfg = store::Config { title: title.clone(), client: self.cfg.client.clone(), ..Default::default() };
        // its own "database": a data folder no other profile uses
        let base = cfg.data_dir.clone();
        let slug = store::slugify(&title, 32);
        let mut dir = base.with_file_name(format!("hmb-{slug}"));
        let mut n = 2;
        while self.profiles.profiles.iter().any(|p| p.data_dir == dir) || dir.exists() {
            dir = base.with_file_name(format!("hmb-{slug}-{n}"));
            n += 1;
        }
        cfg.data_dir = dir;
        self.profiles.profiles.push(cfg);
        let i = self.profiles.profiles.len() - 1;
        self.switch_profile(i);
        self.settings_draft = self.cfg.clone();
        self.show_settings = true;
        self.set_status(format!("Neues Profil „{title}“ – Titel, Ordner und Sync-Ordner einstellen."), true);
    }

    /// Removes the open profile from the list. Its folders and articles stay on disk.
    pub(super) fn delete_profile(&mut self) {
        if self.profiles.profiles.len() < 2 || self.busy() {
            return;
        }
        self.save_current();
        let i = self.profiles.active;
        let gone = self.profiles.profiles.remove(i);
        self.profiles.active = i.min(self.profiles.profiles.len() - 1);
        self.reopen_profile();
        self.set_status(
            format!("Profil „{}“ entfernt – seine Ordner und Artikel bleiben erhalten.", gone.title),
            true,
        );
    }

    /// Reloads everything for `self.profiles.active` without saving the previous profile.
    fn reopen_profile(&mut self) {
        let i = self.profiles.active;
        self.cfg = self.profiles.profiles[i].clone();
        self.settings_draft = self.cfg.clone();
        self.data = self.cfg.data_dir.clone();
        self.demo = false;
        self.editor = None;
        self.selected_tag = None;
        self.search.clear();
        self.page = 0;
        self.news = None;
        self.confirm_delete = false;
        self.confirm_delete_profile = false;
        self.git = git::GitState::default();
        if self.screenshot.is_none() {
            let _ = store::save_profiles(&self.profiles);
            let _ = sync::migrate_layout(&self.cfg);
        }
        self.reload();
        self.expanded = self.onto.parents.values().cloned().collect();
        self.dupes = sync::nested_copies(&self.cfg);
    }

    /// Dropdown in the top bar: open profile, switch, add, edit.
    pub(super) fn profile_selector(&mut self, ui: &mut egui::Ui) {
        let mut pick = None;
        let mut add = false;
        let mut edit = false;
        let titles = self.profiles.titles();
        let active = self.profiles.active;
        ui.add_enabled_ui(!self.busy(), |ui| {
            egui::ComboBox::from_id_salt("profile_select")
                .selected_text(RichText::new(&self.cfg.title).strong().color(INK))
                .width(190.0)
                .show_ui(ui, |ui| {
                    for (i, t) in titles.iter().enumerate() {
                        if ui.selectable_label(i == active, t).clicked() {
                            pick = Some(i);
                        }
                    }
                    ui.separator();
                    if ui.selectable_label(false, "+ Neues Profil …").clicked() {
                        add = true;
                    }
                    if ui.selectable_label(false, "Profil bearbeiten …").clicked() {
                        edit = true;
                    }
                })
                .response
                .on_hover_text("Profil (Hugo-Seite) wechseln – jedes Profil hat eigene Artikel, eigenen Sync-Ordner und eigenes Git-Repository");
        });
        if let Some(i) = pick {
            self.switch_profile(i);
        }
        if add {
            self.new_profile();
        }
        if edit {
            self.settings_draft = self.cfg.clone();
            self.show_settings = true;
        }
    }
}
