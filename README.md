# Video Compressor

[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg?style=flat-square)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg?style=flat-square)](https://github.com/andrr64/video-compressor)
[![FFmpeg](https://img.shields.io/badge/FFmpeg-8.x%20Required-green.svg?style=flat-square)](https://ffmpeg.org/)

A fast, lightweight native desktop app to compress videos down to exact target sizes (like 8 MB for Discord or 25 MB for email) or scale resolutions without quality collapse.

Built with Rust, eframe, and egui. It renders directly through your GPU via WGPU without Electron, Node.js, or web runtimes.

```
+-----------------------------------------------------------------------+
|  Video Compressor      [ Compressor ]  [ About ]               v0.1.0 |
+-----------------------------------------------------------------------+
|  [ Browse Video... ]  sample_footage.mp4 (142.50 MB)                  |
|  Duration: 01:45  |  Resolution: 1920x1080  |  Codecs: h264 / aac     |
+-----------------------------------------------------------------------+
|  Mode: [ Target File Size (2-Pass) ]  [ Resolution & Quality (CRF) ]  |
|                                                                       |
|  Presets: [ 8 MB (Discord) ]  [ 25 MB (Email/WA) ]  [ 50 MB ]  [ 100 MB]  |
|  Custom Budget: [ 15.0 MB ]                                           |
|  Allocated: Video 1080 kbps | Audio 128 kbps | Total 1208 kbps        |
+-----------------------------------------------------------------------+
|  > Encoder Parameters                                                 |
|  Destination: sample_footage_compressed.mp4             [ Select... ] |
+-----------------------------------------------------------------------+
|  [ Start Queue ]   Encoding Pass 2 of 2 (68%)      Speed: 3.2x  48 FPS|
+-----------------------------------------------------------------------+
```

## Why Video Compressor?

Most online compressors upload your private videos to third-party servers, queue you behind paywalls, or cap file sizes. Desktop tools built on Electron consume hundreds of megabytes of RAM just to show a form.

Video Compressor solves this with a native Rust utility:
- Instant startup with zero background services.
- Under 20 MB memory footprint during active encoding.
- Pure local processing through FFmpeg: your files never leave your computer.
- Exact budget adherence through two-pass variable bitrate allocation.

## Compression Modes

### Mode 1: Target File Size (2-Pass Encoding)
Designed for file-sharing limits where exceeding the budget means an upload rejection.
- Calculates the exact video bitrate using video duration, audio budget, and container muxing margins.
- Pass 1 analyzes frame complexity and generates an optimal bitrate distribution log.
- Pass 2 writes the final video, hitting the target size within 1-3% accuracy.
- Optional downscaling prevents starvation artifacts when compressing long videos into small targets.

### Mode 2: Target Resolution (CRF Mode)
Designed for standard resizing and archiving.
- Rescales video dimensions while preserving aspect ratios and forcing even pixel dimensions (`scale='min(W,iw)':-2`).
- Presets: 4K UHD, 1440p 2K, 1080p FHD, 720p HD, 480p SD, 360p, or proportional scaling (75%, 50%).
- Variable Constant Rate Factor (CRF) slider between 16 (high quality) and 36 (compact file), with 23 as the balanced default.

## Do I Need to Download FFmpeg?

**Yes.** Video Compressor uses FFmpeg as its underlying media engine for stream inspection and transcoding.

If you already have `ffmpeg` in your terminal PATH, you are ready to go. If not, install it with one command:

### Windows
Open PowerShell and run:
```powershell
winget install ffmpeg
```
*(Alternative: `choco install ffmpeg`)*

### macOS
Open Terminal and run:
```bash
brew install ffmpeg
```

### Linux (Ubuntu / Debian / Arch)
```bash
# Ubuntu / Debian
sudo apt update && sudo apt install ffmpeg

# Arch Linux
sudo pacman -S ffmpeg
```

*Note: The application detects whether FFmpeg is available on startup and displays an in-app notice with the install command if it is missing.*

## Installation & Running

### Option A: Run Standalone Executable (Windows)
Download or build `video-compressor.exe` and launch it directly. No installer or runtime setup needed.

### Option B: Build from Source

Ensure you have Rust installed (via [rustup.rs](https://rustup.rs/)):

```powershell
# Clone the repository
git clone https://github.com/andrr64/video-compressor.git
cd video-compressor

# Run development build
cargo run

# Build optimized release binary
cargo build --release
```
The optimized executable will be located at `target/release/video-compressor.exe`.

### Running Tests
```powershell
cargo test
```
The test suite includes bitrate calculation unit tests and automated integration tests that generate synthetic media and verify end-to-end encoding passes directly with FFmpeg.

## Technical Architecture

| Component | Technology | Purpose |
| :--- | :--- | :--- |
| Core Language | Rust 2024 Edition | Safe concurrency, memory efficiency, standalone binaries |
| UI Framework | eframe / egui 0.37 | Immediate-mode UI, 60 FPS GPU rendering via WGPU |
| File Dialogs | rfd 0.18 | Native Windows / macOS / Linux file open and save dialogs |
| Media Engine | FFmpeg & ffprobe 8.x | Stream probing, 2-pass bitrate allocation, scale filters |
| Concurrency | crossbeam-channel | Non-blocking background worker thread with atomic cancellation |

## License

MIT License. Developed by [Entropy Labs](https://github.com/andrr64).
