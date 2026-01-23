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

bugs:
- should not auto switch to other frames that just finished loading
- displays 0% even after conversions succeeds
