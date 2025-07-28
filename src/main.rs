use cursive::Cursive;
use cursive::CursiveExt;
use cursive::menu::Tree;
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
            let repo = hardware.repo();
            match fetch_releases(repo) {
                Ok(releases) => {
                    if releases.is_empty() {
                        let hw_select = show_hardware_select as fn(&mut Cursive);
                        siv.add_layer(
                            Dialog::text("Keine passende Software für dieses Gerät gefunden.")
                                .title("Info")
                                .button("OK", move |s| {
                                    s.pop_layer();
                                    hw_select(s);
                                }),
                        );
                    } else {
                        let mut release_select =
                            SelectView::<String>::new().on_submit(|siv, release_tag: &String| {
                                siv.pop_layer();
                                // Finde das Release-Objekt zur gewählten Version
                                let releases = match fetch_releases(repo) {
                                    Ok(r) => r,
                                    Err(_) => {
                                        siv.add_layer(Dialog::info(
                                            "Fehler beim erneuten Laden der Releases.",
                                        ));
                                        return;
                                    }
                                };
                                let selected_release =
                                    releases.into_iter().find(|r| &r.tag_name == release_tag);
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
                                    let mut hw_select = SelectView::<String>::new().on_submit(
                                        |siv, hw: &String| {
                                            siv.pop_layer();
                                            siv.add_layer(
                                                Dialog::text(format!(
                                                    "Sie haben die Hardware-Version '{}' gewählt.",
                                                    hw
                                                ))
                                                .title("Hardware-Version gewählt")
                                                .button("Beenden", |s| s.quit()),
                                            );
                                        },
                                    );
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
                        siv.add_layer(
                            Dialog::around(release_select).title("Software-Version auswählen"),
                        );
                    }
                }
                Err(e) => {
                    siv.add_layer(Dialog::info(format!(
                        "Fehler beim Laden der Releases: {}",
                        e
                    )));
                }
            }
        });
        for hardware in HardwareType::all() {
            hardware_select.add_item(hardware.to_string(), *hardware);
        }
        siv.add_layer(Dialog::around(hardware_select).title("Hardware-Typ auswählen"));
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
