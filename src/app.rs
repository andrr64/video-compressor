use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Color32, CornerRadius, RichText, Stroke, Vec2};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::ffmpeg::command::calculate_bitrate;
use crate::ffmpeg::probe::probe_video;
use crate::ffmpeg::runner::start_compression_thread;
use crate::models::{
    AudioOption, CompressionSettings, ModeTab, ProcessStatus, ResolutionPreset, SpeedPreset,
    VideoCodec, VideoMetadata,
};

// Adobe-inspired dark interface palette
const COLOR_BG: Color32 = Color32::from_rgb(22, 22, 22);             // #161616
const COLOR_SURFACE: Color32 = Color32::from_rgb(34, 34, 34);        // #222222
const COLOR_SURFACE_HOVER: Color32 = Color32::from_rgb(45, 45, 45);  // #2d2d2d
const COLOR_BORDER: Color32 = Color32::from_rgb(56, 56, 56);         // #383838
const COLOR_ACCENT: Color32 = Color32::from_rgb(20, 115, 230);       // #1473e6 (Adobe Blue)
const COLOR_TEXT_PRIMARY: Color32 = Color32::from_rgb(232, 232, 232); // #e8e8e8
const COLOR_TEXT_MUTED: Color32 = Color32::from_rgb(158, 158, 158);   // #9e9e9e
const COLOR_TEXT_SUBTLE: Color32 = Color32::from_rgb(117, 117, 117); // #757575
const COLOR_SUCCESS: Color32 = Color32::from_rgb(46, 160, 67);       // #2ea043
const COLOR_WARNING: Color32 = Color32::from_rgb(217, 119, 6);       // #d97706
const COLOR_ERROR: Color32 = Color32::from_rgb(220, 38, 38);         // #dc2626

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveView {
    Compressor,
    About,
}

pub struct VideoCompressorApp {
    // Navigation
    pub active_view: ActiveView,

    // Current input & probed metadata
    pub input_path: Option<PathBuf>,
    pub metadata: Option<VideoMetadata>,
    pub is_probing: bool,
    pub probe_error: Option<String>,

    // Compression settings
    pub mode: ModeTab,
    pub target_mb: f64,
    pub target_size_resolution: Option<ResolutionPreset>,
    pub res_preset: ResolutionPreset,
    pub crf: u32,
    pub codec: VideoCodec,
    pub speed: SpeedPreset,
    pub audio: AudioOption,
    pub output_path: Option<PathBuf>,
    pub custom_output_name: bool,

    // Background process management
    pub status_rx: Receiver<ProcessStatus>,
    pub status_tx: Sender<ProcessStatus>,
    pub cancel_flag: Arc<AtomicBool>,
    pub current_status: ProcessStatus,

    // UI state
    pub show_advanced: bool,
    pub visuals_initialized: bool,
    pub ffmpeg_available: bool,
}

impl Default for VideoCompressorApp {
    fn default() -> Self {
        let (status_tx, status_rx) = crossbeam_channel::unbounded();
        let ffmpeg_available = std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        Self {
            active_view: ActiveView::Compressor,

            input_path: None,
            metadata: None,
            is_probing: false,
            probe_error: None,

            mode: ModeTab::TargetSize,
            target_mb: 25.0,
            target_size_resolution: None,
            res_preset: ResolutionPreset::Res720p,
            crf: 23,
            codec: VideoCodec::H264,
            speed: SpeedPreset::Fast,
            audio: AudioOption::Aac128k,
            output_path: None,
            custom_output_name: false,

            status_rx,
            status_tx,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            current_status: ProcessStatus::Idle,

            show_advanced: false,
            visuals_initialized: false,
            ffmpeg_available,
        }
    }
}

impl VideoCompressorApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    fn apply_theme(&mut self, ctx: &egui::Context) {
        if self.visuals_initialized {
            return;
        }

        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(COLOR_TEXT_PRIMARY);
        visuals.panel_fill = COLOR_BG;
        visuals.window_fill = COLOR_SURFACE;

