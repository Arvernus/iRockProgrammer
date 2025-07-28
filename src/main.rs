// Für Progressbar-Animation
use eframe::{App, Frame, NativeOptions, egui};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

enum GuiJob {
    ReleasesResult(Result<Vec<Release>, String>),
    AssetProgress(usize),
    AssetResult(Result<String, String>),
    FlashResult(String),
}

struct GuiApp {
    show_info: bool,
    show_update: bool,
    show_restart_msg: Option<String>,
    hardware_types: Vec<HardwareType>,
    selected_hardware: Option<HardwareType>,
    releases: Vec<Release>,
    releases_loading: bool,
    releases_error: Option<String>,
    selected_release: Option<Release>,
    hw_versions: Vec<String>,
    selected_hw_version: Option<String>,
    asset_downloading: bool,
    asset_progress: usize,
    asset_error: Option<String>,
    asset_path: Option<String>,
    flashing: bool,
    flash_result: Option<String>,
    update_result: Option<String>,
    jobs: Vec<GuiJob>,
    job_sender: Option<Sender<GuiJob>>,
    job_receiver: Option<Receiver<GuiJob>>,
    progress_start: Option<Instant>,
}

impl Default for GuiApp {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            show_info: false,
            show_update: false,
            show_restart_msg: None,
            hardware_types: HardwareType::all().to_vec(),
            selected_hardware: None,
            releases: vec![],
            releases_loading: false,
            releases_error: None,
            selected_release: None,
            hw_versions: vec![],
            selected_hw_version: None,
            asset_downloading: false,
            asset_progress: 0,
            asset_error: None,
            asset_path: None,
            flashing: false,
            flash_result: None,
            update_result: None,
            jobs: Vec::new(),
            job_sender: Some(tx),
            job_receiver: Some(rx),
            progress_start: None,
        }
    }
}

