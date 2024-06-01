use std::sync::Arc;

use eframe::egui::{self, Align2, Ui};
use egui_extras::{Column, TableBuilder};
use nomi_core::{
    configs::profile::{ProfileState, VersionProfile},
    downloads::traits::DownloadResult,
    fs::{read_toml_config_sync, write_toml_config_sync},
    repository::launcher_manifest::LauncherManifest,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Sender;
use tracing::error;

use crate::{download::spawn_download, utils::spawn_tokio_future, Storage};

use super::{
    add_profile_menu::{AddProfileMenu, AddProfileMenuState},
    Component, StorageCreationExt,
};

pub struct ProfilesPage<'a> {
    pub download_result_tx: Sender<VersionProfile>,
    pub download_progress_tx: Sender<DownloadResult>,
    pub download_total_tx: Sender<u32>,

    pub is_profile_window_open: &'a mut bool,

    pub state: &'a mut ProfilesState,
    pub menu_state: &'a mut AddProfileMenuState,

    pub launcher_manifest: &'static LauncherManifest,
}

#[derive(Serialize, Deserialize, Default)]
pub struct ProfilesState {
    pub profiles: Vec<Arc<VersionProfile>>,
}

impl ProfilesState {
    pub const fn default_const() -> Self {
        Self {
            profiles: Vec::new(),
        }
    }

    pub fn add_profile(&mut self, profile: VersionProfile) {
        self.profiles.push(profile.into());
    }

    /// Create an id for the profile
    /// depends on the last id in the vector
    pub fn create_id(&self) -> i32 {
        match &self.profiles.iter().max_by_key(|x| x.id) {
            Some(v) => v.id + 1,
            None => 0,
        }
    }

    pub fn update_config(&self) -> anyhow::Result<()> {
        write_toml_config_sync(&self, "./.nomi/configs/Profiles.toml")
    }
}

impl StorageCreationExt for ProfilesPage<'_> {
    fn extend(storage: &mut Storage) -> anyhow::Result<()> {
        let profiles = read_toml_config_sync::<ProfilesState>("./.nomi/configs/Profiles.toml")
            .unwrap_or_default();

        storage.insert(profiles);

        Ok(())
    }
}

impl Component for ProfilesPage<'_> {
    fn ui(self, ui: &mut Ui) {
        {
            ui.toggle_value(self.is_profile_window_open, "Add new profile");

            egui::Window::new("Create new profile")
                .title_bar(true)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .movable(false)
                .open(self.is_profile_window_open)
                .show(ui.ctx(), |ui| {
                    AddProfileMenu {
                        state: self.menu_state,
                        profiles_state: self.state,
                        launcher_manifest: self.launcher_manifest,
                        // is_profile_window_open: self.is_profile_window_open,
                    }
                    .ui(ui);
                });
        }

        ui.style_mut().wrap = Some(false);

        TableBuilder::new(ui)
            .column(Column::auto().at_most(120.0))
            .columns(Column::auto(), 2)
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.label("Name");
                });
                header.col(|ui| {
                    ui.label("Version");
                });
            })
            .body(|mut body| {
                for profile in self.state.profiles.iter().cloned() {
                    body.row(30.0, |mut row| {
                        row.col(|ui| {
                            ui.add(egui::Label::new(&profile.name).truncate(true));
                        });
                        row.col(|ui| {
                            ui.label(profile.version());
                        });
                        row.col(|ui| match &profile.state {
                            ProfileState::Downloaded(instance) => {
                                if ui.button("Launch").clicked() {
                                    let instance = instance.clone();
                                    let (tx, _rx) = tokio::sync::mpsc::channel(100);
                                    spawn_tokio_future(tx, async move {
                                        instance
                                            .launch()
                                            .await
                                            .inspect_err(|e| error!("{}", e))
                                            .unwrap()
                                    });
                                }
                            }
                            ProfileState::NotDownloaded { .. } => {
                                if ui.button("Download").clicked() {
                                    spawn_download(
                                        profile,
                                        self.download_result_tx.clone(),
                                        self.download_progress_tx.clone(),
                                        self.download_total_tx.clone(),
                                    );
                                }
                            }
                        });
                    });
                }
            });
    }
}
