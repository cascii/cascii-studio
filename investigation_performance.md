# Montage Playback Performance Investigation

Date: 2026-03-21

## Issue Summary

Observed behavior in `src/pages/montage.rs`:

- The timeline reaches `Ready (5/5)`.
- Pressing Play causes an initial glitch/freeze.
- Playback appears to restart.
- After that, both video clips and frame-based clips play at an abnormally low frame rate.

This investigation was done by reading the montage playback code path, the frame viewer, the video player, and the media preparation pipeline, plus inspecting the provided sample media file:

- `/Users/hjoncour/Desktop/perf issue.mov`

This pass did **not** include an interactive browser/webview profiler capture, so the findings below are ranked by confidence rather than treated as fully proven runtime facts.

## Sample Media Notes

`ffprobe` for `/Users/hjoncour/Desktop/perf issue.mov` reports:

- Container: `mov`
- Duration: `26.4s`
- Video: `H.264 Main`
- Resolution: `1610x1214`
- Frame rate: `60 fps`
- Pixel format: `yuv420p`
- Audio: `AAC`, mono, `48 kHz`
- Total bitrate: about `4.1 Mb/s`

Why that matters:

- `60 fps` is a much tighter performance budget than `24/30 fps`.
- `1610x1214` is not a standard playback size, so the webview has to scale it.
- The current pipeline plays the original cached asset directly instead of using a normalized proxy.

## Executive Conclusion

The most likely root cause is **not one single bug**, but a combination of:

1. **Playback progress updates forcing top-level montage state updates every tick**
2. **Simultaneous preview/overview work competing with the active player**
3. **Frame playback being driven by `setInterval` plus Yew state updates on the main thread**
4. **Direct playback of original `.mov` assets instead of optimized playback proxies**

The strongest shared cause for "both video and frames are slow" is item 1. The strongest cause specific to frames is item 3. The most likely explanation for the initial freeze/restart feeling is item 2, plus the ready/overview transition timing in item 5 below.

## Ranked Findings

| Confidence | Finding | Why it matches the symptom |
| --- | --- | --- |
| High | Parent-level progress state is updated on every playback tick | Explains why both video and frame playback slow down |
| High | Active playback competes with overview/preload media surfaces | Explains the startup glitch/freeze and extra decode/render load |
| High for frames | `AsciiFramesViewer` uses `setInterval` + state-per-frame playback | Explains low frame rate and restart sensitivity for frame clips |
| Medium | Frame bundles and color cache work are duplicated across viewers | Can add memory pressure and main-thread work even after preload |
| Medium | Original `.mov` files are played directly instead of proxies | Can make the webview work harder, especially at `60 fps` |
| Medium | Overview/ready gating can make playback look frozen, then restart | Matches the "freeze, then restart" sequence |
| Low | Misc. debug logging and timeline bookkeeping | Worth cleaning up, but unlikely to be the primary bottleneck |

## Detailed Findings

### 1. Parent progress updates likely force heavy montage rerenders during playback

Code:

- `src/pages/montage.rs:1436-1452`
- `src/pages/montage.rs:3450-3455`
- `src/pages/montage.rs:3695-3721`
- `src/components/settings/controls.rs:66-78`
- `src/components/settings/controls.rs:147-151`

What happens:

- `on_item_progress` in `montage.rs` receives progress from the active child player.
- While playing, it computes a global value and calls `synced_progress.set(...)`.
- That state is used by:
  - the sidebar transport slider
  - the timeline slider under `#montage-timeline-container`
- For frame clips, `AsciiFramesViewer` emits progress every playback interval.
- For video clips, `VideoPlayer` emits progress on every `timeupdate`.

Why this is a strong suspect:

- It is the clearest shared hot path between video playback and frame playback.
- Updating top-level page state during playback means Yew has to re-run the montage component and diff a large page tree while media is trying to animate.
- The montage page contains the resource tree, explorer tree, workspace, timeline, transport controls, and export UI. Even if not every child fully rerenders, the page-level render/diff churn is still significant.

Why it matches the symptom:

- Both media types slow down.
- The slowdown happens immediately after pressing Play.
- A 60 fps source can easily overwhelm this pattern because it pushes frequent UI updates into the main thread.

Recommended fixes:

