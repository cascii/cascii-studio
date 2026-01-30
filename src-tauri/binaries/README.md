# Bundled FFmpeg Binaries

This folder is for bundling ffmpeg/ffprobe with the application for distribution.

## Development

During development, the app uses system-installed ffmpeg. No binaries needed here.

## Distribution

For distribution to users who may not have ffmpeg installed:

1. Download ffmpeg static builds for your target platforms:
   - macOS: https://evermeet.cx/ffmpeg/
   - Windows: https://www.gyan.dev/ffmpeg/builds/
   - Linux: https://johnvansickle.com/ffmpeg/

2. Place the binaries in this folder:
   ```
   binaries/
   ├── ffmpeg      (macOS/Linux executable)
   ├── ffprobe     (macOS/Linux executable)
   ├── ffmpeg.exe  (Windows executable)
   └── ffprobe.exe (Windows executable)
   ```

3. The app will automatically detect and use these bundled binaries when system ffmpeg is not available.

## How it works

The app checks for ffmpeg in this order:
1. System PATH (ffmpeg/ffprobe commands)
2. Bundled binaries in app resources

This allows the app to work for:
- Developers with ffmpeg installed
- End users without ffmpeg (using bundled binaries)