impl App for GuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // Verarbeite alle Jobs aus dem Channel
        if let Some(rx) = &self.job_receiver {
            while let Ok(job) = rx.try_recv() {
                match job {
                    GuiJob::ReleasesResult(res) => {
                        self.releases_loading = false;
                        match res {
                            Ok(releases) => self.releases = releases,
                            Err(e) => self.releases_error = Some(e),
                        }
                    }
                    GuiJob::AssetProgress(p) => {
                        self.asset_progress = p;
                    }
                    GuiJob::AssetResult(res) => {
                        self.asset_downloading = false;
                        match res {
                            Ok(path) => self.asset_path = Some(path),
                            Err(e) => self.asset_error = Some(e),
                        }
                    }
                    GuiJob::FlashResult(msg) => {
                        self.flashing = false;
                        self.flash_result = Some(msg);
                    }
                }
            }
        }

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button(egui::RichText::new("iRockProgrammer").strong(), |ui| {
                    if ui.button("Über iRockProgrammer").clicked() {
                        self.show_info = true;
                        ui.close();
                    }
                    if ui.button("iRockProgrammer aktualisieren").clicked() {
                        self.show_update = true;
                        ui.close();
                    }
                    if ui.button("iRockProgrammer beenden").clicked() {
                        std::process::exit(0);
                    }
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("iRock Programmer");
            if let Some(msg) = &self.show_restart_msg {
                ui.label(msg);
                if ui.button("OK").clicked() {
                    self.show_restart_msg = None;
                }
                return;
            }

            // Hardware-Auswahl
            if self.selected_hardware.is_none() {
                ui.label("Bitte Hardware-Typ auswählen:");
                for hw in &self.hardware_types {
                    if ui.button(hw.to_string()).clicked() {
                        self.selected_hardware = Some(*hw);
                        self.releases_loading = true;
                        self.progress_start = Some(Instant::now());
                        self.releases_error = None;
                        let repo = hw.repo().to_string();
                        let tx = ctx.clone();
                        let sender = self.job_sender.as_ref().unwrap().clone();
                        thread::spawn(move || {
                            let result = fetch_releases(&repo).map_err(|e| format!("{}", e));
                            let _ = sender.send(GuiJob::ReleasesResult(result));
                            tx.request_repaint();
                        });
                    }
                }
                return;
            }

            // Releases laden
            if self.releases_loading {
                // Animierte Progressbar
                let percent = if let Some(start) = self.progress_start {
                    let elapsed = start.elapsed().as_secs_f32();
                    // 2 Sekunden für 100%
                    (elapsed / 2.0).min(1.0)
                } else {
                    0.0
                };
                ui.label("Lade Releases...");
                ui.add(egui::ProgressBar::new(percent).show_percentage());
                ctx.request_repaint_after(Duration::from_millis(50));
                return;
            } else {
                self.progress_start = None;
            }
            if let Some(e) = &self.releases_error {
                ui.colored_label(egui::Color32::RED, e);
                if ui.button("Zurück").clicked() {
                    self.selected_hardware = None;
                    self.releases.clear();
                    self.releases_error = None;
                }
                return;
            }
            if self.selected_release.is_none() {
                ui.label("Bitte Software-Version auswählen:");
                for release in &self.releases {
                    let display = format!(
                        "{}{}",
                        release.tag_name,
                        if release.prerelease {
                            " (pre-release)"
                        } else {
                            ""
                        }
                    );
                    if ui.button(&display).clicked() {
                        self.selected_release = Some(release.clone());
                        // Hardware-Versionen extrahieren
                        let mut hw_versions = vec![];
                        for asset in &release.stm32_assets {
                            if let Some((name, ext)) = asset.rsplit_once('.') {
                                if ext == "bin" || ext == "hex" {
                                    if let Some((_, hw)) = name.rsplit_once('-') {
                                        hw_versions.push(hw.to_string());
                                    }
                                }
                            }
                        }
                        hw_versions.sort();
                        hw_versions.dedup();
                        self.hw_versions = hw_versions;
                    }
                }
                if ui.button("Zurück").clicked() {
                    self.selected_hardware = None;
                    self.releases.clear();
                }
                return;
            }
            if self.selected_hw_version.is_none() {
                ui.label("Bitte Hardware-Version auswählen:");
                for hw in &self.hw_versions {
                    if ui.button(hw).clicked() {
                        self.selected_hw_version = Some(hw.clone());
                    }
                }
                if ui.button("Zurück").clicked() {
                    self.selected_release = None;
                    self.hw_versions.clear();
                }
                return;
            }
            // Asset-Download
            if self.asset_path.is_none() && !self.asset_downloading {
                if ui.button("Firmware herunterladen").clicked() {
                    self.asset_downloading = true;
                    self.asset_progress = 0;
                    self.asset_error = None;
                    let repo = self.selected_hardware.unwrap().repo().to_string();
                    let tag = self.selected_release.as_ref().unwrap().tag_name.clone();
                    let hw = self.selected_hw_version.as_ref().unwrap().clone();
                    let assets = &self.selected_release.as_ref().unwrap().stm32_assets;
                    let asset_name = assets.iter().find(|a| a.contains(&hw)).cloned();
                    let tx = ctx.clone();
                    let sender = self.job_sender.as_ref().unwrap().clone();
                    thread::spawn(move || {
                        if let Some(asset_name) = asset_name {
                            let result =
                                download_github_asset_progress_gui(&repo, &tag, &asset_name, {
                                    let sender = sender.clone();
                                    let tx = tx.clone();
                                    move |progress| {
                                        let _ = sender.send(GuiJob::AssetProgress(progress));
                                        tx.request_repaint();
                                    }
                                })
                                .map(|p| p.display().to_string())
                                .map_err(|e| format!("{}", e));
                            let _ = sender.send(GuiJob::AssetResult(result));
                            tx.request_repaint();
                        }
                    });
                }
            }
            if self.asset_downloading {
                ui.label("Lade Asset herunter...");
                ui.add(
                    egui::ProgressBar::new(self.asset_progress as f32 / 100.0).show_percentage(),
                );
                return;
            }
            if let Some(e) = &self.asset_error {
                ui.colored_label(egui::Color32::RED, e);
                if ui.button("Zurück").clicked() {
                    self.selected_hw_version = None;
                    self.asset_error = None;
                }
                return;
            }
            if let Some(path) = &self.asset_path {
                ui.label(format!("Asset wurde heruntergeladen: {}", path));
                if !self.flashing {
                    if ui.button("Jetzt auf STM32 flashen").clicked() {
                        self.flashing = true;
                        self.flash_result = None;
                        let flash_path = path.clone();
                        let tx = ctx.clone();
                        let sender = self.job_sender.as_ref().unwrap().clone();
                        thread::spawn(move || {
                            let output = std::process::Command::new("st-flash")
                                .arg("write")
                                .arg(&flash_path)
                                .arg("0x08000000")
                                .output();
                            let msg = match output {
                                Ok(out) if out.status.success() => {
                                    format!(
                                        "Flash erfolgreich!\n\n{}",
                                        String::from_utf8_lossy(&out.stdout)
                                    )
                                }
                                Ok(out) => {
                                    format!(
                                        "Fehler beim Flashen!\n\n{}",
                                        String::from_utf8_lossy(&out.stderr)
                                    )
                                }
                                Err(e) => format!("Fehler beim Starten von st-flash: {}", e),
                            };
                            let _ = sender.send(GuiJob::FlashResult(msg));
                            tx.request_repaint();
                        });
                    }
                } else {
                    ui.label("Flashe Firmware ...");
                }
                if let Some(msg) = &self.flash_result {
                    ui.label(msg);
                    if ui.button("Beenden").clicked() {
                        std::process::exit(0);
                    }
                }
                return;
            }
        });

        if self.show_info {
            egui::Window::new("Info")
                .open(&mut self.show_info)
                .show(ctx, |ui| {
                    ui.label(format!("Version: {}", env!("CARGO_PKG_VERSION")));
                    ui.label("Programmiere iRock Mobile Geräte");
                });
        }
        // Workaround für Borrow-Checker: show_update-Status lokal puffern
        let mut show_update = self.show_update;
        if show_update {
            egui::Window::new("Update")
                .open(&mut show_update)
                .show(ctx, |ui| {
                    if self.update_result.is_none() {
                        ui.label("Update wird durchgeführt...");
                        let sender = self.job_sender.as_ref().unwrap().clone();
                        let tx = ctx.clone();
                        thread::spawn(move || {
                            let result = run_update_and_restart();
                            let _ = sender.send(GuiJob::FlashResult(match result {
                                0 => "Update erfolgreich. Bitte neu starten.".to_string(),
                                _ => "Update fehlgeschlagen.".to_string(),
                            }));
                            tx.request_repaint();
                        });
                    } else {
                        ui.label(self.update_result.as_ref().unwrap());
                        if ui.button("OK").clicked() {
                            // Wert nach außen zurückschreiben
                            self.update_result = None;
                            // show_update wird nach der Closure zurückgeschrieben
                        }
                    }
                });
            self.show_update = show_update;
        }
    }
}

