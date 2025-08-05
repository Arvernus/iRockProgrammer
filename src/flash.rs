use crate::hardware::HardwareType;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct FlashReleaseService {
    state: Arc<Mutex<FlashReleaseState>>,
}

#[derive(Default)]
struct FlashReleaseState {
    last_hw_type: Option<HardwareType>,
    releases: Option<Arc<Vec<Release>>>,
    releases_loading: bool,
    releases_error: Option<String>,
    releases_rx: Option<Receiver<Result<Vec<Release>, Box<dyn std::error::Error + Send + Sync>>>>,
}

impl FlashReleaseService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FlashReleaseState::default())),
        }
    }

    pub fn set_hw_type(&self, hw_type: Option<HardwareType>) {
        let mut state = self.state.lock().unwrap();
        if state.last_hw_type != hw_type {
            state.last_hw_type = hw_type;
            state.releases = None;
            state.releases_error = None;
            state.releases_loading = false;
            state.releases_rx = None;
        }
    }

    pub fn poll(&self) {
        let mut state = self.state.lock().unwrap();
        if let Some(hw_type) = state.last_hw_type {
            if state.releases.is_none() && state.releases_rx.is_none() && !state.releases_loading {
                let repo = hw_type.repo();
                state.releases_loading = true;
                state.releases_error = None;
                let (tx, rx) = mpsc::channel();
                let repo = repo.to_string();
                std::thread::spawn(move || {
                    let result = crate::flash::fetch_releases(&repo);
                    let _ = tx.send(result);
                });
                state.releases_rx = Some(rx);
            }
        }

        if let Some(rx) = &state.releases_rx {
            match rx.try_recv() {
                Ok(result) => {
                    state.releases_loading = false;
                    state.releases_rx = None;
                    match result {
                        Ok(releases) => state.releases = Some(Arc::new(releases)),
                        Err(e) => state.releases_error = Some(format!("Fehler: {}", e)),
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    state.releases_loading = false;
                    state.releases_rx = None;
                    state.releases_error = Some("Fehler beim Laden der Firmware".to_string());
                }
            }
        }
    }

    pub fn get_state(&self) -> (Option<Arc<Vec<Release>>>, bool, Option<String>) {
        let state = self.state.lock().unwrap();
        (
            state.releases.clone(),
            state.releases_loading,
            state.releases_error.clone(),
        )
    }
}
use std::path::PathBuf;

// Asset von GitHub herunterladen und temporär speichern, mit Fortschritt für GUI
pub fn download_github_asset_progress_gui<F>(
    repo: &str,
    tag: &str,
    asset_name: &str,
    mut progress_cb: F,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>>
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

pub fn fetch_releases(
    repo: &str,
) -> Result<Vec<Release>, Box<dyn std::error::Error + Send + Sync>> {
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

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub prerelease: bool,
    pub stm32_assets: Vec<String>,
}

// Es gibt nur noch einen Flash-Weg: probe-rs
// Die Funktion flash_with_probe_rs bleibt erhalten

// Modul für Flash-Logik und Datenabruf
pub struct FlashConfig {
    // Beispiel: Pfad zur Firmware-Datei
    pub firmware_path: String,
    // Weitere Konfigurationsoptionen hier
}

pub struct FlashResult {
    pub success: bool,
    pub message: String,
}

/// Führt den Flash-Vorgang aus
pub fn flash_hardware(config: &FlashConfig) -> FlashResult {
    let msg = flash_with_probe_rs(&config.firmware_path);
    let success = msg.contains("erfolgreich");
    FlashResult {
        success,
        message: msg,
    }
}

/// Flash-Vorgang mit probe-rs
pub fn flash_with_probe_rs(firmware_path: &str) -> String {
    use probe_rs::flashing::DownloadOptions;
    use probe_rs::{Session, SessionConfig, config::TargetSelector};
    use std::fs;
    match (|| -> anyhow::Result<String> {
        let firmware = fs::read(firmware_path)?;
        let mut session = Session::auto_attach(TargetSelector::Auto, SessionConfig::default())?;
        let mut loader = session.target().flash_loader();
        loader.add_data(0x0800_0000, &firmware)?;
        loader.commit(&mut session, DownloadOptions::default())?;
        Ok("Flashen mit probe-rs erfolgreich!".to_string())
    })() {
        Ok(msg) => msg,
        Err(e) => format!("Fehler beim Flashen mit probe-rs: {}", e),
    }
}

/// Ruft die benötigten Daten für das Flashen ab
pub fn fetch_flash_data() -> Result<String, String> {
    // TODO: Implementierung des Datenabrufs
    Ok("Daten abgerufen (Platzhalter)".to_string())
}
use std::sync::mpsc::Sender;

pub enum DownloadMsg {
    Progress(usize),
    Done(String),
    Error(String),
}

pub struct FirmwareDownloadHandle {
    pub rx: Receiver<DownloadMsg>,
}

impl FirmwareDownloadHandle {
    pub fn start(repo: String, tag: String, asset: String) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let tx_progress = tx.clone();
            let res = crate::flash::download_github_asset_progress_gui(
                &repo,
                &tag,
                &asset,
                move |percent| {
                    let _ = tx_progress.send(DownloadMsg::Progress(percent));
                },
            );
            match res {
                Ok(path) => {
                    let _ = tx.send(DownloadMsg::Done(path.display().to_string()));
                }
                Err(e) => {
                    let _ = tx.send(DownloadMsg::Error(format!("Fehler: {}", e)));
                }
            }
        });
        FirmwareDownloadHandle { rx }
    }
}
