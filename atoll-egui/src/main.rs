//! Atoll egui experiment — standalone binary, not the production Tauri app.
//!
//! Build:  cargo build --release
//! Run:    cargo run --release
//! Output: target/release/atoll-egui.exe  (collect separately from Atoll-release/)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod mock;

use app::AtollEguiApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([340.0, 56.0])
            .with_min_inner_size([280.0, 48.0])
            .with_decorations(false)
            .with_transparent(false)
            .with_always_on_top()
            .with_title("Atoll (egui experiment)"),
        ..Default::default()
    };

    eframe::run_native(
        "atoll-egui",
        options,
        Box::new(|_cc| Ok(Box::new(AtollEguiApp::default()))),
    )
}
