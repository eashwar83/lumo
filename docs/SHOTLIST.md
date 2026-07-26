# Lumo User Guide — Screenshot Checklist

This is the capture list for the images referenced in
[`USER_GUIDE.md`](./USER_GUIDE.md). Save each PNG into **`docs/images/`** using
the **exact filename** in the second column — the guide references these names,
so they appear automatically once captured.

**Fastest way to capture:** run
`powershell -ExecutionPolicy Bypass -File scripts\capture_guide_shots.ps1` with
Lumo open. It walks these rows in order, auto-crops to the Lumo window, and saves
each with the right filename. See that script's `-Only` / `-Redo` options to
re-shoot individual rows.

The **"How to set up the shot"** column tells you exactly how to reach each
feature — a keyboard shortcut, a menu path (shown as `Menu > Item`), or a button.
Shortcuts are the defaults; all are rebindable in Settings > Keyboard Shortcuts.

## Tips for good shots
- Load a **colourful, well-lit video** (e.g. a Blender open movie like *Tears of
  Steel*) so grading changes are obvious.
- Use a consistent, reasonably large window (~1280x720 or bigger) across shots.
- Move the mouse first so the **menu bar (top)** and **control bar (bottom)**
  are visible — they fade when the mouse is idle.
- Capture the relevant panel/dialog tightly (the script auto-crops to the Lumo
  window for you).
- Use the app's **dark theme** for a consistent look.

---

