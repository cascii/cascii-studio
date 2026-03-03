# Tabs System Investigation

## Goal

Add a VS Code-style tab bar to the main content area. When a user clicks an item in the Resources or Explorer sidebar, it opens as a tab. Tabs have fixed width, show an `x` close button, and display the full name on hover. Clicking a tab switches the main content to show the corresponding panel.

---

## Current Architecture

### Selection State (project.rs)

The project page tracks four independent selection states:

```rust
let selected_source = use_state(|| None::<SourceContent>);      // drives Source Video panel
let selected_frame_dir = use_state(|| None::<FrameDirectory>);   // drives Frames panel
let selected_cut = use_state(|| None::<VideoCut>);               // also drives Source Video via synthetic SourceContent
let selected_preview = use_state(|| None::<Preview>);            // drives Frames panel (read-only)
```

**Mutual exclusivity rules:**
- `selected_frame_dir` and `selected_preview` clear each other (only one active at a time)
- `selected_cut` creates a synthetic `SourceContent` and sets `selected_source` + `asset_url`
- `selected_source` requires async media preparation (`prepare_media` Tauri command) to get an `asset_url`

### Main Content Rendering (project.rs lines 1665-1975)

The main content area is a 2-column grid:

```
.preview-container (CSS grid: 1fr 1fr)
├── Column 1: "Source Video"
│   ├── Loading state → spinner
│   ├── selected_source + asset_url exists:
│   │   ├── ContentType::Image → <img>
│   │   └── ContentType::Video → <VideoPlayer> with full conversion UI
│   └── Nothing selected → "Select a source file to preview"
│
└── Column 2: "Frames"
    ├── selected_preview exists → AsciiFramesViewer (read-only)
    ├── Nothing at all → "No frames generated yet"
    ├── selected_frame_dir exists → AsciiFramesViewer (full controls)
    └── Nothing selected → "Select a frame directory or preview"
```

### Resource Identification

Each resource type has a unique identifier:

| Type | ID Field | Example |
|------|----------|---------|
| `SourceContent` | `id: String` (UUID) | `"a1b2c3..."` |
| `VideoCut` | `id: String` (UUID) | `"d4e5f6..."` |
| `FrameDirectory` | `directory_path: String` | `"/path/to/frames"` |
| `Preview` | `id: String` (UUID) | `"g7h8i9..."` |

The existing `ResourceRef` enum (in `explorer_types.rs`) already unifies these:

```rust
pub enum ResourceRef {
    SourceFile { source_id: String },
    VideoCut { cut_id: String },
    FrameDirectory { directory_path: String },
    Preview { preview_id: String },
}
```

### Display Name Resolution

Display names are resolved differently per type:
- **SourceContent**: `custom_name` or filename from `file_path`
- **VideoCut**: `custom_name` or `"Cut HH:MM - HH:MM"` format
- **FrameDirectory**: `name` field directly
- **Preview**: `custom_name` or `folder_name`

The `resolve_label()` function in `explorer_tree.rs` (lines 29-86) already does this for all types.

### Sidebar Selection Callbacks

When items are clicked in the sidebar:

1. **`on_select_source`** (line 655): Async media prep → sets `selected_source` + `asset_url`
2. **`on_select_frame_dir_explorer`** (line 1155): Sets `selected_frame_dir`, clears `selected_preview`, fetches conversion settings
3. **`on_select_cut_explorer`** (line 1204): Sets `selected_cut`, creates synthetic SourceContent, async media prep
4. **`on_select_preview_explorer`** (line 1194): Sets `selected_preview`, clears `selected_frame_dir`

---

## Proposed Design

### New Data Structures

```rust
/// Represents an open tab in the main content area.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenTab {
    /// Unique tab identifier (matches ResourceRef serialization)
    pub id: String,
    /// What resource this tab represents
    pub resource: ResourceRef,
    /// Display label for the tab
    pub label: String,
}
```

### New State

