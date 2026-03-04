# Refactoring Investigation: Large File Decomposition

This document analyses the two largest files in the codebase and proposes how to
split each into smaller, focused modules without changing behaviour.

---

## File Size Overview

| File | Lines | Role |
|------|------:|------|
| `src/pages/project.rs` | 2 624 | Frontend – Project page (Yew component) |
| `src-tauri/src/lib.rs` | 2 324 | Backend – All Tauri commands + app setup |
| `src-tauri/src/database.rs` | 1 246 | Backend – All SQLite access (already separate) |
| `src/components/ascii_frames_viewer.rs` | 1 268 | Frontend – Frames viewer (already a standalone component) |
| `src/components/video_player.rs` | 1 045 | Frontend – Video player (already a standalone component) |
| `src/pages/montage.rs` | 844 | Frontend – Montage page |

The two files targeted for refactoring are **`lib.rs`** and **`project.rs`**.

---

## 1. `src-tauri/src/lib.rs` (2 324 lines)

### Current Structure

`lib.rs` is a monolith containing **43 `#[tauri::command]` handlers**, all
request/response structs, helper functions, and the `run()` entry point. The
content falls into these logical domains:

| Domain | Lines (approx.) | Commands | Key items |
|--------|----------------:|----------|-----------|
| **Types & media helpers** | 1–88 | `prepare_media` | `MediaKind`, `PreparedMedia`, `get_media_cache_dir`, `guess_mime_type`, `determine_media_kind` |
| **Settings commands** | 90–117 | `greet`, `load_settings`, `save_settings`, `set_loop_enabled`, `get_loop_enabled` | Thin wrappers around `settings::` |
| **File dialogs** | 118–268 | `pick_directory`, `open_directory`, `pick_files` | OS-specific `open_directory` |
| **File processing helpers** | 270–435 | — | `is_video_file`, `is_mp4_file`, `calculate_file_size`, `copy_or_move_file`, `get_video_duration`, `ffmpeg_convert_to_mp4` |
| **Source file management** | 150–242, 412–841 | `add_source_files`, `create_project`, `get_all_projects`, `get_project`, `get_project_sources`, `rename_source_file`, `delete_source_file`, `delete_project` | `CreateProjectRequest`, `AddSourceFilesRequest/Args`, `FileProgress`, project CRUD |
| **Frame directories** | 624–761 | `get_project_frames`, `get_frame_files`, `read_frame_file`, `read_cframe_file`, `delete_frame_directory`, `update_frame_custom_name` | `FrameDirectory`, `FrameFile`, `scan_frames_in_dir` |
| **ASCII conversion** | 881–1381 | `convert_to_ascii`, `get_project_conversions`, `get_conversion_by_folder_path`, `update_conversion_frame_speed` | `ConvertToAsciiRequest`, `ConversionProgress/Complete`, background tokio task, `extract_audio_from_video` |
| **FFmpeg config** | 917–1064 | `check_system_ffmpeg`, `check_sidecar_ffmpeg` | `command_exists`, `get_sidecar_paths`, `get_ffmpeg_config`, `get_sidecar_config` |
| **Video cuts** | 1430–1754 | `cut_video`, `get_project_cuts`, `delete_cut`, `rename_cut`, `preprocess_video` | `CutVideoRequest/Args`, `PreprocessVideoRequest/Args` |
| **Frame operations** | 1756–1983 | `cut_frames`, `crop_frames` | `CutFramesRequest`, `CropFramesRequest` |
| **Previews** | 1985–2149 | `create_preview`, `get_project_previews`, `delete_preview`, `rename_preview` | `CreatePreviewRequest`, `DeletePreviewRequest`, `RenamePreviewRequest` |
| **Explorer layout** | 2152–2169 | `get_explorer_layout`, `save_explorer_layout` | `SaveExplorerLayoutRequest` |
| **Context menu** | 2171–2268 | `show_resources_context_menu` | `ResourcesContextMenuState`, menu builder, event handler |
| **App bootstrap** | 2270–2324 | — | `run()` with `.invoke_handler(...)` |

### Proposed Module Structure