        // Adobe-style tight 3px radius and crisp 1px borders
        visuals.widgets.noninteractive.bg_fill = COLOR_SURFACE;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, COLOR_BORDER);
        visuals.widgets.noninteractive.corner_radius = CornerRadius::same(3);

        visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 40, 40);
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, COLOR_BORDER);
        visuals.widgets.inactive.corner_radius = CornerRadius::same(3);

        visuals.widgets.hovered.bg_fill = COLOR_SURFACE_HOVER;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, COLOR_ACCENT);
        visuals.widgets.hovered.corner_radius = CornerRadius::same(3);

        visuals.widgets.active.bg_fill = COLOR_ACCENT;
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::WHITE);
        visuals.widgets.active.corner_radius = CornerRadius::same(3);

        visuals.selection.bg_fill = COLOR_ACCENT;
        visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);

        ctx.set_visuals(visuals);
        self.visuals_initialized = true;
    }

    fn select_input_file(&mut self, path: PathBuf) {
        self.input_path = Some(path.clone());
        self.metadata = None;
        self.probe_error = None;
        self.is_probing = true;
        self.current_status = ProcessStatus::Idle;

        if !self.custom_output_name {
            if let Some(parent) = path.parent() {
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "output".to_string());
                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or_else(|| "mp4".to_string());
                self.output_path = Some(parent.join(format!("{stem}_compressed.{ext}")));
            }
        }

        match probe_video(&self.input_path.as_ref().unwrap()) {
            Ok(meta) => {
                let orig_mb = meta.size_mb();
                if orig_mb < self.target_mb && orig_mb > 1.0 {
                    self.target_mb = (orig_mb * 0.5).round().max(1.0);
                }
                self.metadata = Some(meta);
                self.is_probing = false;
            }
            Err(e) => {
                self.probe_error = Some(e.to_string());
                self.is_probing = false;
            }
        }
    }

    fn start_compression(&mut self) {
        let meta = match &self.metadata {
            Some(m) => m.clone(),
            None => return,
        };

        let in_path = match &self.input_path {
            Some(p) => p.clone(),
            None => return,
        };

        let out_path = match &self.output_path {
            Some(p) => p.clone(),
            None => {
                let p = in_path.with_file_name("compressed_output.mp4");
                self.output_path = Some(p.clone());
                p
            }
        };

        let settings = CompressionSettings {
            input_path: in_path,
            output_path: out_path,
            mode: self.mode,
            target_mb: self.target_mb,
            target_size_resolution: self.target_size_resolution,
            res_preset: self.res_preset,
            crf: self.crf,
            codec: self.codec,
            speed: self.speed,
            audio: self.audio,
        };

        self.cancel_flag = Arc::new(AtomicBool::new(false));
        self.current_status = ProcessStatus::Encoding {
            pass: 1,
            total_passes: match self.mode {
                ModeTab::TargetSize => 2,
                ModeTab::Resolution => 1,
            },
            percent: 0.0,
            current_time_secs: 0.0,
            speed: "0.0x".into(),
            fps: 0.0,
            eta_secs: None,
        };

        start_compression_thread(
            settings,
            meta,
            self.status_tx.clone(),
            self.cancel_flag.clone(),
        );
    }

    fn cancel_compression(&mut self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
    }

    fn is_running(&self) -> bool {
        matches!(
            self.current_status,
            ProcessStatus::Encoding { .. } | ProcessStatus::Probing
        )
    }
}

