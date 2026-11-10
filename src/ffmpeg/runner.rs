use anyhow::Result;
use crossbeam_channel::Sender;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::models::{CompressionSettings, ProcessStatus, VideoMetadata};
use super::command::{build_ffmpeg_commands, FfmpegPass};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn start_compression_thread(
    settings: CompressionSettings,
    meta: VideoMetadata,
    status_sender: Sender<ProcessStatus>,
    cancel_flag: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let start_time = Instant::now();

        // Create a temp directory for 2-pass logs
        let temp_dir = match tempfile::tempdir() {
            Ok(dir) => dir,
            Err(e) => {
                let _ = status_sender.send(ProcessStatus::Error(format!(
                    "Failed to create temporary directory for encode logs: {e}"
                )));
                return;
            }
        };

        let passes = build_ffmpeg_commands(&settings, &meta, temp_dir.path());
        let total_passes = passes.len() as u8;

        for pass in passes {
            if cancel_flag.load(Ordering::Relaxed) {
                let _ = status_sender.send(ProcessStatus::Cancelled);
                return;
            }

            let result = run_single_pass(
                &pass,
                total_passes,
                meta.duration_secs,
                &status_sender,
                &cancel_flag,
            );

            match result {
                Ok(true) => {
                    // Pass completed successfully
                }
                Ok(false) => {
                    // Cancelled
                    let _ = status_sender.send(ProcessStatus::Cancelled);
                    // Remove partial output file if it exists
                    let _ = std::fs::remove_file(&settings.output_path);
                    return;
                }
                Err(err_msg) => {
                    let _ = status_sender.send(ProcessStatus::Error(err_msg));
                    let _ = std::fs::remove_file(&settings.output_path);
                    return;
                }
            }
        }

        // Verify output file
        if let Ok(file_meta) = std::fs::metadata(&settings.output_path) {
            let output_bytes = file_meta.len();
            let elapsed_secs = start_time.elapsed().as_secs_f64();
            let _ = status_sender.send(ProcessStatus::Success {
                output_path: settings.output_path,
                original_bytes: meta.size_bytes,
                output_bytes,
                elapsed_secs,
            });
        } else {
            let _ = status_sender.send(ProcessStatus::Error(
                "Encoding finished, but output file could not be found.".into(),
            ));
        }
    });
}

fn run_single_pass(
    pass: &FfmpegPass,
    total_passes: u8,
    total_duration_secs: f64,
    status_sender: &Sender<ProcessStatus>,
    cancel_flag: &Arc<AtomicBool>,
) -> Result<bool, String> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(&pass.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to launch ffmpeg: {e}"))?;

    let stdout = child.stdout.take().ok_or("Failed to capture ffmpeg stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture ffmpeg stderr")?;

    // Read stderr in background thread for debugging in case of error
    let stderr_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut last_lines = Vec::new();
        for line in reader.lines().map_while(Result::ok) {
            last_lines.push(line);
            if last_lines.len() > 30 {
                last_lines.remove(0);
            }
        }
        last_lines.join("\n")
    });

    let stdout_reader = BufReader::new(stdout);
    let mut current_speed = String::from("1.0x");
    let mut current_fps = 0.0f32;
    let mut current_out_time_secs = 0.0f64;

    for line in stdout_reader.lines().map_while(Result::ok) {
        if cancel_flag.load(Ordering::Relaxed) {
            let _ = child.kill();
            return Ok(false);
        }

        let line = line.trim();
        if let Some((key, val)) = line.split_once('=') {
            match key.trim() {
                "speed" => {
                    current_speed = val.trim().to_string();
                }
                "fps" => {
                    if let Ok(fps) = val.trim().parse::<f32>() {
                        current_fps = fps;
                    }
                }
                "out_time_us" | "out_time_ms" => {
                    // out_time_us is in microseconds
                    if let Ok(us) = val.trim().parse::<f64>() {
                        current_out_time_secs = us / 1_000_000.0;
                    }
                }
                "progress" => {
                    let pass_fraction = (current_out_time_secs / total_duration_secs.max(0.1))
                        .clamp(0.0, 1.0) as f32;

                    let total_percent = if total_passes > 1 {
                        let base = (pass.pass_number - 1) as f32 / total_passes as f32;
                        base + (pass_fraction / total_passes as f32)
                    } else {
                        pass_fraction
                    };

                    let eta_secs = calculate_eta(
                        current_out_time_secs,
                        total_duration_secs,
                        &current_speed,
                        pass.pass_number,
                        total_passes,
                    );

                    let _ = status_sender.send(ProcessStatus::Encoding {
                        pass: pass.pass_number,
                        total_passes,
                        percent: total_percent,
                        current_time_secs: current_out_time_secs,
                        speed: current_speed.clone(),
                        fps: current_fps,
                        eta_secs,
                    });
                }
                _ => {}
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait on ffmpeg: {e}"))?;

    let stderr_output = stderr_handle.join().unwrap_or_default();

    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(false);
    }

    if status.success() {
        Ok(true)
    } else {
        Err(format!(
            "FFmpeg exited with error code {:?}.\nDetails:\n{}",
            status.code(),
            stderr_output
        ))
    }
}

fn calculate_eta(
    current_time: f64,
    total_duration: f64,
    speed_str: &str,
    pass: u8,
    total_passes: u8,
) -> Option<u64> {
    let speed_num = speed_str
        .trim_end_matches('x')
        .trim()
        .parse::<f64>()
        .ok()?;

    if speed_num <= 0.05 {
        return None;
    }

    let remaining_pass_secs = (total_duration - current_time).max(0.0);
    let passes_left = (total_passes - pass) as f64;
    let total_remaining_video = remaining_pass_secs + (passes_left * total_duration);

    let eta_real_secs = total_remaining_video / speed_num;
    Some(eta_real_secs.round() as u64)
}