```
src-tauri/src/
├── lib.rs                    (~60 lines)  — mod declarations + run()
├── commands/
│   ├── mod.rs                — re-exports all command fns
│   ├── settings.rs           (~40 lines)  — greet, load/save_settings, loop_enabled
│   ├── dialogs.rs            (~60 lines)  — pick_directory, open_directory, pick_files
│   ├── projects.rs           (~250 lines) — create_project, get_all_projects, get_project,
│   │                                        add_source_files, delete_project,
│   │                                        rename/delete_source_file
│   ├── frames.rs             (~200 lines) — get_project_frames, get_frame_files,
│   │                                        read_frame_file, read_cframe_file,
│   │                                        delete_frame_directory, update_frame_custom_name
│   ├── conversion.rs         (~350 lines) — convert_to_ascii, get_project_conversions,
│   │                                        get_conversion_by_folder_path,
│   │                                        update_conversion_frame_speed
│   ├── cuts.rs               (~250 lines) — cut_video, preprocess_video,
│   │                                        get_project_cuts, delete_cut, rename_cut
│   ├── frame_ops.rs          (~220 lines) — cut_frames, crop_frames
│   ├── previews.rs           (~180 lines) — create_preview, get_project_previews,
│   │                                        delete_preview, rename_preview
│   ├── explorer.rs           (~30 lines)  — get_explorer_layout, save_explorer_layout
│   └── context_menu.rs       (~100 lines) — show_resources_context_menu,
│                                            ResourcesContextMenuState, menu handler
├── helpers/
│   ├── mod.rs                — re-exports
│   ├── media.rs              (~90 lines)  — MediaKind, PreparedMedia, prepare_media,
│   │                                        get_media_cache_dir, guess_mime_type,
│   │                                        determine_media_kind
│   ├── files.rs              (~60 lines)  — is_video_file, is_mp4_file,
│   │                                        calculate_file_size, copy_or_move_file
│   ├── ffmpeg.rs             (~200 lines) — ffmpeg_convert_to_mp4, get_video_duration,
│   │                                        extract_audio_from_video, command_exists,
│   │                                        get_sidecar_paths, get_ffmpeg_config,
│   │                                        get_sidecar_config, check_system/sidecar_ffmpeg
│   └── utils.rs              (~30 lines)  — generate_random_suffix,
│                                            count_frames_and_size, scan_frames_in_dir
├── types.rs                  (~80 lines)  — All request/response structs shared across
│                                            commands (FileProgress, ConversionProgress,
│                                            ConversionComplete, etc.)
├── database.rs               (unchanged)
└── settings.rs               (unchanged)
```

### Key Decisions

1. **`commands/` directory**: Groups all `#[tauri::command]` functions by domain.
   Each file is self-contained and imports from `helpers/`, `types.rs`, and
   `database`.

2. **`helpers/` directory**: Pure functions with no Tauri dependency (except
   `ffmpeg.rs` which needs `tauri::AppHandle` for sidecar path resolution). These
   can be unit-tested independently.

3. **`types.rs`**: Centralises request/response structs (currently scattered
   throughout `lib.rs`). These are mostly `#[derive(Deserialize)]` structs used
   as command arguments.

4. **Slimmed `lib.rs`**: Only contains `mod` declarations and the `run()`
   function with `tauri::generate_handler![...]`. This is the single place that
   registers all commands, so adding a new command still only requires touching
   two files (the command module + `lib.rs`).

5. **`database.rs` stays as-is**: At 1 246 lines it's already domain-focused.
   It could be split later (e.g., `database/projects.rs`, `database/conversions.rs`)
   but that's a separate concern and not urgent.

### Migration Strategy

- Move one domain at a time (e.g., start with `commands/settings.rs` since it's
  the smallest and has no internal dependencies).
- After each move, update `lib.rs` to `mod commands;` and adjust the
  `generate_handler![]` macro to use the fully qualified paths.
- The `generate_handler![]` macro requires all command functions to be in scope,
  so `commands/mod.rs` should `pub use` every command function.
- Run `cargo check` after each domain move to catch import issues immediately.

### Risks & Considerations

- **`generate_handler![]` paths**: Tauri's macro resolves function paths at
  compile time. All command functions must be accessible from the scope where the
  macro is invoked. Solution: `pub use` re-exports in `commands/mod.rs`.