impl eframe::App for VideoCompressorApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_theme(ctx);

        while let Ok(new_status) = self.status_rx.try_recv() {
            self.current_status = new_status;
        }

        if self.is_running() {
            ctx.request_repaint();
        }

        ctx.input(|i| {
            if let Some(file) = i.raw.dropped_files.first() {
                let path = file.path();
                if !path.as_os_str().is_empty() {
                    self.select_input_file(path.to_path_buf());
                }
            }
        });
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            // --- TOP NAVIGATION BAR ---
            self.render_top_bar(ui);

            if !self.ffmpeg_available {
                ui.add_space(6.0);
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(38, 22, 22))
                    .stroke(Stroke::new(1.0, COLOR_ERROR))
                    .corner_radius(3)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("FFmpeg is missing from PATH.").strong().color(COLOR_ERROR));
                            ui.label(
                                RichText::new("Install with: winget install ffmpeg (Windows) or brew install ffmpeg (macOS)")
                                    .color(COLOR_TEXT_PRIMARY)
                                    .size(11.5),
                            );
                        });
                    });
            }

            ui.add_space(8.0);

            match self.active_view {
                ActiveView::Compressor => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        // Source file panel
                        self.render_source_panel(ui);

                        ui.add_space(10.0);

                        // Compression strategy panel
                        self.render_strategy_panel(ui);

                        ui.add_space(10.0);

                        // Encoder parameters
                        self.render_parameters_panel(ui);

                        ui.add_space(10.0);

                        // Destination file panel
                        self.render_output_panel(ui);

                        ui.add_space(12.0);

                        // Action and progress status
                        self.render_action_and_status(ui);

                        ui.add_space(8.0);
                    });
                }
                ActiveView::About => {
                    self.render_about_view(ui);
                }
            }
        });
    }
}