- Stop pushing playback progress into montage page state on every tick.
- Keep playback progress local to the active player during playback.
- Only sync global progress to the parent at a throttled rate, for example `5-10 Hz`.
- During normal playback, update the transport slider imperatively or through a tiny isolated component instead of the full montage page state.
- Preserve full-frequency progress only while the user is actively scrubbing.

Recommended implementation direction:

- Replace `synced_progress.set(...)` in the hot path with a throttled callback.
- Keep a `last_emit_time` and only publish parent progress every `100-200 ms`.
- Alternatively, move the transport UI into a smaller component that owns its own playback indicator state.

### 2. The active player is competing with overview and warmup media work

Code:

- `src/pages/montage.rs:376-419`
- `src/pages/montage.rs:443-480`
- `src/pages/montage.rs:1346-1359`
- `src/pages/montage.rs:3513-3580`
- `src/pages/montage.rs:3592-3670`
- `src/styles/montage.css:79-88`
- `src/styles/montage.css:90-186`

What happens:

- Video assets are "warmed" with hidden `<video preload="auto">` elements via `warmVideoAsset(...)`.
- The overview tiles render their own `MontageVideoStill`, which is also a `<video preload="auto">`.
- The active playback surface mounts a full `VideoPlayer`.
- The active pane is hidden with `opacity: 0` while the overview remains mounted on top until `workspace_ready` becomes true.

That means a single video clip can temporarily involve:

- one hidden warmup video
- one overview video
- one active playback video

Why this is a strong suspect:

- Multiple media elements can compete for decode, buffering, and compositing right when playback begins.
- The active player may already be playing while the overview is still mounted and visually on top.
- This fits the "freeze/glitch first, then restart/settle" pattern much better than a pure decoder issue alone.

Why it matches the symptom:

- The first moment of playback is where the issue is most visible.
- The problem is worse with real video clips, but shared page pressure can spill over to frame clips too.

Recommended fixes:

- When Play is pressed, unmount the overview immediately instead of waiting for the active surface to become ready.
- Do not keep the warmup `<video>` alive once the real active player mounts.
- Replace overview video tiles with posters/stills instead of live `<video>` tags.
- Ensure only one decoder-backed video element exists per asset during active playback.

Recommended implementation direction:

- Change the startup sequence to:
  - decide the active clip
  - hide/unmount overview immediately
  - mount the active player only
- For overview thumbnails, use a static first-frame image or a single captured poster instead of `preload="auto"` videos.

### 3. `AsciiFramesViewer` uses timer-driven playback with state updates every frame

Code:

- `src/components/ascii_frames_viewer.rs:446-532`
- `src/components/ascii_frames_viewer.rs:535-571`

What happens:

- Frame playback is driven by `gloo_timers::callback::Interval`.
- The interval frequency is `1000 / fps`.
- Every tick:
  - calculates the next frame
  - updates `current_index`
  - triggers a component state update
  - emits progress back to the parent

Why this is a strong suspect for frame clips:

- `setInterval` on the main thread drifts under load.
- Component state updates every frame are expensive, especially when the parent also receives progress updates.
- At `30/60 fps`, any extra layout/diff/render work quickly causes dropped frames.

Why it matches the symptom:

- Explains the abnormally low frame rate for ASCII/frame clips directly.
- The "restart" behavior can also happen more visibly here because a fresh `Some(true)` play signal resets playback to frame 0 unless it is treated as a resume case.

Recommended fixes:

- Move frame playback from `Interval` to a `requestAnimationFrame` loop.
- Derive the displayed frame from elapsed wall-clock time instead of incrementing frame-by-frame state.
- Draw the current frame imperatively and avoid Yew state updates for every frame.
- Decouple per-frame local playback from parent progress publication.

Recommended implementation direction:

- Keep `start_time`, `pause_offset`, and `fps` in refs.
- On each animation frame, compute `current_index = floor(elapsed_seconds * fps)`.
- Only call `set_state` when the displayed frame actually changes and when the UI truly depends on it.
- Prefer drawing to canvas or updating one isolated DOM node instead of re-rendering the whole component tree.

### 4. Frame preload data and cache work can still add pressure after "Ready"

Code:

- `src/components/frame_media.rs:255-303`
- `src/pages/montage.rs:2317-2383`
- `src/pages/montage.rs:2438-2465`
- `src/pages/montage.rs:3644-3655`
- `src/components/ascii_frames_viewer.rs:332-352`
- `src/components/ascii_frames_viewer.rs:696-799`

