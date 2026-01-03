# cascii-studio

ASCII art studio built with Tauri + Yew

## Tech Stack

- **Backend**: Rust + Tauri v2
- **Frontend**: Yew (Rust WASM)
- **Database**: SQLite (rusqlite)
- **Styling**: Custom CSS

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).

## Getting Started

### Prerequisites

- Rust (latest stable)
- Trunk: `cargo install trunk`
- System dependencies for Tauri (usually handled by Xcode on macOS)

### Running in Development

```bash
./run.sh
```

Or:

```bash
cargo tauri dev
```

### Building for Production

```bash
cargo tauri build
```

### Testing the Build Locally

To test that the build works correctly (including icon bundling):

```bash
cargo tauri build --debug
```

This will build a debug version for your current platform and ensure all assets (including icons) are properly bundled. For production builds, use `cargo tauri build` without the `--debug` flag.

## Application Data Locations

### Database
- **macOS**: `~/Library/Application Support/cascii_studio/projects.db`
- **Linux**: `~/.config/cascii_studio/projects.db`
- **Windows**: `%APPDATA%\cascii_studio\projects.db`

### Media Cache
- **macOS**: `~/Library/Application Support/cascii_studio/media/`
- **Linux**: `~/.config/cascii_studio/media/`
- **Windows**: `%APPDATA%\cascii_studio\media/`

### Settings
- **macOS**: `~/Library/Application Support/cascii_studio/settings.json`
- **Linux**: `~/.config/cascii_studio/settings.json`
- **Windows**: `%APPDATA%\cascii_studio\settings.json`

## Features

- Create ASCII art projects from images and videos
- Video player with custom controls
- Project management (create, open, delete)
- Local file caching with asset protocol for secure media loading
- Settings configuration

