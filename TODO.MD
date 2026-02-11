.cascii filetype
- zipped text frames.
- color
- font
- player
- type

don't create project if not all files are saved/converted
- converting progress bar

cascii dep: autodetect to match framerate. i suspect that generating more frames than in the source files causes
Speed disparity

features
- auto load frames
- frames buffer
- load frames in parallel

Use side webviews to display frames and mp4 in windows
println should only happen if debug mode on in settings

Focus frame loading on selected frames

bugs:
- should not auto switch to other frames that just finished loading
- displays 0% even after conversions succeeds

- post treatment for frames: select background color and font color as an alternative to black and white

# Maybe run conversion in a completely separate process. 

### Option A: Separate Thread with Channel

```rust
use std::sync::mpsc;

#[tauri::command]
async fn convert_to_ascii(...) {
    let (progress_tx, progress_rx) = mpsc::channel();

    // Spawn conversion in dedicated thread
    std::thread::spawn(move || {
        converter.convert_video_with_progress(..., Some(move |completed, total| {
            // Send to channel (non-blocking)
            let _ = progress_tx.send((completed, total));
        }));
    });

    // Separate task to relay progress (throttled)
    tokio::spawn(async move {
        let mut last_percent = 0u8;
        while let Ok((completed, total)) = progress_rx.recv() {
            let percent = ((completed * 100) / total) as u8;
            if percent > last_percent {
                last_percent = percent;
                app.emit("conversion-progress", ...);
            }
        }
    });
}
```

### Option B: Sidecar Process

For complete isolation, spawn cascii as a subprocess:

```rust
use std::process::{Command, Stdio};

let mut child = Command::new("cascii")
    .args(&["--input", input_path, "--output", output_dir, "--progress"])
    .stdout(Stdio::piped())
    .spawn()?;

// Parse stdout for progress (cascii CLI already has throttled progress output)
```