What happens:

- Frame clips are fully preloaded into `PreloadedFrameBundle`.
- When an `AsciiFramesViewer` receives a preloaded bundle, it clones `preloaded_bundle.frames` into its own local `frames_ref`.
- The overview can mount its own frame viewer.
- The active pane can mount another frame viewer.
- A background color canvas cache worker can still spin up and prerender offscreen canvases.

Why this matters:

- The app may hold duplicate copies of the same frame data in multiple mounted viewers.
- Even if overview viewers are not actively playing, they still mount and initialize.
- Color cache work is designed to back off during B/W playback, but it still exists and still wakes up.

Why it matches the symptom:

- This is probably not the primary reason video clips are slow.
- It is a realistic contributor to overall main-thread pressure and memory pressure in mixed timelines.

Recommended fixes:

- Share preloaded frame bundles instead of cloning full frame vectors per viewer.
- Do not mount full `AsciiFramesViewer` instances in overview tiles.
- Use a lighter static preview component for overview.
- Disable background color cache workers for non-active viewers.
- Suspend non-essential background frame work while playback is active.

### 5. The active playback source is the original cached media, not an optimized playback proxy

Code:

- `src-tauri/src/commands/media.rs:87-116`
- `src/pages/montage.rs:2156-2190`
- `src/pages/montage.rs:2235-2269`

What happens:

- `prepare_media(...)` only hard-links or copies the original file into cache.
- It does not normalize the media for playback.
- The montage player then feeds that cached original asset directly to the webview.

Why this matters:

- The sample file is a `60 fps` `.mov` with nonstandard dimensions.
- System webviews are often less forgiving than desktop media players when asked to decode arbitrary source media while the same UI thread is also doing heavy app work.

Why it matches the symptom:

- It can explain why a particular file feels much worse than a simpler MP4.
- It does not fully explain why frame clips are also slow, so this is probably a contributor rather than the main root cause.

Recommended fixes:

- Introduce optimized playback proxies for montage preview, separate from export.
- Normalize source video to a webview-friendly format such as H.264/AAC MP4.
- Consider a preview preset such as:
  - `1920x1080` or source-bounded max size
  - `30 fps` for preview
  - medium CRF / medium preset

Fast validation step:

- Transcode `/Users/hjoncour/Desktop/perf issue.mov` to a normalized MP4 and test the exact same montage path.
- If playback improves noticeably, the raw source format is part of the problem.

### 6. Overview/ready gating can create the visible "freeze then restart" effect

Code:

- `src/pages/montage.rs:1346-1359`
- `src/pages/montage.rs:1362-1378`
- `src/pages/montage.rs:3516-3520`
- `src/components/video_player.rs:429-458`
- `src/components/video_player.rs:527-599`
- `src/components/ascii_frames_viewer.rs:542-568`

What happens:

- Playback can be requested before the active workspace is visually revealed.
- The active pane stays hidden until `workspace_ready`.
- The video player may receive `should_play = Some(true)` before or during readiness changes.
- The frame viewer treats a fresh `Some(true)` as "start from frame 0".

Why this matters:

- Even if playback is technically progressing, the user may still see the overview.
- When the active pane finally becomes visible, playback can appear to jump or restart from the beginning.

Why it matches the symptom:

- The reported sequence is specifically "freeze, then restart."
- This is a good explanation for the *perception* of restart, even if some of the motion happened behind the overview.

Recommended fixes:

- Do not start active playback while the active pane is still hidden.
- Or invert the sequence: make the active pane visible first, then start playback.
- Preserve play/resume state carefully so a readiness transition cannot reset the frame viewer back to frame 0 accidentally.

### 7. Lower-confidence contributors

#### 7a. Debug logging

Code:

- `src/components/video_player.rs:437-452`
- `src/components/video_player.rs:557-580`
- `src/pages/montage.rs:1307-1337`
- `src/pages/montage.rs:1369-1418`

Notes:

- There is active console logging around playback state changes.
- I do not see per-frame logging in the hot playback loop, so this is unlikely to be the primary cause.
- Still worth reducing once the main issues are fixed.

#### 7b. Source clip duration bookkeeping

Code:

