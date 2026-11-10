# Video Compressor

A native desktop utility written in Rust for compressing video files using FFmpeg.

Built with eframe and egui, it provides a responsive dark-themed interface inspired by professional video tooling, rendering natively via hardware-accelerated WGPU without web runtimes or background daemons.

## Capabilities

### 1. Target File Size Compression (2-Pass Mode)
- Computes target video and audio bitrates based on duration and container overhead.
- Runs 2-pass encoding with `libx264` or `libx265` to match file size limits accurately.
- Includes quick presets for 8 MB (Discord attachment limit), 25 MB (email limit), 50 MB, 100 MB, or custom megabyte values.
- Optional resolution downscaling prevents heavy pixelation when compressing long video into small file limits.

### 2. Resolution and Quality Scaling (CRF Mode)
- Scales video dimensions while preserving aspect ratios and forcing even dimensions (`scale='min(W,iw)':-2`).
- Presets include 4K UHD, 1440p 2K, 1080p FHD, 720p HD, 480p SD, 360p, and proportional scaling (75%, 50%).
- Variable Constant Rate Factor (CRF) slider between 16 (high quality) and 36 (small size), with 23 as the default balance.

### 3. Application Workflow
- Drag and drop input files directly into the window or select through native file dialogs.
- Inspects duration, dimensions, container size, and active codecs with `ffprobe`.
- Transcoding runs in a background thread, keeping the interface at 60 frames per second.
- Shows real-time progress percentage, pass counter, encoding speed multiplier, FPS, and estimated time remaining.
- Cancel button terminates running FFmpeg subprocesses cleanly and removes incomplete output files.
- Displays input versus output file size comparisons, reduction percentages, and shortcuts to open the destination folder or play the resulting file.

## Prerequisites

- Rust toolchain (Rust 1.80 or newer)
- FFmpeg (ensure `ffmpeg` and `ffprobe` binaries are available in your system `PATH`)

## Building and Running

### Run Development Build
```powershell
cargo run
```

### Build Optimized Release Executable
```powershell
cargo build --release
```
The compiled executable will be generated at `target/release/video-compressor.exe`.

### Run Automated Tests
```powershell
cargo test
```
The test suite includes bitrate calculation unit tests and end-to-end integration tests that generate synthetic media and verify encoding passes directly against FFmpeg.

## Architecture

```
src/
├── main.rs            # Application entry point and window viewport configuration
├── lib.rs             # Public module declarations for binaries and integration tests
├── app.rs             # Application state, egui interface layout, and theme definitions
├── models.rs          # Data structures for metadata, compression settings, and progress events
└── ffmpeg/
    ├── mod.rs         # Submodule exports
    ├── probe.rs       # ffprobe execution and JSON metadata extraction
    ├── command.rs     # Bitrate math and FFmpeg argument builders
    └── runner.rs      # Subprocess management, stdout progress stream parsing, and cancellation
tests/
└── integration_test.rs# Automated integration tests using synthetic media
```

## License

MIT License. Developed by Entropy Labs.
