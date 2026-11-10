use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

use crate::models::VideoMetadata;

#[derive(Deserialize, Debug)]
struct FfprobeOutput {
    streams: Option<Vec<FfprobeStream>>,
    format: Option<FfprobeFormat>,
}

#[derive(Deserialize, Debug)]
struct FfprobeStream {
    codec_name: Option<String>,
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<String>,
}

#[derive(Deserialize, Debug)]
struct FfprobeFormat {
    duration: Option<String>,
    size: Option<String>,
}

pub fn probe_video(path: &Path) -> Result<VideoMetadata> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration,size:stream=width,height,codec_name,codec_type,duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .with_context(|| "Failed to execute ffprobe. Make sure FFmpeg is installed and in your PATH.")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("ffprobe failed: {err}");
    }

    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout)
        .with_context(|| "Failed to parse ffprobe JSON output")?;

    let mut video_codec = String::from("unknown");
    let mut audio_codec: Option<String> = None;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut stream_duration: Option<f64> = None;

    if let Some(streams) = parsed.streams {
        for s in streams {
            let c_type = s.codec_type.as_deref().unwrap_or("");
            if c_type == "video" && width == 0 {
                video_codec = s.codec_name.unwrap_or_else(|| "unknown".into());
                width = s.width.unwrap_or(0);
                height = s.height.unwrap_or(0);
                if let Some(d) = s.duration.and_then(|d| d.parse::<f64>().ok()) {
                    stream_duration = Some(d);
                }
            } else if c_type == "audio" && audio_codec.is_none() {
                audio_codec = s.codec_name;
            }
        }
    }

    let duration_secs = parsed
        .format
        .as_ref()
        .and_then(|f| f.duration.as_ref())
        .and_then(|d| d.parse::<f64>().ok())
        .or(stream_duration)
        .unwrap_or(0.0);

    let file_meta_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let size_bytes = parsed
        .format
        .as_ref()
        .and_then(|f| f.size.as_ref())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(file_meta_size);

    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    Ok(VideoMetadata {
        path: path.to_path_buf(),
        filename,
        duration_secs,
        width,
        height,
        size_bytes,
        video_codec,
        audio_codec,
    })
}