| # | Filename | Feature | How to set up the shot |
|---|----------|---------|------------------------|
| 01 | `01-hero-player-ui.png` | Player overview | Play a video, then move the mouse so both the menu bar (top) and control bar (bottom) are visible. Full window. This is the hero image. |
| 02 | `02-home-screen.png` | Home screen | Show the browse screen: menu bar > View > Home (or click Home in the left side-nav). If no video is loaded, it's already showing. |
| 03 | `03-menu-bar.png` | Menu bar | Move the mouse to reveal the menu bar, then click **Media** so its dropdown is open over the video. |
| 04 | `04-command-palette.png` | Command palette | Press **Ctrl+Shift+P** to open the Command Palette, then type a few letters (e.g. "screen") so results appear. |
| 05 | `05-playback-controls.png` | Control bar | Move the mouse so the bottom control bar shows (play/pause, seek bar, volume, time, buttons). Tight crop of the bar. |
| 06 | `06-ab-range.png` | A-B loop | Press **K** to mark point A, seek forward a bit, press **K** again to mark point B. The highlighted A-B range appears on the seek bar. (A third K clears it.) |
| 07 | `07-open-network.png` | Network streams | Menu bar > Media > Open Network Stream (or press **Ctrl+N**). Shows the WebDAV/DLNA/SMB browser. |
| 08 | `08-playlist-drawer.png` | Playlist | Open a local file (its folder auto-loads), then press **Tab** to open the playlist drawer with items listed. |
| 09 | `09-favourites.png` | Favourites | First favourite a couple of videos with **B** while playing. Then Media > Favourites (or **Ctrl+B**) to show the favourites grid. |
| 10 | `10-video-popover.png` | Video popover | In the control bar (bottom-right group of buttons), click the **Video** button (the gear/sliders icon) to open the Video popover. |
| 11 | `11-auto-enhance.png` | Auto Enhance | Open the Video popover (Video button), click **Auto Enhance**, and keep the popover open so the filled-in slider values are visible. |
| 12 | `12-colour-grade.png` | Colour grade | In the Video popover, show the colour-grade sliders (Exposure / Temperature / Tint / Highlights / Shadows, plus Brightness / Contrast / Saturation / Gamma / Hue). |
| 13 | `13-curves-editor.png` | Curves editor | Open the Curves panel: press **U** (or menu Video > Curves). Include the **Auto** button in frame. |
| 14 | `14-sharpness-grain.png` | Sharpness & grain | In the Video popover, show the **Sharpness (Amount + Radius)** and **Film Grain** controls. |
| 15 | `15-quality-presets.png` | Quality & upscaling | In the Video popover, show **Quality presets (Fast/Balanced/High)** and **AI Upscale (Anime / Live-action)**. |
| 16 | `16-look-presets.png` | Look presets | In the Video popover, show the **preset list** (Punch/Warm/Cool/Vivid/…). Also reachable via menu Video > Look Presets. |
| 17 | `17-split-view.png` | Before/After split | Press **Ctrl+W** (or menu Video > Before / after split view). Choose a frame where the two halves clearly differ. |
| 18 | `18-view-original-osd.png` | View Original / history | Press **\** (View Original) so the "Viewing original" readout shows; or press **Ctrl+Z** so the "Change N/M" position readout appears. |
| 19 | `19-ai-settings.png` | AI Enhance settings | Menu bar > Tools > Preferences, then Advanced > AI Enhance. Shows provider, API key (masked), base URL, and model. |
| 20 | `20-ai-prompt-window.png` | AI Correction prompt | Open the Video popover (Video button) and click **AI Enhance (Cloud)**. The AI Correction window opens with provider/model selectors, the prompt box, and the Recent dropdown. (Needs an API key set — see shot 19.) |
| 21 | `21-ai-reference-images.png` | Reference images | In the AI Correction window (shot 20), under **Reference** click **+ Add image** and add 1–3 images so the thumbnail chips show. |
| 22 | `22-ai-undo-readout.png` | AI undo readout | After running an AI correction, press **Ctrl+Z** so the multi-line readout (model / prompt / result) is on screen. |
| 23 | `23-aspect-crop.png` | Aspect & crop | Press **E** to cycle aspect ratio (the label changes), and/or **C** to auto-crop. Also under menu Video (Aspect Ratio / Crop). Capture the menu or the aspect label. |
| 24 | `24-zoom-rotate.png` | Zoom & rotate | Zoom with **Alt+Up / Alt+Down**, rotate with **R**. Capture the zoomed/rotated frame, or open the menu Video > Zoom / Rotate submenu. |
| 25 | `25-audio-panel.png` | Audio panel | Press **Shift+A** (or menu Audio > Audio Panel) to open the Audio panel with Night Mode, Equalizer, and Volume Boost. |
| 26 | `26-subtitle-menu.png` | Subtitles | Move the mouse to reveal the menu bar, then open the **Subtitle** menu (track selection + Dual subtitles). |
| 27 | `27-online-subtitles.png` | Online subtitles | Menu bar > Subtitle > Find Online Subtitles. Run a search so a results list is visible. |
| 28 | `28-scenes.png` | Scenes/chapters | First scan: menu bar > Playback > Scenes > Scan for scenes. Then the seek bar shows scene marks — capture the seek bar and/or the Scenes menu. |
| 29 | `29-screenshot.png` | Screenshot | Press **S** to capture the current frame; capture the confirmation toast that appears near the top of the window. |
| 30 | `30-contact-sheet.png` | Contact sheet | Menu bar > Media > Save Contact Sheet (**Ctrl+T**). Then open the result: Media > Open Export Folder (**Ctrl+Shift+E**) and capture the grid image. |
| 31 | `31-clip-gif-export.png` | Clip/GIF export | Set an A-B range first (**K**, seek, **K**). The clip/export controls appear; capture them. (Export Clip = **Ctrl+E**, Export GIF = **Ctrl+Shift+G**, both under menu Media.) |
| 32 | `32-merge-dialog.png` | Merge Files | Menu bar > Media > Merge Files. Click **+ Add files** and add 2–3 files so the list and the compatibility note are visible. |
| 33 | `33-split-dialog.png` | Split File | Menu bar > Media > Split File. Pick a mode tab (e.g. Timestamps or Equal parts) so the split options show. |
| 34 | `34-shortcuts-help.png` | Shortcuts help | Press **?** (or **F1**), or menu bar > Help > Keyboard Shortcuts, to show the shortcuts help overlay. |
| 35 | `35-settings-advanced.png` | Settings > Advanced | Menu bar > Tools > Preferences > Advanced. Scroll to the **ffmpeg path** and **Seek-Bar Thumbnails** rows. |

> Menu items for **Merge/Split/Export Clip** are greyed out until an **ffmpeg
> path** is set (Settings > Advanced) — set it before capturing shots 31–33.

---

### After capturing
Once the PNGs are in `docs/images/`, open `docs/USER_GUIDE.md` (or view it on
GitHub) to confirm every image renders. Any broken-image icon means a filename
mismatch — check it against the table above.
