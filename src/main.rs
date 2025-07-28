use cursive::Cursive;
use cursive::CursiveExt;
use cursive::menu::Tree;
use cursive::view::Nameable;
use cursive::views::{Dialog, SelectView};
use serde::Deserialize;

mod self_update_mod;
use self_update_mod::{check_for_update, run_update_and_restart};
// ...

#[derive(Debug, Deserialize)]
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
    let mut siv = Cursive::default();

    // Menüleiste
    siv.menubar().add_subtree(
        "iRockProgrammer",
        Tree::new()
            .leaf("Über iRockProgrammer", |s| {
                let version = env!("CARGO_PKG_VERSION");
                let info = format!("Version: {}\n", version);
                s.add_layer(Dialog::text(info).title("Info").button("OK", |s| {
                    s.pop_layer();
                }))
            })
            .delimiter()
            .leaf("iRockProgrammer aktualisieren", |s| check_for_update(s))
            .delimiter()
            .leaf("iRockProgrammer beenden", |s| s.quit()),
    );
    siv.set_autohide_menu(false); // Menüleiste immer anzeigen
    siv.add_global_callback(cursive::event::Key::Esc, |s| s.select_menubar());

    // Hardware-Auswahl-Dialog als Funktion, damit wir ihn nach OK anzeigen können
    fn show_hardware_select(siv: &mut Cursive) {
        let mut hardware_select = SelectView::<HardwareType>::new().on_submit(|siv, hardware| {
            siv.pop_layer();
            use cursive::views::{Dialog, LinearLayout, TextView, ProgressBar};
            use cursive::utils::Counter;
            let counter = Counter::new(0);
            siv.add_layer(
                Dialog::around(
                    LinearLayout::vertical()
                        .child(TextView::new("Lade Release-Informationen ..."))
                        .child(ProgressBar::new().with_value(counter.clone()).max(100).with_name("release_progress"))
                ).title("Lade Releases")
            );
            let cb_sink = siv.cb_sink().clone();
            let repo = hardware.repo().to_string();
            std::thread::spawn(move || {
                // Simulierter Fortschritt, da fetch_releases synchron ist und keine Teilfortschritte liefert
                let cb_sink_progress = cb_sink.clone();
                let counter_clone = counter.clone();
                std::thread::spawn(move || {
                    for i in 1..=100 {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        let c = counter_clone.clone();
                        let cb = cb_sink_progress.clone();
                        let _ = cb.send(Box::new(move |_s: &mut Cursive| {
                            c.set(i);
                        }));
                    }
                });
                let result = fetch_releases(&repo);
                let cb_sink_done = cb_sink.clone();
                cb_sink_done.send(Box::new(move |s: &mut Cursive| {
                    s.pop_layer();
                    match result {
                        Ok(releases) => {
                            if releases.is_empty() {
                                let hw_select = show_hardware_select as fn(&mut Cursive);
                                s.add_layer(
                                    Dialog::text("Keine passende Software für dieses Gerät gefunden.")
                                        .title("Info")
                                        .button("OK", move |s| {
                                            s.pop_layer();
                                            hw_select(s);
                                        }),
                                );
                            } else {
                                let repo = std::sync::Arc::new(repo.to_string());
                                let mut release_select = SelectView::<String>::new().on_submit({
                                    let repo = std::sync::Arc::clone(&repo);
                                    move |siv, release_tag: &String| {
                                        siv.pop_layer();
                                        // Finde das Release-Objekt zur gewählten Version
                                        let releases = match fetch_releases(&repo) {
                                            Ok(r) => r,
                                            Err(_) => {
                                                siv.add_layer(Dialog::info(
                                                    "Fehler beim erneuten Laden der Releases.",
                                                ));
                                                return;
                                            }
                                        };
                                        let selected_release = releases.into_iter().find(|r| &r.tag_name == release_tag);
                                        if let Some(release) = selected_release {
                                            // Extrahiere Hardwareversionen aus Asset-Namen mit match
                                            let mut hw_versions: Vec<String> = Vec::new();
                                            for asset in &release.stm32_assets {
                                                match asset.rsplit_once('.') {
                                                    Some((name, "bin")) | Some((name, "hex")) => {
                                                        if let Some((_, hw)) = name.rsplit_once('-') {
                                                            hw_versions.push(hw.to_string());
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            hw_versions.sort();
                                            hw_versions.dedup();
                                            if hw_versions.is_empty() {
                                                siv.add_layer(Dialog::info(
                                                    "Keine Hardware-Versionen in den Assets gefunden.",
                                                ));
                                                return;
                                            }
                                            let repo = std::sync::Arc::clone(&repo);
                                            let mut hw_select = SelectView::<String>::new().on_submit({
                                                let repo = std::sync::Arc::clone(&repo);
                                                move |siv, hw: &String| {
                                                    siv.pop_layer();
                                                    // Asset-Name für die gewählte Hardware-Version finden
                                                    let asset_name = release.stm32_assets.iter().find(|asset| {
                                                        asset.contains(hw)
                                                    });
                                                    if let Some(asset_name) = asset_name {
                                                        // Download-URL für das Asset holen
                                                        let asset_name = asset_name.clone();
                                                        let repo = std::sync::Arc::clone(&repo);
                                                        let tag = release.tag_name.clone();
                                                        use cursive::views::{ProgressBar, LinearLayout, TextView};
                                                        let cb_sink = siv.cb_sink().clone();
                                                        use cursive::utils::Counter;
                                                        let counter = Counter::new(0);
                                                        siv.add_layer(
                                                            Dialog::around(
                                                                LinearLayout::vertical()
                                                                    .child(TextView::new("Lade Asset herunter ..."))
                                                                    .child(ProgressBar::new().with_value(counter.clone()).max(100).with_name("download_progress"))
                                                            ).title("Download")
                                                        );
                                                        std::thread::spawn(move || {
                                                            let counter_clone = counter.clone();
                                                            let cb_sink_progress = cb_sink.clone();
                                                            let result = download_github_asset_progress(&repo, &tag, &asset_name, move |current, total| {
                                                                if total > 0 {
                                                                    let percent = (current as f32 / total as f32 * 100.0) as usize;
                                                                    let c = counter_clone.clone();
                                                                    let cb = cb_sink_progress.clone();
                                                                    let _ = cb.send(Box::new(move |_s: &mut Cursive| {
                                                                        c.set(percent);
                                                                    }));
                                                                }
                                                            });
                                                            let cb_sink_done = cb_sink.clone();
                                                            cb_sink_done.send(Box::new(move |s: &mut Cursive| {
                                                                s.pop_layer();
                                                                match result {
                                                                    Ok(path) => {
                                                                let flash_path = path.display().to_string();
                                                                s.add_layer(
                                                                    Dialog::text(format!("Asset wurde heruntergeladen: {}\n\nJetzt auf STM32 flashen?", flash_path))
                                                                        .button("Flashen", move |siv| {
                                                                            siv.pop_layer();
                                                                            siv.add_layer(Dialog::text("Flashe Firmware ...").title("Flashen").with_name("flash_status"));
                                                                            let flash_path = flash_path.clone();
                                                                            let cb_sink = siv.cb_sink().clone();
                                                                            std::thread::spawn(move || {
                                                                                let output = std::process::Command::new("st-flash")
                                                                                    .arg("write")
                                                                                    .arg(&flash_path)
                                                                                    .arg("0x08000000")
                                                                                    .output();
                                                                                let msg = match output {
                                                                                    Ok(out) if out.status.success() => {
                                                                                        format!("Flash erfolgreich!\n\n{}", String::from_utf8_lossy(&out.stdout))
                                                                                    },
                                                                                    Ok(out) => {
                                                                                        format!("Fehler beim Flashen!\n\n{}", String::from_utf8_lossy(&out.stderr))
                                                                                    },
                                                                                    Err(e) => format!("Fehler beim Starten von st-flash: {}", e),
                                                                                };
                                                                                cb_sink.send(Box::new(move |s: &mut Cursive| {
                                                                                    s.call_on_name("flash_status", |v: &mut Dialog| {
                                                                                        use cursive::views::TextView;
                                                                                        v.set_content(TextView::new(msg.clone()));
                                                                                    });
                                                                                    // Button zum Beenden anbieten
                                                                                    s.add_layer(Dialog::text("Fertig!").button("Beenden", |s| s.quit()));
                                                                                })).ok();
                                                                            });
                                                                        })
                                                                        .button("Beenden", |s| s.quit())
                                                                );
                                                                    }
                                                                    Err(e) => {
                                                                        s.add_layer(Dialog::info(format!(
                                                                            "Fehler beim Herunterladen: {}",
                                                                            e
                                                                        )));
                                                                    }
                                                                }
                                                            })).ok();
                                                        });
                                                    } else {
                                                        siv.add_layer(Dialog::info("Kein passendes Asset gefunden."));
                                                    }
                                                }
                                            });
                                            for hw in hw_versions {
                                                hw_select.add_item(hw.clone(), hw);
                                            }
                                            siv.add_layer(
                                                Dialog::around(hw_select)
                                                    .title("Hardware-Version auswählen"),
                                            );
                                        } else {
                                            siv.add_layer(Dialog::info("Release nicht gefunden."));
                                        }
                                    }
                                });
                                for release in releases {
                                    let display = format!(
                                        "{}{}",
                                        release.tag_name,
                                        if release.prerelease {
                                            " (pre-release)"
                                        } else {
                                            ""
                                        }
                                    );
                                    release_select.add_item(display, release.tag_name.clone());
                                }
                                s.add_layer(
                                    Dialog::around(release_select).title("Software-Version auswählen"),
                                );
                            }
                        }
                        Err(e) => {
                            s.add_layer(Dialog::info(format!(
                                "Fehler beim Laden der Releases: {}",
                                e
                            )));
                        }
                    }
                })).ok();
            });
        });
        for hardware in HardwareType::all() {
            hardware_select.add_item(hardware.to_string(), *hardware);
        }
        siv.add_layer(Dialog::around(hardware_select).title("Hardware-Typ auswählen"));
    }

    // Hilfsfunktion: Asset von GitHub herunterladen und temporär speichern
    // Hilfsfunktion: Asset von GitHub herunterladen und temporär speichern, mit Fortschritt
    fn download_github_asset_progress<F>(
        repo: &str,
        tag: &str,
        asset_name: &str,
        mut progress_cb: F,
    ) -> Result<std::path::PathBuf, Box<dyn std::error::Error + Send + Sync>>
    where
        F: FnMut(u64, u64) + Send + 'static,
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
                progress_cb(downloaded, total);
            } else {
                break;
            }
        }
        // tempdir lebt nur solange wie das TempDir-Objekt, daher Pfad kopieren
        let final_path = std::env::temp_dir().join(asset_name);
        std::fs::copy(&file_path, &final_path)?;
        Ok(final_path)
    }
    // Zeige Restart-Info als Dialog, falls vorhanden, danach Hardware-Auswahl
    if let Ok(msg) = std::fs::read_to_string(".irock_restart_msg") {
        let _ = std::fs::remove_file(".irock_restart_msg");
        siv.add_layer(Dialog::text(msg.trim()).button("OK", |s| {
            s.pop_layer();
            show_hardware_select(s);
        }));
    } else {
        show_hardware_select(&mut siv);
    }

    siv.run();

    // Nach Beenden der TUI prüfen, ob ein Update gewünscht war
    if std::fs::metadata(".irock_update.flag").is_ok() {
        let _ = std::fs::remove_file(".irock_update.flag");
        std::process::exit(run_update_and_restart());
    }
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