- `src/pages/montage.rs:792-805`

Notes:

- Source clips start with `length_seconds: 0.0`.
- That is more of a correctness/progress issue than a direct playback performance issue.
- It is worth cleaning up, but it does not explain the freeze/slow playback behavior by itself.

## Most Likely Root Cause Chain

The current most likely sequence is:

1. The user presses Play after preload says `Ready`.
2. The montage page mounts or activates the playback surface.
3. At the same time, overview media surfaces and warmup media surfaces are still present.
4. The active player begins emitting progress.
5. Parent montage state is updated on every tick.
6. The main thread now has to handle:
   - media decode/render
   - page-level Yew render/diff work
   - timeline slider updates
   - sidebar transport updates
   - any remaining overview/caching work
7. Playback stutters badly.
8. Once the readiness/visibility transition completes, the user perceives a restart or jump.

## Recommended Fix Order

### Priority 1: Remove playback-tick updates from montage page state

This is the first thing I would change.

- Throttle `on_item_progress`.
- Stop updating `synced_progress` at frame/video cadence.
- Keep the transport indicator local or isolated.

Expected impact:

- Should improve both video and frames immediately.
- Lowest-risk architectural fix with the best shared upside.

### Priority 2: Stop mounting/keeping extra media surfaces during active playback

- Unmount overview immediately on Play.
- Remove decoder-backed overview videos.
- Tear down warmup video elements once active playback begins.

Expected impact:

- Should reduce the startup glitch/freeze.
- Should lower decode/compositing contention for video clips.

### Priority 3: Rework frame playback to use an animation clock, not `Interval`

- Move frame playback to `requestAnimationFrame`.
- Base frame selection on elapsed time.
- Avoid full component state changes every frame.

Expected impact:

- Biggest improvement for frame clips.
- Makes playback behavior more stable under load.

### Priority 4: Add playback proxies for video sources

- Normalize to preview-friendly MP4 proxies.
- Use those proxies in montage playback.

Expected impact:

- Especially helpful for odd-dimension, high-frame-rate, or less webview-friendly source files.

### Priority 5: Reduce background work in inactive viewers

- No full `AsciiFramesViewer` in overview.
- No color cache workers for hidden/non-active viewers.
- Share frame bundles rather than cloning them.

Expected impact:

- Cleans up residual overhead and memory pressure.

## Suggested Instrumentation Before/While Fixing

To confirm the above with hard data, add these measurements:

1. Count montage renders per second

- Add a debug counter in `montage.rs` and verify whether playback is causing page renders at `30-60 Hz`.

2. Count active media surfaces

- Log how many `<video>` elements exist when playback begins.
- Verify whether the same clip has warmup, overview, and active video elements simultaneously.

3. Measure progress callback frequency

- Log or sample how often `on_item_progress` fires for:
  - video clips
  - frame clips

4. Compare playback with progress sync disabled

- Temporarily no-op `synced_progress.set(...)` during playback.
- If playback becomes smooth, the main shared bottleneck is confirmed.

5. Compare playback with overview disabled

- Force the app to skip `show_workspace_overview`.
- If the startup freeze disappears, the overview path is confirmed as a contributor.

6. Compare raw MOV vs normalized MP4

- Transcode the provided sample and retest the same montage.
- This isolates decoder/container overhead from UI/render overhead.

7. Profile frame viewer worker activity

- Confirm whether background color caching is still active during playback.

## Concrete Next Experiments

If this investigation is turned into a fix plan, I would test in this order:

1. Temporarily disable `on_item_progress -> synced_progress.set(...)` during playback.
2. Temporarily remove overview rendering during active playback.
3. Temporarily disable `warmVideoAsset(...)`.
4. Test the provided `.mov` after transcoding to a normalized MP4 proxy.
5. Rework `AsciiFramesViewer` playback timing.

That order should separate the shared bottleneck from the media-format-specific bottleneck very quickly.

## Final Assessment

The strongest explanation is:

- **The montage page is doing too much work on the main thread while playback is running.**

The two most likely design issues are:

- **top-level state updates on every playback tick**
- **extra preview/overview media work still existing at playback start**

The frame viewer also has an additional structural problem:

- **timer-driven playback with component state churn at frame cadence**

If only one thing is changed first, it should be the playback progress architecture in `src/pages/montage.rs`.
