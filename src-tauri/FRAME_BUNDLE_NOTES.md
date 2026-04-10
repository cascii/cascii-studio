# Frame Bundle Notes

## What Was Added

A backend-only prototype was added in `src-tauri/src/frame_bundle.rs`.

It exposes two pure Rust helpers:

- `bundle_frame_directory_in_place(input_dir, output_stem)`
- `bundle_frame_directory(input_dir, output_dir, output_stem)`

These helpers are compiled into the Tauri crate, but they are not connected to any Tauri command, menu action, UI button, or conversion flow yet.

## What The Code Does

The module scans a frame directory for `frame_*.txt` and `frame_*.cframe` files, sorts them using the same `frame_####` convention already used elsewhere in the project, and writes two bundled outputs:

- `<stem>_frames.json`
- `<stem>_cframes.bin`

`<stem>_frames.json` is a minified JSON array of full frame strings.

`<stem>_cframes.bin` is a packed multi-frame color blob with this layout:

1. `u32` frame count, little-endian
2. `u32` width, little-endian
3. `u32` height, little-endian
4. Repeated per-frame payload bytes, where each cell is stored as `(char, r, g, b)`

That packed binary format matches what `cascii_core_view::parse_packed_cframes` expects, so it is compatible with the existing packed-color loading path used elsewhere in the ecosystem.

## Validation Built In

The helper does not just concatenate files blindly. It validates a few things that matter later when this is wired into the app:

- Every logical frame must have a `.cframe` file, because the output always includes the packed color bundle.
- All `.cframe` files must have identical dimensions.
- If a `.txt` file exists next to a `.cframe`, its content must match the text decoded from the `.cframe`.
- If a `.txt` file is missing, the helper reconstructs the text frame from the `.cframe` so the JSON bundle can still be produced.
- Text is normalized to `\n` line endings and guaranteed to end with a trailing newline, which matches the decoded `.cframe` text format.

## How To Use Later

Nothing calls this yet. When you want to wire it in, the intended usage looks like this:

```rust
use crate::frame_bundle::bundle_frame_directory_in_place;

let bundle = bundle_frame_directory_in_place(&frames_dir, "candles")?;
println!(
    "bundled {} frames to {} and {}",
    bundle.frame_count,
    bundle.frames_json_path.display(),
    bundle.packed_cframes_path.display()
);
```

Or if the outputs should go somewhere else:

```rust
use crate::frame_bundle::bundle_frame_directory;

let bundle = bundle_frame_directory(&source_dir, &output_dir, "preview")?;
```

## Likely Integration Points Later

The most obvious future call sites are:

- after `convert_to_ascii` finishes writing a new frame directory
- after `cut_frames` creates a derived frame directory
- after `crop_frames` rewrites or duplicates a frame directory
- in a future explicit “bundle frames” command if the workflow should stay manual

## Why This Was Added This Way

The goal here was to add the bundling logic without prematurely deciding how the UI or command surface should expose it.

Keeping it as an isolated backend module gives you:

- a compile-time checked implementation ready for later wiring
- a single place that defines the text JSON and packed color binary format
- a reusable API for both automatic and manual bundling flows
- tests that prove the binary layout and JSON output are correct for simple cases

## Files Changed

- `src-tauri/src/frame_bundle.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/FRAME_BUNDLE_NOTES.md`
