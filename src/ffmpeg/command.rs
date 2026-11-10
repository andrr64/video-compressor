use std::path::Path;

use crate::models::{
    AudioOption, CompressionSettings, ModeTab,
    VideoMetadata,
};

// Re-export or alias if needed
pub struct CalculatedBitrate {
    pub video_kbps: u64,
    pub audio_kbps: u64,
    pub total_kbps: u64,
}

pub fn calculate_bitrate(
    target_mb: f64,
    duration_secs: f64,
    audio: AudioOption,
) -> CalculatedBitrate {
    let duration = duration_secs.max(0.1);
    // 1 MB = 1024 * 1024 bytes = 8,388,608 bits = 8,192 kbits (in 1024-base)
    // Using standard 1000-based bits for ffmpeg kbps (1 kbps = 1000 bps)
    // 1 MB = 1,000,000 bytes = 8,000,000 bits.
    // Or 1 MiB = 1024 * 1024 * 8 = 8,388,608 bits = 8388.608 kbits.
    // Let's use 8192 kbits for binary MB.
    let total_kbits = target_mb * 8192.0;
    let total_bitrate_kbps = total_kbits / duration;

    let mut desired_audio_kbps = audio.bitrate_kbps() as f64;

    // If audio would take more than 30% of total bitrate, scale it down to preserve video quality
    if desired_audio_kbps > 0.0 && desired_audio_kbps > total_bitrate_kbps * 0.3 {
        desired_audio_kbps = (total_bitrate_kbps * 0.25).max(32.0);
    }

    // Reserve 4% for container / packet header overhead (MP4 muxing)
    let container_overhead = total_bitrate_kbps * 0.04;
    let available_video = total_bitrate_kbps - desired_audio_kbps - container_overhead;
    let video_kbps = available_video.max(50.0).round() as u64;
    let audio_kbps = desired_audio_kbps.round() as u64;

    CalculatedBitrate {
        video_kbps,
        audio_kbps,
        total_kbps: total_bitrate_kbps.round() as u64,
    }
}

pub struct FfmpegPass {
    pub pass_number: u8,
    #[allow(dead_code)]
    pub total_passes: u8,
    pub args: Vec<String>,
}

pub fn build_ffmpeg_commands(
    settings: &CompressionSettings,
    meta: &VideoMetadata,
    passlog_dir: &Path,
) -> Vec<FfmpegPass> {
    let passlog_prefix = passlog_dir.join("ffmpeg2pass").to_string_lossy().to_string();

    match settings.mode {
        ModeTab::Resolution => {
            let mut args = vec![
                "-y".into(),
                "-i".into(),
                settings.input_path.to_string_lossy().into(),
                "-c:v".into(),
                settings.codec.ffmpeg_name().into(),
                "-preset".into(),
                settings.speed.ffmpeg_name().into(),
                "-crf".into(),
                settings.crf.to_string(),
                "-vf".into(),
                settings.res_preset.to_scale_filter(),
            ];

            append_audio_args(&mut args, settings.audio, None);

            args.extend([
                "-movflags".into(),
                "+faststart".into(),
                "-progress".into(),
                "pipe:1".into(),
                settings.output_path.to_string_lossy().into(),
            ]);

            vec![FfmpegPass {
                pass_number: 1,
                total_passes: 1,
                args,
            }]
        }

        ModeTab::TargetSize => {
            let bitrates = calculate_bitrate(settings.target_mb, meta.duration_secs, settings.audio);
            let video_b_arg = format!("{}k", bitrates.video_kbps);
            let maxrate_arg = format!("{}k", (bitrates.video_kbps as f64 * 1.5).round() as u64);
            let bufsize_arg = format!("{}k", bitrates.video_kbps * 2);

            let scale_filter = settings.target_size_resolution.map(|r| r.to_scale_filter());

            let null_sink = if cfg!(target_os = "windows") {
                "NUL"
            } else {
                "/dev/null"
            };

            // Pass 1: Analysis pass
            let mut pass1_args = vec![
                "-y".into(),
                "-i".into(),
                settings.input_path.to_string_lossy().into(),
                "-c:v".into(),
                settings.codec.ffmpeg_name().into(),
                "-b:v".into(),
                video_b_arg.clone(),
                "-maxrate".into(),
                maxrate_arg.clone(),
                "-bufsize".into(),
                bufsize_arg.clone(),
                "-preset".into(),
                settings.speed.ffmpeg_name().into(),
                "-pass".into(),
                "1".into(),
                "-passlogfile".into(),
                passlog_prefix.clone(),
            ];

            if let Some(ref filter) = scale_filter {
                pass1_args.push("-vf".into());
                pass1_args.push(filter.clone());
            }

            pass1_args.extend([
                "-an".into(),
                "-f".into(),
                "null".into(),
                "-progress".into(),
                "pipe:1".into(),
                null_sink.into(),
            ]);

            // Pass 2: Encoding pass
            let mut pass2_args = vec![
                "-y".into(),
                "-i".into(),
                settings.input_path.to_string_lossy().into(),
                "-c:v".into(),
                settings.codec.ffmpeg_name().into(),
                "-b:v".into(),
                video_b_arg,
                "-maxrate".into(),
                maxrate_arg,
                "-bufsize".into(),
                bufsize_arg,
                "-preset".into(),
                settings.speed.ffmpeg_name().into(),
                "-pass".into(),
                "2".into(),
                "-passlogfile".into(),
                passlog_prefix,
            ];

            if let Some(ref filter) = scale_filter {
                pass2_args.push("-vf".into());
                pass2_args.push(filter.clone());
            }

            append_audio_args(&mut pass2_args, settings.audio, Some(bitrates.audio_kbps));

            pass2_args.extend([
                "-movflags".into(),
                "+faststart".into(),
                "-progress".into(),
                "pipe:1".into(),
                settings.output_path.to_string_lossy().into(),
            ]);

            vec![
                FfmpegPass {
                    pass_number: 1,
                    total_passes: 2,
                    args: pass1_args,
                },
                FfmpegPass {
                    pass_number: 2,
                    total_passes: 2,
                    args: pass2_args,
                },
            ]
        }
    }
}

