mod hardware;
use eframe::egui;
use hardware::HardwareType;

use std::sync::Arc;

#[derive(Clone)]
struct SelectedFirmware {
    tag: String,
    asset: String,
}

struct MyApp {
    active_view: View,
    selected_hw_type: Option<HardwareType>,
    flash_release_service: flash::FlashReleaseService,
    selected_firmware: Option<SelectedFirmware>,
    download_handle: Option<flash::FirmwareDownloadHandle>,
    download_progress: Option<usize>,
    download_done: bool,
    download_error: Option<String>,
    downloaded_path: Option<String>,
    flash_result_message: Option<String>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            active_view: View::default(),
            selected_hw_type: None,
            flash_release_service: flash::FlashReleaseService::new(),
            selected_firmware: None,
            download_handle: None,
            download_progress: None,
            download_done: false,
            download_error: None,
            downloaded_path: None,
            flash_result_message: None,
        }
    }
}
mod flash;

#[derive(PartialEq)]
enum View {
    Flash,
    SetSerial,
    SetCapacity,
    ReadSystem,
    AppUpdate,
    About,
    Settings,
    Help,
}

// Standard‑View ist Flash
impl Default for View {
    fn default() -> Self {
        View::Flash
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Menüleiste
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Device", |ui| {
                    if ui.button("Flash").clicked() {
                        self.active_view = View::Flash;
                    }
                    if ui.button("Set serial number").clicked() {
                        self.active_view = View::SetSerial;
                    }
                    if ui.button("Set capacity").clicked() {
                        self.active_view = View::SetCapacity;
                    }
                    if ui.button("Read system values").clicked() {
                        self.active_view = View::ReadSystem;
                    }
                });
                ui.menu_button("App", |ui| {
                    if ui.button("Update app").clicked() {
                        self.active_view = View::AppUpdate;
                    }
                    if ui.button("About").clicked() {
                        self.active_view = View::About;
                    }
                });
                ui.menu_button("Settings", |ui| {
                    if ui.button("Settings").clicked() {
                        self.active_view = View::Settings;
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Manual").clicked() {
                        self.active_view = View::Help;
                    }
                });
            });
        });

        // Zentraler Content
        egui::CentralPanel::default().show(ctx, |ui| match self.active_view {
            View::Flash => {
                ui.heading("Flash device");
                ui.separator();
                ui.label("1. Select hardware type:");
                let mut hw_type_changed = false;
                ui.horizontal(|ui| {
                    for hw_type in HardwareType::all().iter() {
                        let selected = self.selected_hw_type == Some(*hw_type);
                        if ui.selectable_label(selected, hw_type.to_string()).clicked() {
                            if self.selected_hw_type != Some(*hw_type) {
                                self.selected_hw_type = Some(*hw_type);
                                hw_type_changed = true;
                            }
                        }
                    }
                });

                // Service informieren
                self.flash_release_service
                    .set_hw_type(self.selected_hw_type);
                self.flash_release_service.poll();

                if self.selected_hw_type.is_some() {
                    let (releases, releases_loading, releases_error) =
                        self.flash_release_service.get_state();
                    ui.add_space(16.0);
                    ui.label("2. Select firmware:");
                    if releases_loading {
                        ui.label("Loading firmware...");
                    } else if let Some(err) = &releases_error {
                        ui.colored_label(egui::Color32::RED, err);
                    } else if let Some(releases) = &releases {
                        if releases.is_empty() {
                            ui.label("No firmware found.");
                        } else {
                            for release in releases.iter() {
                                ui.collapsing(
                                    format!(
                                        "{}{}",
                                        release.tag_name,
                                        if release.prerelease {
                                            " (Pre-release)"
                                        } else {
                                            ""
                                        }
                                    ),
                                    |ui| {
                                        for asset in &release.stm32_assets {
                                            let is_selected = if let Some(sel) =
                                                &self.selected_firmware
                                            {
                                                sel.tag == release.tag_name && sel.asset == *asset
                                            } else {
                                                false
                                            };
                                            if ui.selectable_label(is_selected, asset).clicked() {
                                                // Wenn eine neue Firmware gewählt wird, alles zurücksetzen
                                                self.selected_firmware = Some(SelectedFirmware {
                                                    tag: release.tag_name.clone(),
                                                    asset: asset.clone(),
                                                });
                                                self.download_progress = None;
                                                self.download_done = false;
                                                self.download_error = None;
                                                self.downloaded_path = None;
                                                self.download_handle = None;
                                            }
                                        }
                                    },
                                );
                            }
                        }
                        if let Some(sel) = &self.selected_firmware {
                            if self.download_handle.is_none()
                                && self.download_progress.is_none()
                                && self.download_error.is_none()
                                && !self.download_done
                            {
                                if let Some(hw) = self.selected_hw_type {
                                    let repo = hw.repo().to_string();
                                    let tag = sel.tag.clone();
                                    let asset = sel.asset.clone();
                                    self.download_progress = Some(0); // Progressbar sofort anzeigen
                                    self.download_handle = Some(
                                        flash::FirmwareDownloadHandle::start(repo, tag, asset),
                                    );
                                }
                            }
                            // Download-Progressbar und Flash-Button
                            if let Some(handle) = &mut self.download_handle {
                                while let Ok(msg) = handle.rx.try_recv() {
                                    match msg {
                                        flash::DownloadMsg::Progress(p) => {
                                            self.download_progress = Some(p);
                                        }
                                        flash::DownloadMsg::Done(path) => {
                                            self.download_done = true;
                                            self.downloaded_path = Some(path);
                                        }
                                        flash::DownloadMsg::Error(e) => {
                                            self.download_error = Some(e);
                                        }
                                    }
                                }
                            }
                            if let Some(progress) = self.download_progress {
                                if !self.download_done {
                                    ui.label("Downloading...");
                                    ui.add(
                                        egui::ProgressBar::new(progress as f32 / 100.0)
                                            .show_percentage(),
                                    );
                                } else if self.download_done {
                                    ui.label("Download complete.");
                                    ui.add_space(16.0);
                                    ui.label("3. Flash firmware:");
                                    if ui.button("Start flashing firmware now").clicked() {
                                        if let Some(path) = &self.downloaded_path {
                                            let result = flash::flash_with_st_flash(path);
                                            self.flash_result_message = Some(result);
                                        }
                                    }
                                    if let Some(msg) = &self.flash_result_message {
                                        ui.add_space(8.0);
                                        ui.label(msg);
                                    }
                                }
                            }
                            if let Some(err) = &self.download_error {
                                ui.colored_label(egui::Color32::RED, err);
                            }
                        }
                    }
                }
            }
            View::SetSerial => {
                ui.heading("Set serial number");
            }
            View::SetCapacity => {
                ui.heading("Set capacity");
            }
            View::ReadSystem => {
                ui.heading("Read system values");
            }
            View::AppUpdate => {
                ui.heading("App update");
            }
            View::About => {
                ui.heading("About this app");
                ui.label("BMS Installer Maintenance v0.1");
            }
            View::Settings => {
                ui.heading("Settings");
            }
            View::Help => {
                ui.heading("Help / Manual");
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "iRock Programmer",
        options,
        // ★ Hier muss das Boxed-Closure ein Result zurückgeben! ★
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}
