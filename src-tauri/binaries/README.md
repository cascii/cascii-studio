# Bundled FFmpeg Binaries

This folder is for bundling ffmpeg/ffprobe with the application for distribution.

**Note:** The binaries are NOT stored in git. They are downloaded automatically during CI builds.

## CI/CD

The GitHub Actions workflows automatically download platform-specific ffmpeg binaries:
- macOS: https://evermeet.cx/ffmpeg/
- Windows: https://www.gyan.dev/ffmpeg/builds/
- Linux: https://johnvansickle.com/ffmpeg/

## Local Development

For local development, you have two options:

1. **Use system ffmpeg** (recommended): Install ffmpeg via your package manager
   - macOS: `brew install ffmpeg`
   - Linux: `apt install ffmpeg` or equivalent
   - Windows: `choco install ffmpeg` or download manually

2. **Use bundled binaries**: Download and place binaries here manually:
   ```
   binaries/
   ├── ffmpeg      (macOS/Linux executable)
   ├── ffprobe     (macOS/Linux executable)
   ├── ffmpeg.exe  (Windows executable)
   └── ffprobe.exe (Windows executable)
   ```

## How it works

The app checks for ffmpeg in this order:
1. User preference from Settings (System or Sidecar)
2. System PATH (ffmpeg/ffprobe commands)
3. Bundled binaries in app resources

This allows the app to work for:
- Developers with ffmpeg installed
- End users without ffmpeg (using bundled binaries)