- **Shared state**: `ResourcesContextMenuState` is managed via `app.manage()` in
  `run()` and accessed via `tauri::State<>` in the command. Moving the state
  struct to `commands/context_menu.rs` and importing it in `lib.rs` works fine.
- **`app: tauri::AppHandle` parameter**: Several commands take the app handle.
  This doesn't change when moved to submodules.

---

## 2. `src/pages/project.rs` (2 624 lines)

### Current Structure

This file contains a single `#[function_component(ProjectPage)]` with:

| Section | Lines (approx.) | Description |
|---------|----------------:|-------------|
| **Imports** | 1–18 | External crates + internal components |
| **JS bindings (inline)** | 20–155 | `tauri_invoke`, viewer controls sync, resize observer, `tauri_listen`/`tauri_unlisten` |
| **Type definitions** | 157–256 | `FileProgress`, `PreparedMedia`, `MediaKind`, `ContentType`, `SourceContent`, `FrameDirectory`, `PreviewSettings`, `Preview` |
| **Constants & helpers** | 258–347 | Rate-limiting consts, `PlaybackSyncLimiter`, utility fns (`file_name_from_path`, `without_extension`, label fns, `tab_id_for_resource`, `open_or_activate_tab`) |
| **Props** | 349–354 | `ProjectPageProps` |
| **State declarations** | 357–444 | 45+ `use_state`/`use_mut_ref` hooks |
| **Effects** | 446–771 | Data-fetching (project, sources, frames, cuts, previews), conversion progress listener, completion listener, polling timer |
| **Callbacks: media selection** | 773–843 | `on_select_source` |
| **Callbacks: video operations** | 845–957 | `on_cut_video`, `on_preprocess_video` |
| **Callbacks: CRUD operations** | 959–1414 | `on_delete_cut`, `on_rename_cut`, `on_delete_source_file`, `on_delete_frame`, `on_cut_frames`, `on_crop_frames`, `on_preview_created`, `on_delete_preview`, `on_rename_preview_explorer` |
| **Callbacks: explorer sidebar** | 1437–1831 | Select/rename/open for source, frame, cut, preview; add files; toggle section; layout change |
| **Callbacks: tabs** | 1833–1996 | `on_select_tab`, `on_close_tab`, `on_reorder_tabs` |
| **Render: pre-computation** | 1998–2057 | Conversions HTML, labels |
| **Render: HTML template** | 2059–2623 | Full page layout with sidebar, tabs, preview columns |

### Why This File Is So Large

1. **Duplicated types**: `MediaKind`, `PreparedMedia`, `ContentType`, `SourceContent`,
   `FrameDirectory`, `Preview`, `PreviewSettings` are re-defined here (they also
   exist in the backend). They should live in a shared types module.

2. **Inline JS bindings**: ~135 lines of inline JavaScript for Tauri IPC,
   resize observers, and viewer controls sync. These are generic utilities.

3. **Monolithic component**: The `ProjectPage` function is ~2 270 lines long. It
   manages state for: project data, source files, frame directories, video cuts,
   previews, tabs, explorer sidebar, playback controls, conversion progress,
   and file upload progress.

4. **Repetitive callback patterns**: Many callbacks follow the same pattern:
   clone state → `spawn_local` → `tauri_invoke` → update state. This accounts
   for ~1 000 lines of boilerplate.

### Proposed Module Structure