fn append_audio_args(args: &mut Vec<String>, audio: AudioOption, target_kbps: Option<u64>) {
    match audio {
        AudioOption::Mute => {
            args.push("-an".into());
        }
        AudioOption::Copy => {
            args.extend(["-c:a".into(), "copy".into()]);
        }
        AudioOption::Aac128k | AudioOption::Aac96k | AudioOption::Aac64k => {
            let kbps = target_kbps.unwrap_or_else(|| audio.bitrate_kbps() as u64);
            args.extend([
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                format!("{}k", kbps),
            ]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ResolutionPreset, SpeedPreset, VideoCodec};
    use std::path::PathBuf;

    #[test]
    fn test_calculate_bitrate_short_video() {
        // 10 MB target, 60s video
        let res = calculate_bitrate(10.0, 60.0, AudioOption::Aac128k);
        assert!(res.total_kbps > 1000);
        assert!(res.video_kbps > 800);
        assert_eq!(res.audio_kbps, 128);
    }

    #[test]
    fn test_calculate_bitrate_small_budget_scales_audio() {
        // 1 MB target, 120s video (very constrained)
        let res = calculate_bitrate(1.0, 120.0, AudioOption::Aac128k);
        // Total kbits = 8192 / 120 = 68.2 kbps
        // Desired audio (128) > 30% of 68, so audio should scale down
        assert!(res.audio_kbps < 128);
        assert!(res.video_kbps >= 50);
    }

    #[test]
    fn test_build_resolution_command() {
        let meta = VideoMetadata {
            path: PathBuf::from("in.mp4"),
            filename: "in.mp4".into(),
            duration_secs: 60.0,
            width: 1920,
            height: 1080,
            size_bytes: 50_000_000,
            video_codec: "h264".into(),
            audio_codec: Some("aac".into()),
        };
        let settings = CompressionSettings {
            input_path: PathBuf::from("in.mp4"),
            output_path: PathBuf::from("out.mp4"),
            mode: ModeTab::Resolution,
            target_mb: 25.0,
            target_size_resolution: None,
            res_preset: ResolutionPreset::Res720p,
            crf: 23,
            codec: VideoCodec::H264,
            speed: SpeedPreset::Fast,
            audio: AudioOption::Aac128k,
        };

        let temp_dir = std::env::temp_dir();
        let passes = build_ffmpeg_commands(&settings, &meta, &temp_dir);
        assert_eq!(passes.len(), 1);
        assert_eq!(passes[0].pass_number, 1);
        assert!(passes[0].args.contains(&"-crf".to_string()));
        assert!(passes[0].args.contains(&"scale='min(1280,iw)':-2".to_string()));
    }

    #[test]
    fn test_build_target_size_command_two_pass() {
        let meta = VideoMetadata {
            path: PathBuf::from("in.mp4"),
            filename: "in.mp4".into(),
            duration_secs: 100.0,
            width: 1920,
            height: 1080,
            size_bytes: 100_000_000,
            video_codec: "h264".into(),
            audio_codec: Some("aac".into()),
        };
        let settings = CompressionSettings {
            input_path: PathBuf::from("in.mp4"),
            output_path: PathBuf::from("out.mp4"),
            mode: ModeTab::TargetSize,
            target_mb: 15.0,
            target_size_resolution: Some(ResolutionPreset::Res720p),
            res_preset: ResolutionPreset::Res1080p,
            crf: 23,
            codec: VideoCodec::H264,
            speed: SpeedPreset::Medium,
            audio: AudioOption::Aac96k,
        };

        let temp_dir = std::env::temp_dir();
        let passes = build_ffmpeg_commands(&settings, &meta, &temp_dir);
        assert_eq!(passes.len(), 2);
        assert_eq!(passes[0].pass_number, 1);
        assert_eq!(passes[1].pass_number, 2);
        assert!(passes[0].args.contains(&"-pass".to_string()));
        assert!(passes[0].args.contains(&"1".to_string()));
        assert!(passes[1].args.contains(&"2".to_string()));
    }
}