impl VideoCompressorApp {
    fn render_top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Video Compressor")
                    .size(16.0)
                    .strong()
                    .color(COLOR_TEXT_PRIMARY),
            );

            ui.add_space(16.0);

            // Tab navigation
            let comp_btn = ui.selectable_label(
                self.active_view == ActiveView::Compressor,
                RichText::new("Compressor").size(13.0),
            );
            if comp_btn.clicked() {
                self.active_view = ActiveView::Compressor;
            }

            let about_btn = ui.selectable_label(
                self.active_view == ActiveView::About,
                RichText::new("About").size(13.0),
            );
            if about_btn.clicked() {
                self.active_view = ActiveView::About;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("v0.1.0")
                        .size(11.0)
                        .monospace()
                        .color(COLOR_TEXT_SUBTLE),
                );
            });
        });

        ui.add_space(4.0);
        ui.separator();
    }

    fn render_source_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .fill(COLOR_SURFACE)
            .stroke(Stroke::new(1.0, COLOR_BORDER))
            .corner_radius(3)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Browse Video...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter(
                                "Video Files",
                                &["mp4", "mkv", "mov", "avi", "webm", "flv", "m4v", "wmv"],
                            )
                            .pick_file()
                        {
                            self.select_input_file(path);
                        }
                    }

                    if let Some(ref path) = self.input_path {
                        ui.label(
                            RichText::new(path.file_name().unwrap_or_default().to_string_lossy())
                                .strong()
                                .color(COLOR_TEXT_PRIMARY),
                        );
                    } else {
                        ui.label(
                            RichText::new("Drag and drop video file into this window to start")
                                .color(COLOR_TEXT_MUTED),
                        );
                    }
                });

                if let Some(ref meta) = self.metadata {
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    egui::Grid::new("source_meta_grid")
                        .num_columns(4)
                        .spacing([24.0, 5.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("Duration").color(COLOR_TEXT_MUTED));
                            ui.label(RichText::new(meta.duration_formatted()).strong());

                            ui.label(RichText::new("Resolution").color(COLOR_TEXT_MUTED));
                            ui.label(RichText::new(format!("{} × {}", meta.width, meta.height)).strong());
                            ui.end_row();

                            ui.label(RichText::new("Source Size").color(COLOR_TEXT_MUTED));
                            ui.label(RichText::new(format!("{:.2} MB", meta.size_mb())).strong());

                            ui.label(RichText::new("Codecs").color(COLOR_TEXT_MUTED));
                            let audio_str = meta.audio_codec.as_deref().unwrap_or("none");
                            ui.label(RichText::new(format!("{} / {}", meta.video_codec, audio_str)).strong());
                            ui.end_row();
                        });
                }

                if let Some(ref err) = self.probe_error {
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("Input error: {err}")).color(COLOR_ERROR));
                }
            });
    }

    fn render_strategy_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .fill(COLOR_SURFACE)
            .stroke(Stroke::new(1.0, COLOR_BORDER))
            .corner_radius(3)
            .inner_margin(10.0)
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Compression Mode")
                        .size(13.0)
                        .strong()
                        .color(COLOR_TEXT_PRIMARY),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.mode,
                        ModeTab::TargetSize,
                        RichText::new("Target File Size (2-Pass)").size(12.5),
                    );
                    ui.selectable_value(
                        &mut self.mode,
                        ModeTab::Resolution,
                        RichText::new("Resolution & Quality (CRF)").size(12.5),
                    );
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                match self.mode {
                    ModeTab::TargetSize => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Target Presets:").color(COLOR_TEXT_MUTED));
                            let presets = [
                                (8.0, "8 MB (Discord)"),
                                (25.0, "25 MB (Email/WA)"),
                                (50.0, "50 MB"),
                                (100.0, "100 MB"),
                            ];

                            for (size, label) in presets {
                                let selected = (self.target_mb - size).abs() < 0.1;
                                if ui.selectable_label(selected, label).clicked() {
                                    self.target_mb = size;
                                }
                            }
                        });

                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Custom Budget:").color(COLOR_TEXT_MUTED));
                            ui.add(
                                egui::DragValue::new(&mut self.target_mb)
                                    .speed(0.5)
                                    .range(0.5..=5000.0)
                                    .suffix(" MB"),
                            );
                        });

                        if let Some(ref meta) = self.metadata {
                            let bitrates = calculate_bitrate(self.target_mb, meta.duration_secs, self.audio);
                            ui.add_space(4.0);

                            ui.label(
                                RichText::new(format!(
                                    "Allocated: Video {} kbps | Audio {} kbps | Total {} kbps",
                                    bitrates.video_kbps, bitrates.audio_kbps, bitrates.total_kbps
                                ))
                                .size(11.0)
                                .color(COLOR_ACCENT),
                            );

                            if bitrates.video_kbps < 200 {
                                ui.label(
                                    RichText::new(
                                        "Warning: Video bitrate under 200 kbps for this duration. Downscaling resolution recommended."
                                    )
                                    .size(11.0)
                                    .color(COLOR_WARNING),
                                );
                            }
                        }

                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Output Scale:").color(COLOR_TEXT_MUTED));

                            let mut current_idx = match self.target_size_resolution {
                                None => 0,
                                Some(ResolutionPreset::Res1080p) => 1,
                                Some(ResolutionPreset::Res720p) => 2,
                                Some(ResolutionPreset::Res480p) => 3,
                                Some(ResolutionPreset::Res360p) => 4,
                                Some(ResolutionPreset::Scale75) => 5,
                                Some(ResolutionPreset::Scale50) => 6,
                                _ => 0,
                            };

                            let options = [
                                "Keep Original",
                                "Scale to 1080p FHD",
                                "Scale to 720p HD",
                                "Scale to 480p SD",
                                "Scale to 360p",
                                "Scale to 75%",
                                "Scale to 50%",
                            ];

                            egui::ComboBox::from_id_salt("target_size_res_combo")
                                .selected_text(options[current_idx])
                                .show_ui(ui, |ui| {
                                    for (idx, label) in options.iter().enumerate() {
                                        if ui.selectable_value(&mut current_idx, idx, *label).clicked() {
                                            self.target_size_resolution = match idx {
                                                1 => Some(ResolutionPreset::Res1080p),
                                                2 => Some(ResolutionPreset::Res720p),
                                                3 => Some(ResolutionPreset::Res480p),
                                                4 => Some(ResolutionPreset::Res360p),
                                                5 => Some(ResolutionPreset::Scale75),
                                                6 => Some(ResolutionPreset::Scale50),
                                                _ => None,
                                            };
                                        }
                                    }
                                });
                        });
                    }

                    ModeTab::Resolution => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Target Resolution:").color(COLOR_TEXT_MUTED));
                            egui::ComboBox::from_id_salt("res_preset_combo")
                                .selected_text(self.res_preset.label())
                                .show_ui(ui, |ui| {
                                    for preset in ResolutionPreset::ALL {
                                        ui.selectable_value(&mut self.res_preset, preset, preset.label());
                                    }
                                });
                        });

                        ui.add_space(4.0);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Quality (CRF):").color(COLOR_TEXT_MUTED));
                            ui.add(egui::Slider::new(&mut self.crf, 16..=36).text("CRF"));
                        });

                        ui.label(
                            RichText::new(
                                "CRF 18-22: Visual lossless | CRF 23: Default balanced | CRF 28: High compression"
                            )
                            .size(11.0)
                            .color(COLOR_TEXT_SUBTLE),
                        );
                    }
                }
            });
    }

    fn render_parameters_panel(&mut self, ui: &mut egui::Ui) {
        ui.collapsing(
            RichText::new("Encoder Parameters").size(12.5).color(COLOR_TEXT_PRIMARY),
            |ui| {
                egui::Frame::group(ui.style())
                    .fill(COLOR_SURFACE)
                    .stroke(Stroke::new(1.0, COLOR_BORDER))
                    .corner_radius(3)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        egui::Grid::new("param_grid")
                            .num_columns(2)
                            .spacing([18.0, 6.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Video Codec:").color(COLOR_TEXT_MUTED));
                                egui::ComboBox::from_id_salt("codec_combo")
                                    .selected_text(self.codec.label())
                                    .show_ui(ui, |ui| {
                                        for c in VideoCodec::ALL {
                                            ui.selectable_value(&mut self.codec, c, c.label());
                                        }
                                    });
                                ui.end_row();

                                ui.label(RichText::new("Preset Speed:").color(COLOR_TEXT_MUTED));
                                egui::ComboBox::from_id_salt("speed_combo")
                                    .selected_text(self.speed.label())
                                    .show_ui(ui, |ui| {
                                        for s in SpeedPreset::ALL {
                                            ui.selectable_value(&mut self.speed, s, s.label());
                                        }
                                    });
                                ui.end_row();

                                ui.label(RichText::new("Audio Handling:").color(COLOR_TEXT_MUTED));
                                egui::ComboBox::from_id_salt("audio_combo")
                                    .selected_text(self.audio.label())
                                    .show_ui(ui, |ui| {
                                        for a in AudioOption::ALL {
                                            ui.selectable_value(&mut self.audio, a, a.label());
                                        }
                                    });
                                ui.end_row();
                            });
                    });
            },
        );
    }

    fn render_output_panel(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .fill(COLOR_SURFACE)
            .stroke(Stroke::new(1.0, COLOR_BORDER))
            .corner_radius(3)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Destination:").color(COLOR_TEXT_MUTED));

                    if let Some(ref path) = self.output_path {
                        ui.monospace(
                            RichText::new(path.to_string_lossy())
                                .color(COLOR_TEXT_PRIMARY)
                                .size(11.0),
                        );
                    } else {
                        ui.label(
                            RichText::new("Default: <filename>_compressed.mp4")
                                .color(COLOR_TEXT_SUBTLE),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Select...").clicked() {
                            let mut dialog = rfd::FileDialog::new()
                                .add_filter("MP4 Video", &["mp4"])
                                .add_filter("MKV Video", &["mkv"]);

                            if let Some(ref current) = self.output_path {
                                if let Some(parent) = current.parent() {
                                    dialog = dialog.set_directory(parent);
                                }
                            }

                            if let Some(dest) = dialog.save_file() {
                                self.output_path = Some(dest);
                                self.custom_output_name = true;
                            }
                        }
                    });
                });
            });
    }

    fn render_action_and_status(&mut self, ui: &mut egui::Ui) {
        let is_running = self.is_running();
        let has_video = self.metadata.is_some();

        ui.horizontal(|ui| {
            if !is_running {
                let btn = egui::Button::new(
                    RichText::new("Start Queue")
                        .size(13.5)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(COLOR_ACCENT)
                .min_size(Vec2::new(140.0, 32.0));

                if ui.add_enabled(has_video, btn).clicked() {
                    self.start_compression();
                }
            } else {
                let cancel_btn = egui::Button::new(
                    RichText::new("Cancel")
                        .size(13.5)
                        .strong()
                        .color(Color32::WHITE),
                )
                .fill(COLOR_ERROR)
                .min_size(Vec2::new(100.0, 32.0));

                if ui.add(cancel_btn).clicked() {
                    self.cancel_compression();
                }
            }
        });

        ui.add_space(8.0);

        match &self.current_status {
            ProcessStatus::Idle => {}
            ProcessStatus::Probing => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Reading media streams...").color(COLOR_TEXT_MUTED));
                });
            }
            ProcessStatus::Encoding {
                pass,
                total_passes,
                percent,
                current_time_secs,
                speed,
                fps,
                eta_secs,
            } => {
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(26, 26, 26))
                    .stroke(Stroke::new(1.0, COLOR_ACCENT))
                    .corner_radius(3)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let pass_label = if *total_passes > 1 {
                                format!("Pass {} of {}: ", pass, total_passes)
                            } else {
                                String::new()
                            };
                            let action = if *pass == 1 && *total_passes > 1 {
                                "Analyzing Bitrate Profile"
                            } else {
                                "Encoding Video Stream"
                            };

                            ui.label(
                                RichText::new(format!("{pass_label}{action}"))
                                    .strong()
                                    .color(COLOR_TEXT_PRIMARY),
                            );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let eta_str = match eta_secs {
                                    Some(secs) => format!("ETA {:02}:{:02}", secs / 60, secs % 60),
                                    None => "ETA Calculating...".to_string(),
                                };
                                ui.label(
                                    RichText::new(eta_str)
                                        .monospace()
                                        .size(11.5)
                                        .color(COLOR_TEXT_MUTED),
                                );
                            });
                        });

                        ui.add_space(5.0);

                        ui.add(
                            egui::ProgressBar::new(*percent)
                                .show_percentage()
                                .animate(true)
                                .fill(COLOR_ACCENT),
                        );

                        ui.add_space(5.0);

                        ui.horizontal(|ui| {
                            let processed_secs = *current_time_secs as u64;
                            ui.label(
                                RichText::new(format!(
                                    "Processed: {:02}:{:02}",
                                    processed_secs / 60,
                                    processed_secs % 60
                                ))
                                .size(11.0)
                                .color(COLOR_TEXT_MUTED),
                            );
                            ui.separator();
                            ui.label(
                                RichText::new(format!("Speed: {speed}"))
                                    .size(11.0)
                                    .color(COLOR_TEXT_MUTED),
                            );
                            ui.separator();
                            ui.label(
                                RichText::new(format!("FPS: {:.1}", fps))
                                    .size(11.0)
                                    .color(COLOR_TEXT_MUTED),
                            );
                        });
                    });
            }
            ProcessStatus::Success {
                output_path,
                original_bytes,
                output_bytes,
                elapsed_secs,
            } => {
                let orig_mb = *original_bytes as f64 / (1024.0 * 1024.0);
                let out_mb = *output_bytes as f64 / (1024.0 * 1024.0);
                let saved_mb = (orig_mb - out_mb).max(0.0);
                let percent_saved = if orig_mb > 0.0 {
                    (saved_mb / orig_mb) * 100.0
                } else {
                    0.0
                };

                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(20, 32, 24))
                    .stroke(Stroke::new(1.0, COLOR_SUCCESS))
                    .corner_radius(3)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Encoding Complete")
                                .strong()
                                .size(14.0)
                                .color(COLOR_SUCCESS),
                        );

                        ui.add_space(3.0);
                        ui.label(
                            RichText::new(format!(
                                "Original: {:.2} MB   Output: {:.2} MB   (Saved {:.2} MB, {:.1}%)",
                                orig_mb, out_mb, saved_mb, percent_saved
                            ))
                            .color(COLOR_TEXT_PRIMARY),
                        );
                        ui.label(
                            RichText::new(format!("Elapsed time: {:.1} seconds", elapsed_secs))
                                .size(11.0)
                                .color(COLOR_TEXT_MUTED),
                        );

                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("Open Folder").clicked() {
                                open_file_folder(output_path);
                            }
                            if ui.button("Play File").clicked() {
                                open_video_file(output_path);
                            }
                        });
                    });
            }
            ProcessStatus::Cancelled => {
                egui::Frame::group(ui.style())
                    .fill(COLOR_SURFACE)
                    .stroke(Stroke::new(1.0, COLOR_WARNING))
                    .corner_radius(3)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("Encoding cancelled by user.")
                                .color(COLOR_WARNING),
                        );
                    });
            }
            ProcessStatus::Error(err) => {
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(38, 22, 22))
                    .stroke(Stroke::new(1.0, COLOR_ERROR))
                    .corner_radius(3)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new("Encoding Error").strong().color(COLOR_ERROR));
                        ui.add_space(3.0);
                        ui.monospace(RichText::new(err).size(10.5).color(COLOR_TEXT_PRIMARY));
                    });
            }
        }
    }

    fn render_about_view(&self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .fill(COLOR_SURFACE)
            .stroke(Stroke::new(1.0, COLOR_BORDER))
            .corner_radius(3)
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Video Compressor")
                        .size(18.0)
                        .strong()
                        .color(COLOR_TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new("Native desktop utility for targeted bitrate allocation and resolution scaling")
                        .size(12.0)
                        .color(COLOR_TEXT_MUTED),
                );

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);

                ui.label(RichText::new("Technical Specifications").strong().color(COLOR_TEXT_PRIMARY));
                ui.add_space(6.0);

                egui::Grid::new("about_grid")
                    .num_columns(2)
                    .spacing([24.0, 8.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Version").color(COLOR_TEXT_MUTED));
                        ui.label(RichText::new("0.1.0").monospace());
                        ui.end_row();

                        ui.label(RichText::new("Media Engine").color(COLOR_TEXT_MUTED));
                        ui.label(RichText::new("FFmpeg 8.x (libavcodec, libavformat)").monospace());
                        ui.end_row();

                        ui.label(RichText::new("UI Framework").color(COLOR_TEXT_MUTED));
                        ui.label(RichText::new("egui / eframe (hardware-accelerated immediate mode)").monospace());
                        ui.end_row();

                        ui.label(RichText::new("Target OS").color(COLOR_TEXT_MUTED));
                        ui.label(RichText::new("Windows x86_64").monospace());
                        ui.end_row();

                        ui.label(RichText::new("Architecture").color(COLOR_TEXT_MUTED));
                        ui.label(RichText::new("Asynchronous multi-threaded worker pipeline").monospace());
                        ui.end_row();
                    });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(12.0);

                ui.label(RichText::new("Capabilities").strong().color(COLOR_TEXT_PRIMARY));
                ui.add_space(6.0);

                ui.label(
                    RichText::new(
                        "• 2-Pass variable bitrate encoding for precise file size budgets (Discord 8MB, Email 25MB, custom)\n\
                         • Constant Rate Factor (CRF) scaling across standard resolutions (4K to 360p)\n\
                         • Aspect-ratio preserved even-dimension scaling filter\n\
                         • AAC audio re-encoding and original stream pass-through"
                    )
                    .color(COLOR_TEXT_MUTED),
                );

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(12.0);

                ui.label(
                    RichText::new("Entropy Labs. Released under the MIT License.")
                        .size(11.0)
                        .color(COLOR_TEXT_SUBTLE),
                );
            });
    }
}

fn open_file_folder(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg("/select,")
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path.parent() {
            let _ = std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn();
        }
    }
}

fn open_video_file(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", ""])
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open")
            .arg(path)
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open")
            .arg(path)
            .spawn();
    }
}
