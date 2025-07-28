use cursive::Cursive;
use cursive::CursiveExt;
use cursive::Printer;
use cursive::Vec2;
use cursive::menu::Tree;
use cursive::view::View;
use cursive::views::{Dialog, SelectView};
use serde::Deserialize;
use std::error::Error;

#[derive(Debug, Deserialize)]
struct Release {
    tag_name: String,
    name: Option<String>,
    prerelease: bool,
    draft: bool,
}

/// Ein View, das ein gegebenes ASCII-Art-Logo zentriert darstellt.
pub struct BackgroundView {
    ascii_art: String,
}

impl BackgroundView {
    pub fn new(ascii_art: String) -> Self {
        BackgroundView { ascii_art }
    }
}

impl View for BackgroundView {
    fn draw(&self, printer: &Printer) {
        let lines: Vec<&str> = self.ascii_art.lines().collect();
        let art_height = lines.len();
        let art_width = lines.iter().map(|line| line.len()).max().unwrap_or(0);
        let offset_x = (printer.size.x.saturating_sub(art_width)) / 2;
        let offset_y = (printer.size.y.saturating_sub(art_height)) / 2;
        for (i, line) in lines.iter().enumerate() {
            printer.print((offset_x, offset_y + i), line);
        }
    }
    fn required_size(&mut self, _constraints: Vec2) -> Vec2 {
        let lines: Vec<&str> = self.ascii_art.lines().collect();
        let height = lines.len();
        let width = lines.iter().map(|line| line.len()).max().unwrap_or(0);
        Vec2::new(width, height)
    }
}

// Menüpunkt-Callback für das Update
fn check_for_update(siv: &mut cursive::Cursive) {
    siv.add_layer(Dialog::info("Suche nach Updates..."));
    std::thread::spawn({
        let cb_sink = siv.cb_sink().clone();
        move || {
            let result = run_update();
            cb_sink
                .send(Box::new(move |s: &mut cursive::Cursive| {
                    match result {
                        Ok(status) => {
                            if status {
                                s.add_layer(Dialog::info("Update erfolgreich durchgeführt! Das Programm muss evtl. neu gestartet werden."));
                            } else {
                                s.add_layer(Dialog::info("Kein Update notwendig."));
                            }
                        }
                        Err(e) => {
                            s.add_layer(Dialog::info(format!("Fehler beim Update: {}", e)));
                        }
                    }
                }))
                .unwrap();
        }
    });
}

fn run_update() -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    use self_update::backends::github::Update;
    let status = Update::configure()
        .repo_owner("Arvernus")
        .repo_name("iRockProgrammer")
        .bin_name("iRockProgrammer")
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()?
        .update()?;
    Ok(status.updated())
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
    siv.menubar()
        .add_subtree(
            "Datei",
            Tree::new()
                .leaf("Programm aktualisieren", |s| check_for_update(s))
                .delimiter()
                .leaf("Beenden", |s| s.quit()),
        )
        .add_subtree(
            "Info",
            Tree::new().leaf("Über", |s| {
                s.add_layer(Dialog::info("iRockProgrammer v1.0\n(c) 2025 Joscha Wagner"))
            }),
        );
    siv.set_autohide_menu(false); // Menüleiste immer anzeigen
    siv.add_global_callback(cursive::event::Key::Esc, |s| s.select_menubar());

    let ascii_logo = r#" ################## 
####################
####            ####
####   .#####.   ###
####  .#######   ###
####  .#######   ###
####  ########   ###
#############.   ###
########.       ####
####################
 ################## "#
        .to_string();
    siv.add_fullscreen_layer(BackgroundView::new(ascii_logo));

    let mut hardware_select = SelectView::<HardwareType>::new().on_submit(|siv, hardware| {
        siv.pop_layer();
        let repo = hardware.repo();
        match fetch_releases(repo) {
            Ok(releases) => {
                let mut release_select =
                    SelectView::<String>::new().on_submit(|siv, release: &String| {
                        siv.pop_layer();
                        siv.add_layer(
                            Dialog::text(format!("Du hast Release '{}' gewählt.", release))
                                .title("Release Auswahl")
                                .button("OK", |s| s.quit()),
                        );
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
                    release_select.add_item(display, release.tag_name);
                }
                siv.add_layer(Dialog::around(release_select).title("Wähle die Software-Version"));
            }
            Err(e) => {
                siv.add_layer(Dialog::info(format!(
                    "Fehler beim Abrufen der Releases: {}",
                    e
                )));
            }
        }
    });

    for hardware in HardwareType::all() {
        hardware_select.add_item(hardware.to_string(), *hardware);
    }

    siv.add_layer(Dialog::around(hardware_select).title("Wähle den Hardware-Typ"));
    siv.run();
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

    let releases = response
        .items
        .into_iter()
        .map(|r| Release {
            tag_name: r.tag_name,
            name: r.name,
            prerelease: r.prerelease,
            draft: r.draft,
        })
        .collect();

    Ok(releases)
}