```rust
let open_tabs = use_state(|| Vec::<OpenTab>::new());
let active_tab_id = use_state(|| None::<String>);
```

### Tab ID Scheme

Use the same scheme as TreeNodeId for consistency:
- `"tab:source:{source_id}"`
- `"tab:cut:{cut_id}"`
- `"tab:framedir:{directory_path}"`
- `"tab:preview:{preview_id}"`

### Tab Behavior

1. **Opening a tab**: Clicking an item in the sidebar:
   - If tab already exists in `open_tabs` → just set it as active
   - If tab doesn't exist → append to `open_tabs` and set active
   - Also run the existing selection callback (media prep, settings loading, etc.)

2. **Switching tabs**: Clicking a tab in the tab bar:
   - Set `active_tab_id` to the clicked tab's id
   - Update `selected_*` state to match the tab's resource
   - For source/cut tabs → re-trigger media loading if `asset_url` not cached

3. **Closing a tab**: Clicking the `x` button:
   - Remove from `open_tabs`
   - If it was the active tab → activate the next tab (or previous, or none)
   - Clear corresponding `selected_*` state if no other tab of same type exists

4. **Panel mapping**:

   | Tab Resource | Affects Column | State Change |
   |-------------|---------------|--------------|
   | SourceFile | Source Video | `selected_source` + `asset_url` |
   | VideoCut | Source Video | `selected_source` + `asset_url` (via synthetic SourceContent) |
   | FrameDirectory | Frames | `selected_frame_dir`, clears `selected_preview` |
   | Preview | Frames | `selected_preview`, clears `selected_frame_dir` |

---

## Implementation Plan

### Files to Create

| File | Purpose |
|------|---------|
| `src/components/tab_bar.rs` | TabBar component (renders tabs row) |
| `src/styles/tabs.css` | Tab bar styling |

### Files to Modify

| File | Change |
|------|--------|
| `src/pages/project.rs` | Add tab state, modify selection callbacks, insert TabBar, update rendering logic |
| `src/components/mod.rs` | Add `pub mod tab_bar;` |
| `index.html` | Add `<link data-trunk rel="css" href="src/styles/tabs.css" />` |
| `Cargo.toml` | Add `LucideX` icon feature (for close button) |

### Step 1: Create `TabBar` Component

```
src/components/tab_bar.rs
```

**Props:**
```rust
pub struct TabBarProps {
    pub tabs: Vec<OpenTab>,
    pub active_tab_id: Option<String>,
    pub on_select_tab: Callback<String>,    // tab id
    pub on_close_tab: Callback<String>,     // tab id
}
```

**Rendering:**
- Horizontal scrollable row of tabs
- Each tab: fixed width (~140px), text-overflow ellipsis, `title` attr for hover tooltip
- Active tab: highlighted background, bottom border accent
- Close `x` button on each tab (visible on hover or when active)
- Icon per resource type (reuse same icons from tree_node.rs)

### Step 2: Create Tab Bar CSS

```css
.tab-bar {
    display: flex;
    align-items: stretch;
    height: 35px;
    background: #252526;
    border-bottom: 1px solid #333;
    overflow-x: auto;
    overflow-y: hidden;
    flex-shrink: 0;
}

.tab-bar::-webkit-scrollbar { height: 3px; }

.tab {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 140px;
    min-width: 140px;
    max-width: 140px;
    padding: 0 8px;
    height: 100%;
    cursor: pointer;
    color: #888;
    font-size: 13px;
    border-right: 1px solid #333;
    background: #1e1e1e;
    user-select: none;
    box-sizing: border-box;
}

.tab--active {
    background: #1e1e1e;
    color: #fff;
    border-bottom: 2px solid #007acc;  /* VS Code blue accent */
}

.tab:hover:not(.tab--active) {
    background: #2a2d2e;
}

.tab__icon { width: 14px; height: 14px; flex-shrink: 0; color: #888; }
.tab__label { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tab__close { width: 16px; height: 16px; opacity: 0; /* visible on hover/active */ }
.tab:hover .tab__close, .tab--active .tab__close { opacity: 1; }
```

