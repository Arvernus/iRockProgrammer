use cursive::Cursive;
use self_update::backends::github::Update;
use std::env;
use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;

pub fn check_for_update(siv: &mut Cursive) {
    // Schreibe Datei-Flag, um Update-Wunsch zu signalisieren
    let _ = std::fs::write(".irock_update.flag", b"1");
    siv.quit();
}

pub fn run_update_and_restart() -> i32 {
    // Terminal: Alternate Screen Buffer verlassen, Farben/Cursor/Bildschirm zurücksetzen
    println!("\x1b[?1049l\x1b[0m\x1b[2J\x1b[H");
    println!("\n=== Starting update... ===\n");
    let result: Result<bool, Box<dyn std::error::Error + Send + Sync>> = (|| {
        let status = Update::configure()
            .repo_owner("Arvernus")
            .repo_name("iRockProgrammer")
            .bin_name("iRockProgrammer")
            .show_download_progress(true)
            .current_version(env!("CARGO_PKG_VERSION"))
            .build()?
            .update()?;
        Ok(status.updated())
    })();

    let version = env!("CARGO_PKG_VERSION");
    let restart_msg = match result {
        Ok(true) => {
            println!("\n=== Update completed successfully! Restarting... ===\n");
            format!("Updated to version {}.", version)
        }
        Ok(false) => {
            println!("\n=== No update necessary. Restarting... ===\n");
            format!("Running current version {}.", version)
        }
        Err(e) => {
            eprintln!("\n=== Error during update: {} ===\n", e);
            return 1;
        }
    };
    // Write restart message to file
    let _ = std::fs::write(".irock_restart_msg", &restart_msg);
    let exe = env::current_exe().unwrap();
    let exe_cstr = CString::new(exe.as_os_str().as_bytes()).unwrap();
    let args: Vec<CString> = env::args().map(|a| CString::new(a).unwrap()).collect();
    let argv: Vec<&CStr> = args.iter().map(|a| a.as_c_str()).collect();
    let err = nix::unistd::execv(&exe_cstr, &argv);
    eprintln!("Error during restart: {:?}", err);
    1
}
