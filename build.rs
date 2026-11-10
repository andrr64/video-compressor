#[cfg(windows)]
fn main() {
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icon.ico");
    res.set("FileDescription", "Video Compressor");
    res.set("ProductName", "Video Compressor");
    res.set("OriginalFilename", "video-compressor.exe");
    if let Err(e) = res.compile() {
        eprintln!("Warning: Failed to compile Windows resource: {e}");
    }
}

#[cfg(not(windows))]
fn main() {}