```
src/pages/
├── project/
│   ├── mod.rs                (~80 lines)   — re-exports ProjectPage + ProjectPageProps
│   ├── types.rs              (~120 lines)  — All type definitions (MediaKind, PreparedMedia,
│   │                                         SourceContent, FrameDirectory, Preview, etc.)
│   ├── bindings.rs           (~140 lines)  — All wasm_bindgen/inline_js blocks
│   │                                         (tauri_invoke, tauri_listen, viewer controls,
│   │                                         resize observer, appConvertFileSrc)
│   ├── helpers.rs            (~80 lines)   — PlaybackSyncLimiter, constants, utility fns
│   │                                         (file_name_from_path, without_extension,
│   │                                         label fns, tab_id_for_resource,
│   │                                         open_or_activate_tab)
│   ├── hooks/
│   │   ├── mod.rs            — re-exports all custom hooks
│   │   ├── use_project_data.rs  (~120 lines) — Custom hook that fetches project, sources,
│   │   │                                       frames, cuts, previews and returns state handles
│   │   ├── use_conversion_events.rs (~110 lines) — Conversion progress listener +
│   │   │                                           completion listener + polling timer
│   │   └── use_playback_sync.rs (~80 lines) — Playback sync limiter logic, viewer controls
│   │                                          resize observer
│   ├── callbacks/
│   │   ├── mod.rs            — re-exports all callback factory functions
│   │   ├── media.rs          (~120 lines)  — on_select_source, on_select_cut_explorer
│   │   ├── video_ops.rs      (~120 lines)  — on_cut_video, on_preprocess_video
│   │   ├── crud.rs           (~400 lines)  — on_delete_cut, on_delete_source_file,
│   │   │                                     on_delete_frame, on_delete_preview,
│   │   │                                     on_rename_*, on_open_*
│   │   ├── frames.rs         (~150 lines)  — on_cut_frames, on_crop_frames,
│   │   │                                     on_select_frame_dir_explorer,
│   │   │                                     on_preview_created
│   │   ├── explorer.rs       (~100 lines)  — on_add_files_explorer, on_toggle_section,
│   │   │                                     on_explorer_layout_change
│   │   └── tabs.rs           (~170 lines)  — on_select_tab, on_close_tab, on_reorder_tabs
│   └── view.rs              (~600 lines)   — The html! template (render function)
```

### Key Decisions

1. **`project/` directory replaces `project.rs`**: Rust's module system allows a
   `project/mod.rs` to replace `project.rs`. The `mod.rs` defines the
   `ProjectPage` component and delegates to submodules.

2. **Custom hooks**: Yew supports custom hooks via functions that return state
   handles. The data-fetching effects (~200 lines) and event listener setup
   (~200 lines) become reusable hooks:
   - `use_project_data(project_id) -> ProjectData` — fetches all project resources
   - `use_conversion_events(project_id, ...) -> ConversionState` — manages listeners + polling

3. **Callback factory modules**: Each callback module exports functions that take
   the required state handles as parameters and return a `Callback<T>`. Example:
   ```rust
   // callbacks/media.rs
   pub fn make_on_select_source(
       selected_source: &UseStateHandle<Option<SourceContent>>,
       asset_url: &UseStateHandle<Option<String>>,
       url_cache: &UseStateHandle<HashMap<String, String>>,
       // ...
   ) -> Callback<SourceContent> { ... }
   ```
   This keeps the main component function short — just calling factory functions.

4. **Types module**: Centralises the ~100 lines of struct/enum definitions. These
   types are also duplicated from the backend; a future improvement would be a
   shared crate (see "Future: Shared Types Crate" below).

5. **Bindings module**: All `#[wasm_bindgen(inline_js = ...)]` blocks move here.
   The inline JS strings are large and obscure the Rust logic around them.

6. **View module**: The `html!` template (~560 lines) moves to its own file. The
   main `project_page()` function calls `view::render(...)` passing all the
   computed props. This is the single largest readability win.

### Migration Strategy

- **Phase 1: Extract types + bindings + helpers** (safe, mechanical moves)
  1. Create `src/pages/project/` directory
  2. Move type definitions → `types.rs`
  3. Move JS bindings → `bindings.rs`
  4. Move helpers → `helpers.rs`
  5. Move `project.rs` content → `mod.rs`
  6. Update imports across the codebase (`use crate::pages::project::*` paths stay the same if `mod.rs` re-exports)

- **Phase 2: Extract callbacks** (requires passing state handles)
  1. Start with the simplest callbacks (e.g., `on_toggle_section`, `on_open_*`)
  2. Group related callbacks into factory modules
  3. Replace inline closures in `mod.rs` with calls to factory functions

- **Phase 3: Extract hooks** (requires understanding Yew custom hooks)
  1. Extract `use_project_data` (the initial data-fetching effect)
  2. Extract `use_conversion_events` (progress/completion listeners + polling)
  3. Extract `use_playback_sync` (resize observer + sync limiter)

