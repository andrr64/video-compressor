#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use video_compressor::app::VideoCompressorApp;

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([740.0, 760.0])
            .with_min_inner_size([640.0, 600.0])
            .with_title("Video Compressor"),
        ..Default::default()
    };

    eframe::run_native(
        "Video Compressor",
        native_options,
        Box::new(|cc| Ok(Box::new(VideoCompressorApp::new(cc)))),
    )
}
