#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod icon;
mod service;
mod store;
mod sync;
mod tray;

use eframe::egui;
use std::io::Read;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // `hmb --snippet [text…] [--source name]`: store a snippet and exit.
    // Without text the snippet is read from stdin (used by the right-click service).
    if args.iter().any(|a| a == "--snippet") {
        std::process::exit(snippet_cli(&args));
    }

    // `hmb --sync` / `hmb --full-sync`: sync with the sync folder once, without a window.
    let full = args.iter().any(|a| a == "--full-sync");
    if full || args.iter().any(|a| a == "--sync") {
        let mut cfg = store::load_config();
        let r = if full { sync::full_sync(&cfg) } else { sync::sync(&cfg) };
        match r {
            Ok(r) => {
                println!("{}", r.log.join("\n"));
                for e in &r.imported {
                    println!("  {} {} ({})", e.action, e.path, e.client);
                }
                cfg.last_tx = r.last_tx;
                cfg.last_tx_files = r.last_tx_files;
                cfg.last_sync = Some(chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false));
                let _ = store::save_config(&cfg);
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Synchronisieren fehlgeschlagen: {e}");
                std::process::exit(1);
            }
        }
    }

    let screenshot = args
        .iter()
        .position(|a| a == "--screenshot")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("hmb – Hugo Microblogger")
        .with_icon(std::sync::Arc::new(icon::app_icon()))
        .with_inner_size([1320.0, 820.0])
        .with_min_inner_size([960.0, 560.0])
        .with_drag_and_drop(true);
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions {
        viewport,
        run_and_return: false,
        ..Default::default()
    };
    #[cfg(target_os = "macos")]
    if screenshot.is_some() {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }

    eframe::run_native(
        "hmb",
        options,
        Box::new(move |cc| Ok(Box::new(app::HmbApp::new(cc, screenshot)))),
    )
}

fn snippet_cli(args: &[String]) -> i32 {
    let mut source = String::from("Kommandozeile");
    let mut words = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if i == 0 || a == "--snippet" {
        } else if a == "--source" {
            if let Some(s) = args.get(i + 1) {
                source = s.clone();
            }
            i += 1;
        } else {
            words.push(a.clone());
        }
        i += 1;
    }
    let text = if words.is_empty() {
        let mut s = String::new();
        let _ = std::io::stdin().read_to_string(&mut s);
        s
    } else {
        words.join(" ")
    };
    let cfg = store::load_config();
    match store::add_snippet(&cfg.meta_dir(), &text, &source) {
        Ok(()) => {
            println!("Snippet gespeichert in {}", store::snippets_path(&cfg.meta_dir()).display());
            0
        }
        Err(e) => {
            eprintln!("Snippet nicht gespeichert: {e}");
            1
        }
    }
}