- **Phase 4: Extract view** (final step)
  1. Move the `html!` template to `view.rs`
  2. Define a `ViewProps` struct with all the data the template needs
  3. The main component function becomes: state setup → callback creation → `view::render(props)`

### Risks & Considerations

- **Yew hook rules**: Hooks must be called unconditionally at the top level of a
  component. Custom hooks that wrap `use_state`/`use_effect_with` work fine as
  long as they're always called in the same order. The proposed `use_project_data`
  and `use_conversion_events` would be called at the top of `project_page()`.

- **Callback cloning overhead**: Yew `Callback` values are `Rc`-wrapped and cheap
  to clone. Moving callbacks to factory functions doesn't add overhead.

- **Borrow checker with many state handles**: Callback factories need many
  `UseStateHandle` parameters (the current code clones handles extensively).
  Consider grouping related state into structs:
  ```rust
  struct PlaybackState {
      is_playing: UseStateHandle<bool>,
      should_reset: UseStateHandle<bool>,
      synced_progress: UseStateHandle<f64>,
      seek_percentage: UseStateHandle<Option<f64>>,
      // ...
  }
  ```
  This reduces parameter count from 45+ individual handles to 5-6 state structs.

- **Re-exports for backward compatibility**: `crate::pages::project::ProjectPage`
  and `ProjectPageProps` must remain importable. `mod.rs` handles this with
  `pub use`.

---

## 3. Other Files Worth Noting

### `src-tauri/src/database.rs` (1 246 lines)

Currently a single file with all SQL queries. Could be split into:
- `database/projects.rs` — Project CRUD
- `database/sources.rs` — SourceContent CRUD
- `database/conversions.rs` — AsciiConversion CRUD
- `database/cuts.rs` — VideoCut CRUD
- `database/previews.rs` — Preview CRUD
- `database/audio.rs` — AudioExtraction CRUD
- `database/explorer.rs` — Explorer layout persistence

**Not urgent** — the file is well-organised with clear section comments and
consistent patterns. But it would follow the same strategy if desired.

### `src/pages/montage.rs` (844 lines)

Similar pattern to `project.rs` but smaller. Could benefit from the same
types/bindings/callbacks extraction if it grows further.

### `src/components/ascii_frames_viewer.rs` (1 268 lines)

Already a self-contained component. Could extract its inline JS bindings and
callback logic, but low priority.

---

## 4. Future: Shared Types Crate

Both the backend (`lib.rs`) and frontend (`project.rs`) define the same types:
- `MediaKind` (Image/Video)
- `PreparedMedia`
- `SourceContent` / `ContentType`
- `FrameDirectory`
- `Preview` / `PreviewSettings`
- `FileProgress`

These are serialised/deserialised across the Tauri IPC boundary and must stay
in sync. A shared crate (`cascii-types` or `cascii-shared`) would:
- Eliminate the duplicate definitions
- Ensure frontend and backend always agree on field names and types
- Reduce the risk of deserialization failures from type drift

This would be a workspace-level change:
```
cascii-studio/
├── Cargo.toml          (workspace)
├── cascii-shared/      (new crate)
│   ├── Cargo.toml
│   └── src/lib.rs      — shared types
├── src-tauri/
│   ├── Cargo.toml      — depends on cascii-shared
│   └── src/...
└── src/                 (frontend)
    └── ...              — depends on cascii-shared
```

---

## 5. Recommended Priority Order

1. **`src-tauri/src/lib.rs` → `commands/` + `helpers/`**
   - Highest impact: 43 commands in one file
   - Lowest risk: no UI changes, pure backend restructuring
   - Easy to verify: `cargo check` + existing manual testing

2. **`src/pages/project.rs` → `project/` directory** (Phases 1–2)
   - Extract types, bindings, helpers first (mechanical, safe)
   - Then extract callbacks (requires care with hook ordering)

3. **`src/pages/project.rs` → `project/` directory** (Phases 3–4)
   - Custom hooks + view extraction (more complex, do after stabilising phases 1–2)

4. **Shared types crate** (optional, future)
   - Only if type drift becomes a maintenance burden

5. **`database.rs` split** (optional, future)
   - Only if the file continues to grow significantly
