use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use video_compressor::ffmpeg::probe::probe_video;
use video_compressor::ffmpeg::runner::start_compression_thread;
use video_compressor::models::{
    AudioOption, CompressionSettings, ModeTab, ProcessStatus, ResolutionPreset, SpeedPreset,
    VideoCodec,
};

fn generate_test_video(output_path: &PathBuf, duration_secs: u32) {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc=duration={duration_secs}:size=1280x720:rate=30"),
            "-f",
            "lavfi",
            "-i",
            &format!("sine=frequency=1000:duration={duration_secs}"),
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
        ])
        .arg(output_path)
        .status()
        .expect("Failed to execute ffmpeg to generate test video");

    assert!(status.success(), "Failed to generate test video");
}

#[test]
fn test_probe_and_resolution_compression() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let input_path = temp_dir.path().join("sample_input.mp4");
    let output_path = temp_dir.path().join("sample_output_480p.mp4");

    generate_test_video(&input_path, 3);

    // 1. Probe test
    let meta = probe_video(&input_path).expect("probe_video failed");
    assert_eq!(meta.width, 1280);
    assert_eq!(meta.height, 720);
    assert!((meta.duration_secs - 3.0).abs() < 0.5);
    assert_eq!(meta.video_codec, "h264");
    assert!(meta.audio_codec.is_some());

    // 2. Compress by resolution (scale down to 480p)
    let settings = CompressionSettings {
        input_path: input_path.clone(),
        output_path: output_path.clone(),
        mode: ModeTab::Resolution,
        target_mb: 1.0,
        target_size_resolution: None,
        res_preset: ResolutionPreset::Res480p,
        crf: 28,
        codec: VideoCodec::H264,
        speed: SpeedPreset::Ultrafast,
        audio: AudioOption::Aac64k,
    };

    let (tx, rx) = crossbeam_channel::unbounded();
    let cancel = Arc::new(AtomicBool::new(false));

    start_compression_thread(settings, meta, tx, cancel);

    let mut finished = false;
    while let Ok(status) = rx.recv_timeout(Duration::from_secs(30)) {
        match status {
            ProcessStatus::Success {
                output_path: out,
                output_bytes,
                ..
            } => {
                assert_eq!(out, output_path);
                assert!(output_bytes > 0);
                finished = true;
                break;
            }
            ProcessStatus::Error(err) => {
                panic!("Compression failed: {err}");
            }
            ProcessStatus::Cancelled => {
                panic!("Unexpected cancellation");
            }
            _ => {}
        }
    }

    assert!(finished, "Compression did not finish in time");
    assert!(output_path.exists());

    // Probe the output file to verify resolution was scaled down to 480p height
    let out_meta = probe_video(&output_path).expect("Failed to probe output video");
    assert_eq!(out_meta.height, 480);
}

#[test]
fn test_probe_and_target_size_compression_two_pass() {
    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let input_path = temp_dir.path().join("sample_input_twopass.mp4");
    let output_path = temp_dir.path().join("sample_output_twopass.mp4");

    generate_test_video(&input_path, 4);

    let meta = probe_video(&input_path).expect("probe_video failed");

    // Target size = 0.5 MB
    let settings = CompressionSettings {
        input_path: input_path.clone(),
        output_path: output_path.clone(),
        mode: ModeTab::TargetSize,
        target_mb: 0.5,
        target_size_resolution: Some(ResolutionPreset::Res480p),
        res_preset: ResolutionPreset::Res720p,
        crf: 23,
        codec: VideoCodec::H264,
        speed: SpeedPreset::Ultrafast,
        audio: AudioOption::Aac64k,
    };

    let (tx, rx) = crossbeam_channel::unbounded();
    let cancel = Arc::new(AtomicBool::new(false));

    start_compression_thread(settings, meta, tx, cancel);

    let mut saw_pass_1 = false;
    let mut saw_pass_2 = false;
    let mut finished = false;

    while let Ok(status) = rx.recv_timeout(Duration::from_secs(30)) {
        match status {
            ProcessStatus::Encoding { pass, .. } => {
                if pass == 1 {
                    saw_pass_1 = true;
                } else if pass == 2 {
                    saw_pass_2 = true;
                }
            }
            ProcessStatus::Success {
                output_bytes, ..
            } => {
                finished = true;
                let out_mb = output_bytes as f64 / (1024.0 * 1024.0);
                // Verify output is around target size (within reasonable container variance)
                assert!(out_mb < 0.65, "Output file size {out_mb}MB exceeded target 0.5MB budget");
                break;
            }
            ProcessStatus::Error(err) => {
                panic!("Target size compression failed: {err}");
            }
            _ => {}
        }
    }

    assert!(finished, "Target size compression did not finish in time");
    assert!(saw_pass_1 || saw_pass_2, "Expected to see passes");
    assert!(output_path.exists());
}
