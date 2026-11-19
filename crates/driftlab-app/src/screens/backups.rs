//! Calibration backups: list, restore, export, delete (with confirmation).

use crate::app::{App, OpResult};
use crate::widgets::{self, card, chip, ghost, h1};
use driftlab_core::storage;
use egui::Ui;
use std::path::PathBuf;

#[derive(Default)]
pub struct BackupsUi {
    pub confirm_delete: Option<PathBuf>,
    pub confirm_restore: Option<PathBuf>,
}

pub fn ui(app: &mut App, ui: &mut Ui, now: f64) {
    let t = app.tokens;
    h1(ui, app.t("bk.title"));
    widgets::dim(ui, &t, app.t("bk.sub"));
    ui.add_space(12.0);
    let list = storage::list_backups(&app.data_dir);
    if list.is_empty() {
        card(ui, &t, |ui| {
            ui.label(app.t("bk.empty"));
        });
        return;
    }
    let connected_family = app.dev().map(|d| d.info.family);
    for (path, b) in list {
        card(ui, &t, |ui| {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&b.name).strong());
                    ui.label(widgets::num(&b.timestamp, 13.0));
                    ui.horizontal(|ui| {
                        chip(ui, &t, &driftlab_core::mask_serial(&b.serial), false);
                        if let Some(f) = &b.firmware {
                            chip(ui, &t, &format!("FW {f}"), false);
                        }
                    });
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if app.backups_ui.confirm_delete.as_ref() == Some(&path) {
                        if widgets::primary(ui, &t, app.t("bk.confirm_delete"), true).clicked() {
                            let _ = storage::delete_backup(&path);
                            app.backups_ui.confirm_delete = None;
                        }
                        if ghost(ui, &t, app.t("btn.cancel"), true).clicked() {
                            app.backups_ui.confirm_delete = None;
                        }
                    } else if app.backups_ui.confirm_restore.as_ref() == Some(&path) {
                        if widgets::primary(ui, &t, app.t("bk.confirm_restore"), true).clicked() {
                            let blob = b.blob.clone();
                            app.run_job("restore", OpResult::Unit, move |d| d.restore(&blob));
                            app.backups_ui.confirm_restore = None;
                        }
                        if ghost(ui, &t, app.t("btn.cancel"), true).clicked() {
                            app.backups_ui.confirm_restore = None;
                        }
                    } else {
                        if ghost(ui, &t, app.t("bk.delete"), true).clicked() {
                            app.backups_ui.confirm_delete = Some(path.clone());
                        }
                        if ghost(ui, &t, app.t("bk.export"), true).clicked() {
                            if let Some(dst) = rfd::FileDialog::new()
                                .set_file_name(path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
                                .save_file()
                            {
                                match std::fs::copy(&path, &dst) {
                                    Ok(_) => app.toast(now, format!("{} {}", app.t("toast.saved"), dst.display()), false),
                                    Err(e) => app.toast(now, e.to_string(), true),
                                }
                            }
                        }
                        let can = connected_family == Some(b.blob.family)
                            && app.dev().is_some_and(|d| d.info.connection != driftlab_core::Connection::Bluetooth);
                        if widgets::primary(ui, &t, app.t("bk.restore"), can && app.pending.is_none()).clicked() {
                            app.backups_ui.confirm_restore = Some(path.clone());
                        }
                    }
                });
            });
        });
        ui.add_space(8.0);
    }
    widgets::dim(ui, &t, app.t("bk.restore_note"));
}