use serde::Deserialize;

mod self_update_mod;
use self_update_mod::run_update_and_restart;

#[derive(Debug, Deserialize, Clone)]
struct Release {
    tag_name: String,
    prerelease: bool,
    stm32_assets: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
enum HardwareType {
    IRock424,
    IRock212,
    IRock200,
    IRock300,
    IRock400,
}

impl HardwareType {
    fn repo(&self) -> &'static str {
        match self {
            HardwareType::IRock424 => "Arvernus/iRock-424",
            HardwareType::IRock212 => "Arvernus/iRock-212",
            HardwareType::IRock200 | HardwareType::IRock300 | HardwareType::IRock400 => {
                "Arvernus/iRock-200-300-400"
            }
        }
    }
    fn all() -> &'static [HardwareType] {
        &[
            HardwareType::IRock424,
            HardwareType::IRock212,
            HardwareType::IRock200,
            HardwareType::IRock300,
            HardwareType::IRock400,
        ]
    }
}

impl std::fmt::Display for HardwareType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            HardwareType::IRock424 => "iRock 424",
            HardwareType::IRock212 => "iRock 212",
            HardwareType::IRock200 => "iRock 200",
            HardwareType::IRock300 => "iRock 300",
            HardwareType::IRock400 => "iRock 400",
        };
        write!(f, "{}", s)
    }
}