### Step 3: Add Tab State to project.rs

```rust
let open_tabs = use_state(|| Vec::<OpenTab>::new());
let active_tab_id = use_state(|| None::<String>);
```

### Step 4: Modify Selection Callbacks

Wrap each `on_select_*` callback to also open/activate a tab:

```rust
// Example for on_select_source:
// 1. Run existing media loading logic
// 2. Build OpenTab { id: format!("tab:source:{}", source.id), resource: ResourceRef::SourceFile { source_id }, label }
// 3. If tab not in open_tabs → push it
// 4. Set active_tab_id
```

### Step 5: Add Tab Switching Logic

When a tab is clicked:
- Set `active_tab_id`
- Match on `ResourceRef` variant
- Call the appropriate existing selection callback to load the resource

### Step 6: Add Tab Close Logic

When a tab's close button is clicked:
- Remove tab from `open_tabs`
- If it was active: activate adjacent tab or clear selection
- If no tabs left: clear all `selected_*` states

### Step 7: Insert TabBar in HTML

Between error alerts and preview-container in the main content area:

```html
<div class="main-content">
    // Error alerts...
    // Add files progress...

    <TabBar
        tabs={(*open_tabs).clone()}
        active_tab_id={(*active_tab_id).clone()}
        on_select_tab={on_select_tab}
        on_close_tab={on_close_tab}
    />

    <div class="preview-container">
        // ... existing rendering ...
    </div>
</div>
```

### Step 8: Update Rendering Logic

The rendering logic in the preview-container stays mostly the same — it already reads from `selected_source`, `selected_frame_dir`, and `selected_preview`, which are updated when tabs are switched.

---

## Edge Cases

1. **URL caching**: The existing `url_cache: HashMap<String, String>` already caches prepared media URLs. When switching back to a previously opened source tab, the cache prevents redundant `prepare_media` calls.

2. **Conversion state**: Active conversions (`active_conversions_ref`) are tracked per source_id. The conversion progress UI should continue regardless of which tab is active.

3. **Controls sync**: The Controls section (play/pause/seek/volume) should sync with whatever source+frames combo is active for the current tab.

4. **Tab persistence**: Not required for v1, but the tab list could be persisted to the database alongside `explorer_layout` for session restore.

5. **Duplicate prevention**: When clicking the same item again, just activate the existing tab instead of creating a duplicate.

6. **Tab reordering**: Not required for v1. Can be added later with drag-and-drop.

7. **Middle-click**: Consider supporting middle-click to close tabs (browser convention).

---

## Visual Reference

```
┌─────────────────────────────────────────────────────────┬──────────────┐
│ [video.mp4 ×] [cut_01.mp4 ×] [frames/ ×] [preview ×]  │  [icons...]  │
├─────────────────────────────────────────────────────────┤  RESOURCES   │
│                                                         │  Source Files│
│  ┌──────────────────┐  ┌──────────────────┐            │    ...       │
│  │  SOURCE VIDEO     │  │  FRAMES          │            │  Frames      │
│  │                   │  │                  │            │    ...       │
│  │  <VideoPlayer>    │  │  <AsciiViewer>   │            │  EXPLORER    │
│  │                   │  │                  │            │    ...       │
│  └──────────────────┘  └──────────────────┘            │  CONTROLS    │
│                                                         │    ...       │
└─────────────────────────────────────────────────────────┴──────────────┘
```

---

## Complexity Assessment

- **New component**: ~120 lines (TabBar)
- **CSS**: ~80 lines (tabs.css)
- **project.rs changes**: ~100 lines (state, callbacks, tab open/close/switch logic)
- **Total estimate**: ~300 lines of new/modified code
- **Risk**: Low — additive change, existing rendering logic stays intact
