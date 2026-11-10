use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct VideoMetadata {
    pub path: PathBuf,
    pub filename: String,
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
    pub video_codec: String,
    pub audio_codec: Option<String>,
}

impl VideoMetadata {
    pub fn size_mb(&self) -> f64 {
        self.size_bytes as f64 / (1024.0 * 1024.0)
    }

    pub fn duration_formatted(&self) -> String {
        let total_secs = self.duration_secs.round() as u64;
        let hours = total_secs / 3600;
        let mins = (total_secs % 3600) / 60;
        let secs = total_secs % 60;
        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, mins, secs)
        } else {
            format!("{:02}:{:02}", mins, secs)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeTab {
    TargetSize,
    Resolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionPreset {
    Res360p,
    Res480p,
    Res720p,
    Res1080p,
    Res1440p,
    Res4k,
    Scale75,
    Scale50,
}

impl ResolutionPreset {
    pub const ALL: [ResolutionPreset; 8] = [
        ResolutionPreset::Res1080p,
        ResolutionPreset::Res720p,
        ResolutionPreset::Res480p,
        ResolutionPreset::Res360p,
        ResolutionPreset::Res1440p,
        ResolutionPreset::Res4k,
        ResolutionPreset::Scale75,
        ResolutionPreset::Scale50,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ResolutionPreset::Res360p => "360p (640×360)",
            ResolutionPreset::Res480p => "480p (854×480)",
            ResolutionPreset::Res720p => "720p HD (1280×720)",
            ResolutionPreset::Res1080p => "1080p FHD (1920×1080)",
            ResolutionPreset::Res1440p => "1440p 2K (2560×1440)",
            ResolutionPreset::Res4k => "4K UHD (3840×2160)",
            ResolutionPreset::Scale75 => "75% of Original",
            ResolutionPreset::Scale50 => "50% of Original",
        }
    }

    pub fn to_scale_filter(&self) -> String {
        match self {
            ResolutionPreset::Res360p => "scale='min(640,iw)':-2".to_string(),
            ResolutionPreset::Res480p => "scale='min(854,iw)':-2".to_string(),
            ResolutionPreset::Res720p => "scale='min(1280,iw)':-2".to_string(),
            ResolutionPreset::Res1080p => "scale='min(1920,iw)':-2".to_string(),
            ResolutionPreset::Res1440p => "scale='min(2560,iw)':-2".to_string(),
            ResolutionPreset::Res4k => "scale='min(3840,iw)':-2".to_string(),
            ResolutionPreset::Scale75 => "scale=trunc(iw*0.75/2)*2:trunc(ih*0.75/2)*2".to_string(),
            ResolutionPreset::Scale50 => "scale=trunc(iw*0.5/2)*2:trunc(ih*0.5/2)*2".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
}

impl VideoCodec {
    pub const ALL: [VideoCodec; 2] = [VideoCodec::H264, VideoCodec::H265];

    pub fn ffmpeg_name(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "libx264",
            VideoCodec::H265 => "libx265",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264 (Maximum Compatibility)",
            VideoCodec::H265 => "H.265 / HEVC (Higher Efficiency)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedPreset {
    Ultrafast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
}

impl SpeedPreset {
    pub const ALL: [SpeedPreset; 6] = [
        SpeedPreset::Ultrafast,
        SpeedPreset::Veryfast,
        SpeedPreset::Faster,
        SpeedPreset::Fast,
        SpeedPreset::Medium,
        SpeedPreset::Slow,
    ];

    pub fn ffmpeg_name(&self) -> &'static str {
        match self {
            SpeedPreset::Ultrafast => "ultrafast",
            SpeedPreset::Veryfast => "veryfast",
            SpeedPreset::Faster => "faster",
            SpeedPreset::Fast => "fast",
            SpeedPreset::Medium => "medium",
            SpeedPreset::Slow => "slow",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SpeedPreset::Ultrafast => "Ultrafast (Lowest quality/ratio)",
            SpeedPreset::Veryfast => "Very Fast",
            SpeedPreset::Faster => "Faster",
            SpeedPreset::Fast => "Fast (Recommended)",
            SpeedPreset::Medium => "Medium (Balanced)",
            SpeedPreset::Slow => "Slow (Maximum quality/ratio)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioOption {
    Aac128k,
    Aac96k,
    Aac64k,
    Copy,
    Mute,
}

impl AudioOption {
    pub const ALL: [AudioOption; 5] = [
        AudioOption::Aac128k,
        AudioOption::Aac96k,
        AudioOption::Aac64k,
        AudioOption::Copy,
        AudioOption::Mute,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            AudioOption::Aac128k => "AAC 128 kbps (Default)",
            AudioOption::Aac96k => "AAC 96 kbps (Voice / Compact)",
            AudioOption::Aac64k => "AAC 64 kbps (Max Space Saver)",
            AudioOption::Copy => "Copy Original Audio",
            AudioOption::Mute => "Mute (Remove Audio)",
        }
    }

    pub fn bitrate_kbps(&self) -> u32 {
        match self {
            AudioOption::Aac128k => 128,
            AudioOption::Aac96k => 96,
            AudioOption::Aac64k => 64,
            AudioOption::Copy => 128,
            AudioOption::Mute => 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompressionSettings {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
    pub mode: ModeTab,
    // Target size settings
    pub target_mb: f64,
    pub target_size_resolution: Option<ResolutionPreset>,
    // Resolution settings
    pub res_preset: ResolutionPreset,
    pub crf: u32,
    // General settings
    pub codec: VideoCodec,
    pub speed: SpeedPreset,
    pub audio: AudioOption,
}

#[derive(Debug, Clone)]
pub enum ProcessStatus {
    Idle,
    #[allow(dead_code)]
    Probing,
    Encoding {
        pass: u8,
        total_passes: u8,
        percent: f32,
        current_time_secs: f64,
        speed: String,
        fps: f32,
        eta_secs: Option<u64>,
    },
    Success {
        output_path: PathBuf,
        original_bytes: u64,
        output_bytes: u64,
        elapsed_secs: f64,
    },
    Error(String),
    Cancelled,
}
