# TODO

## Process Isolation (If Needed)

For maximum memory isolation, run conversion in a completely separate process. Only implement if memory issues persist after current optimizations.

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

This completely isolates conversion memory from the Tauri process.