fn main() {
    let options = NativeOptions::default();
    let _ = eframe::run_native(
        "iRock Programmer",
        options,
        Box::new(|_cc| Ok(Box::<GuiApp>::default())),
    );
}
// Hilfsfunktion: Asset von GitHub herunterladen und temporär speichern, mit Fortschritt für GUI
fn download_github_asset_progress_gui<F>(
    repo: &str,
    tag: &str,
    asset_name: &str,
    mut progress_cb: F,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnMut(usize) + Send + 'static,
{
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        return Err("Ungültiges Repository-Format".into());
    }
    let owner = parts[0];
    let repo_name = parts[1];
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let asset_url = rt.block_on(async {
        let octocrab = octocrab::Octocrab::default();
        let release = octocrab
            .repos(owner, repo_name)
            .releases()
            .get_by_tag(tag)
            .await?;
        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| format!("Asset '{}' nicht gefunden", asset_name))?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(asset.browser_download_url.clone())
    })?;
    // Asset herunterladen mit Fortschritt
    let response = rt.block_on(async { reqwest::get(asset_url).await })?;
    let total = response.content_length().unwrap_or(0);
    let mut downloaded = 0u64;
    let dir = tempdir()?;
    let file_path = dir.path().join(asset_name);
    let mut file = File::create(&file_path)?;
    let mut stream = response;
    loop {
        let chunk = rt.block_on(async { stream.chunk().await })?;
        if let Some(chunk) = chunk {
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            let percent = if total > 0 {
                (downloaded as f32 / total as f32 * 100.0) as usize
            } else {
                0
            };
            progress_cb(percent);
        } else {
            break;
        }
    }
    // tempdir lebt nur solange wie das TempDir-Objekt, daher Pfad kopieren
    let final_path = std::env::temp_dir().join(asset_name);
    std::fs::copy(&file_path, &final_path)?;
    Ok(final_path)
}

fn fetch_releases(repo: &str) -> Result<Vec<Release>, Box<dyn std::error::Error + Send + Sync>> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async_fetch_releases(repo))
}

async fn async_fetch_releases(
    repo: &str,
) -> Result<Vec<Release>, Box<dyn std::error::Error + Send + Sync>> {
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        return Err("Ungültiges Repository-Format".into());
    }
    let owner = parts[0];
    let repo_name = parts[1];

    let octocrab = octocrab::Octocrab::default();
    let response = octocrab
        .repos(owner, repo_name)
        .releases()
        .list()
        .per_page(100)
        .send()
        .await?;

    let mut filtered_releases = Vec::new();
    for r in response.items {
        let stm32_assets: Vec<String> = r
            .assets
            .iter()
            .filter_map(|a| {
                let name = &a.name;
                if name.ends_with(".bin") || name.ends_with(".hex") || name.ends_with(".dfu") {
                    Some(name.clone())
                } else {
                    None
                }
            })
            .collect();
        if !stm32_assets.is_empty() {
            filtered_releases.push(Release {
                tag_name: r.tag_name,
                prerelease: r.prerelease,
                stm32_assets,
            });
        }
    }
    Ok(filtered_releases)
}
