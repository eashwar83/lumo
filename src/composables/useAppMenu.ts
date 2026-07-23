import { computed } from "vue";
import type { MenuNode, MenuTopLevel } from "../types/menu";
import type { MediaTrack } from "../types/media";
import {
    formatChord,
    type ShortcutActionId,
} from "../constants/shortcuts";
import { EQ_PRESETS, NIGHT_MODE_LABELS } from "./useAudioEnhancements";
import type { AudioEnhancementsController, NightModeLevel } from "./useAudioEnhancements";
import type { AbRangeController } from "./useAbRange";
import type { SkipMarkersController } from "./useSkipMarkers";
import type { VideoTransformController } from "./useVideoTransform";
import type {
    AiUpscaleMode,
    QualityPreset,
    VideoEnhancementsController,
} from "./useVideoEnhancements";
import type { VideoPresetsController } from "./useVideoPresets";

// Builds the application menu tree. Everything here is a thin wrapper over
// actions that already exist elsewhere — the menu is an additional route to
// them, never a second implementation.
//
// Continuous controls (EQ bands, colour sliders, sharpness) deliberately open
// their panel instead of appearing as menu items: a dropdown is a bad place for
// a slider.

const sep: MenuNode = { kind: "separator" };

type TrackMenuSource = {
    tracks: MediaTrack[];
    select: (track: MediaTrack) => void | Promise<void>;
    emptyLabel: string;
};

export type AppMenuOptions = {
    // --- state probes ---
    isFileLoaded: () => boolean;
    isLocalMedia: () => boolean;
    isPlaying: () => boolean;
    isFullscreen: () => boolean;
    isAlwaysOnTop: () => boolean;
    isLoopOne: () => boolean;
    isFavorite: () => boolean;
    areSubtitlesVisible: () => boolean;
    isDualSubEnabled: () => boolean;
    currentSpeed: () => number;
    playbackRates: () => number[];
    aspectLabel: () => string;
    hasAbRange: () => boolean;
    clipExportAvailable: () => boolean;
    /** Formatted accelerator for a rebindable action. */
    keyFor: (id: ShortcutActionId) => string | undefined;

    // --- controllers ---
    audio: AudioEnhancementsController;
    enhancements: VideoEnhancementsController;
    videoPresets: VideoPresetsController;
    transform: VideoTransformController;
    ab: AbRangeController;
    skip: SkipMarkersController;

    // --- track lists ---
    audioTracks: () => TrackMenuSource;
    subtitleTracks: () => TrackMenuSource;

    // --- commands ---
    openFilePicker: () => void;
    openFileOrFolderPicker: () => void;
    gotoPanel: (panel: "home" | "history" | "favorites" | "network") => void;
    addToPlaylist: () => void;
    exportContactSheet: () => void;
    exportClip: (asGif: boolean) => void;
    openExportFolder: () => void;
    quit: () => void;

    togglePlayPause: () => void;
    seekRelative: (seconds: number) => void;
    seekToStart: () => void;
    previousTrack: () => void;
    nextTrack: () => void;
    frameStep: (forward: boolean) => void;
    setSpeed: (rate: number) => void;
    resetSpeed: () => void;
    setSlomoFactor: (factor: number) => void;
    slomoFactor: () => number;
    toggleLoop: () => void;

    adjustVolume: (delta: number) => void;
    toggleMuted: () => void;
    isMuted: () => boolean;
    addExternalAudio: () => void;
    adjustAudioDelay: (delta: number) => void;
    resetAudioDelay: () => void;
    openAudioPanel: () => void;

    toggleFullscreen: () => void;
    toggleAlwaysOnTop: () => void;
    cycleAspectRatio: () => void;
    autoCropNow: () => void;
    clearCrop: () => void;
    fitWindowToVideo: () => void;
    stepWindowSize: (factor: number) => void;
    openCurves: () => void;
    toggleSplitCompare: () => void;
    syncSubtitlesByEar: () => void;
    nextScene: () => void;
    previousScene: () => void;
    rescanScenes: () => void;
    sceneCount: () => number;
    scenesAreChapters: () => boolean;
    autoEnhance: () => void;
    resetVideoSettings: () => void;
    takeScreenshot: (withSubtitles: boolean) => void;

    addExternalSubtitle: () => void;
    findOnlineSubtitles: () => void;
    toggleSubtitleVisibility: () => void;
    setDualSubEnabled: (enabled: boolean) => void;
    adjustSubtitleDelay: (delta: number) => void;
    resetSubtitleDelay: () => void;
    openSubtitleAdvanced: () => void;

    openSettings: () => void;
    toggleShortcutsHelp: () => void;
    toggleInfo: () => void;
    regenerateThumbnails: () => void;

    togglePlaylist: () => void;
    toggleFavorite: () => void;
    showProgress: () => void;
    hideMenuBar: () => void;

    checkForUpdates: () => void;
    showAbout: () => void;
};

export const useAppMenu = (options: AppMenuOptions) => {
    const key = (id: ShortcutActionId) => options.keyFor(id);

    /** Track lists become a flat radio group; empty lists show why. */
    const trackItems = (source: TrackMenuSource): MenuNode[] => {
        if (!source.tracks.length) {
            return [
                {
                    kind: "action",
                    label: source.emptyLabel,
                    disabled: true,
                    run: () => {},
                },
            ];
        }
        return source.tracks.map((track) => ({
            kind: "action" as const,
            label:
                [track.title?.trim(), track.lang?.trim()]
                    .filter(Boolean)
                    .join(" · ") || `Track ${track.id}`,
            checked: track.selected,
            run: () => void source.select(track),
        }));
    };

    const buildMedia = (): MenuNode[] => {
        const loaded = options.isFileLoaded();
        const local = options.isLocalMedia();
        const hasRange = options.hasAbRange();
        return [
            {
                kind: "action",
                label: "Open File…",
                shortcut: key("openFile"),
                run: options.openFilePicker,
            },
            {
                kind: "action",
                label: "Open File or Folder…",
                shortcut: key("openFileOrFolder"),
                run: options.openFileOrFolderPicker,
            },
            {
                kind: "action",
                label: "Open Network Stream…",
                shortcut: key("openNetworkStream"),
                run: () => options.gotoPanel("network"),
            },
            sep,
            {
                kind: "action",
                label: "Recent",
                shortcut: key("showRecent"),
                run: () => options.gotoPanel("history"),
            },
            {
                kind: "action",
                label: "Favourites",
                shortcut: key("showFavourites"),
                run: () => options.gotoPanel("favorites"),
            },
            {
                kind: "action",
                label: options.isFavorite()
                    ? "Remove from Favourites"
                    : "Add to Favourites",
                shortcut: key("toggleFavorite"),
                disabled: !loaded,
                run: options.toggleFavorite,
            },
            {
                kind: "action",
                label: "Add to Playlist…",
                shortcut: key("addToPlaylist"),
                disabled: !loaded,
                run: options.addToPlaylist,
            },
            sep,
            {
                kind: "action",
                label: "Save Contact Sheet",
                shortcut: key("saveContactSheet"),
                disabled: !local,
                run: options.exportContactSheet,
            },
            {
                kind: "action",
                label: "Export Clip",
                shortcut: key("exportClip"),
                disabled: !local || !hasRange || !options.clipExportAvailable(),
                run: () => options.exportClip(false),
            },
            {
                kind: "action",
                label: "Export GIF",
                shortcut: key("exportGif"),
                disabled: !local || !hasRange,
                run: () => options.exportClip(true),
            },
            {
                kind: "action",
                label: "Open Export Folder",
                shortcut: key("openExportFolder"),
                run: options.openExportFolder,
            },
            sep,
            {
                kind: "action",
                label: "Quit",
                shortcut: key("quitApp"),
                run: options.quit,
            },
        ];
    };

    const buildPlayback = (): MenuNode[] => {
        const loaded = options.isFileLoaded();
        const speed = options.currentSpeed();
        const slomo = options.slomoFactor();
        const ab = options.ab;
        return [
            {
                kind: "action",
                label: options.isPlaying() ? "Pause" : "Play",
                shortcut: key("togglePlayPause"),
                disabled: !loaded,
                run: options.togglePlayPause,
            },
            {
                kind: "action",
                label: "Jump to Start",
                shortcut: key("seekToStart"),
                disabled: !loaded,
                run: options.seekToStart,
            },
            sep,
            {
                kind: "action",
                label: "Previous",
                shortcut: key("previousTrack"),
                run: options.previousTrack,
            },
            {
                kind: "action",
                label: "Next",
                shortcut: key("nextTrack"),
                run: options.nextTrack,
            },
            sep,
            {
                kind: "submenu",
                label: "Seek",
                disabled: !loaded,
                children: [
                    {
                        kind: "action",
                        label: "Back 1 second",
                        shortcut: key("seekBackwardExact"),
                        run: () => options.seekRelative(-1),
                    },
                    {
                        kind: "action",
                        label: "Forward 1 second",
                        shortcut: key("seekForwardExact"),
                        run: () => options.seekRelative(1),
                    },
                    sep,
                    {
                        kind: "action",
                        label: "Back 5 seconds",
                        shortcut: key("seekBackward"),
                        run: () => options.seekRelative(-5),
                    },
                    {
                        kind: "action",
                        label: "Forward 5 seconds",
                        shortcut: key("seekForward"),
                        run: () => options.seekRelative(5),
                    },
                    sep,
                    {
                        kind: "action",
                        label: "Back 60 seconds",
                        shortcut: key("seekBackwardLong"),
                        run: () => options.seekRelative(-60),
                    },
                    {
                        kind: "action",
                        label: "Forward 60 seconds",
                        shortcut: key("seekForwardLong"),
                        run: () => options.seekRelative(60),
                    },
                ],
            },
            {
                kind: "submenu",
                label: "Frame Step",
                disabled: !loaded,
                children: [
                    {
                        kind: "action",
                        label: "Previous Frame",
                        shortcut: key("frameStepBackward"),
                        run: () => options.frameStep(false),
                    },
                    {
                        kind: "action",
                        label: "Next Frame",
                        shortcut: key("frameStepForward"),
                        run: () => options.frameStep(true),
                    },
                ],
            },
            sep,
            {
                kind: "submenu",
                label: "Speed",
                disabled: !loaded,
                children: [
                    ...options.playbackRates().map((rate) => ({
                        kind: "action" as const,
                        label: `${rate}×`,
                        checked: Math.abs(rate - speed) < 0.001,
                        run: () => options.setSpeed(rate),
                    })),
                    sep,
                    {
                        kind: "action",
                        label: "Slower",
                        shortcut: key("speedDown"),
                        run: () => options.setSpeed(Math.max(0.25, speed - 0.25)),
                    },
                    {
                        kind: "action",
                        label: "Faster",
                        shortcut: key("speedUp"),
                        run: () => options.setSpeed(Math.min(4, speed + 0.25)),
                    },
                    {
                        kind: "action",
                        label: "Normal Speed",
                        shortcut: key("resetSpeed"),
                        run: options.resetSpeed,
                    },
                ],
            },
            {
                kind: "submenu",
                label: "Ultra Slo-Mo",
                disabled: !loaded,
                children: [
                    ...([
                        { value: 0.5, label: "½ speed" },
                        { value: 0.25, label: "¼ speed" },
                        { value: 0.125, label: "⅛ speed" },
                    ].map((entry) => ({
                        kind: "action" as const,
                        label: entry.label,
                        checked: Math.abs(entry.value - slomo) < 0.001,
                        run: () => options.setSlomoFactor(entry.value),
                    })) as MenuNode[]),
                    sep,
                    {
                        kind: "action",
                        label: `Hold ${key("ultraSlomoHold") ?? "X"} while playing`,
                        disabled: true,
                        run: () => {},
                    },
                ],
            },
            sep,
            {
                kind: "submenu",
                label: "A-B Range",
                disabled: !loaded,
                children: [
                    {
                        kind: "action",
                        label:
                            ab.pointA.value === null
                                ? "Mark Start"
                                : ab.pointB.value === null
                                  ? "Mark End"
                                  : "Clear Range",
                        shortcut: key("abRangeCycle"),
                        run: () => void ab.cycle(),
                    },
                    {
                        kind: "action",
                        label: "Clear Range",
                        shortcut: key("abRangeClear"),
                        disabled: !ab.isActive.value,
                        run: () => void ab.clear(),
                    },
                    sep,
                    {
                        kind: "action",
                        label: "Loop the Range",
                        checked: ab.loopEnabled.value,
                        run: () => void ab.setLoopEnabled(!ab.loopEnabled.value),
                    },
                ],
            },
            {
                kind: "submenu",
                label: options.scenesAreChapters() ? "Chapters" : "Scenes",
                disabled: !loaded,
                children: [
                    {
                        kind: "action",
                        label: "Next",
                        shortcut: key("nextScene"),
                        disabled: options.sceneCount() < 2,
                        run: options.nextScene,
                    },
                    {
                        kind: "action",
                        label: "Previous",
                        shortcut: key("previousScene"),
                        disabled: options.sceneCount() < 2,
                        run: options.previousScene,
                    },
                    sep,
                    {
                        kind: "action",
                        label: options.sceneCount() > 1 ? `Re-scan (${options.sceneCount()} found)` : "Scan for scenes",
                        run: options.rescanScenes,
                    },
                ],
            },
            {
                kind: "action",
                label: "Loop File",
                shortcut: key("toggleLoop"),
                checked: options.isLoopOne(),
                run: options.toggleLoop,
            },
            sep,
            {
                kind: "submenu",
                label: "Skip Markers",
                children: [
                    {
                        kind: "action",
                        label: "Save A→B as Intro",
                        disabled: !options.hasAbRange(),
                        run: () => {
                            const a = ab.pointA.value;
                            const b = ab.pointB.value;
                            if (a !== null && b !== null) options.skip.saveIntro(a, b);
                        },
                    },
                    {
                        kind: "action",
                        label: "Save A as Credits Start",
                        disabled: ab.pointA.value === null,
                        run: () => {
                            const a = ab.pointA.value;
                            if (a !== null) options.skip.saveCredits(a);
                        },
                    },
                    {
                        kind: "action",
                        label: "Clear Markers for This Folder",
                        disabled:
                            !options.skip.hasIntroMarkers.value &&
                            !options.skip.hasCreditsMarker.value,
                        run: options.skip.clearMarkers,
                    },
                    sep,
                    {
                        kind: "action",
                        label: "Skip Automatically",
                        checked: options.skip.autoSkip.value,
                        run: () =>
                            options.skip.setAutoSkip(!options.skip.autoSkip.value),
                    },
                ],
            },
        ];
    };

    const buildAudio = (): MenuNode[] => {
        const loaded = options.isFileLoaded();
        const audio = options.audio;
        const nightLevels: NightModeLevel[] = ["off", "light", "medium", "strong"];
        return [
            {
                kind: "submenu",
                label: "Audio Track",
                disabled: !loaded,
                children: trackItems(options.audioTracks()),
            },
            {
                kind: "action",
                label: "Add External Audio…",
                disabled: !loaded,
                run: options.addExternalAudio,
            },
            sep,
            {
                kind: "action",
                label: "Volume Up",
                shortcut: key("volumeUp"),
                run: () => options.adjustVolume(5),
            },
            {
                kind: "action",
                label: "Volume Down",
                shortcut: key("volumeDown"),
                run: () => options.adjustVolume(-5),
            },
            {
                kind: "action",
                label: "Mute",
                shortcut: key("toggleMute"),
                checked: options.isMuted(),
                run: options.toggleMuted,
            },
            {
                kind: "submenu",
                label: "Volume Boost",
                children: [100, 125, 150, 200].map((step) => ({
                    kind: "action" as const,
                    label: `${step}%`,
                    checked: audio.state.gain === step,
                    run: () => audio.setGain(step),
                })),
            },
            sep,
            {
                kind: "submenu",
                label: "Night Mode",
                children: nightLevels.map((level) => ({
                    kind: "action" as const,
                    label: NIGHT_MODE_LABELS[level],
                    checked: audio.state.nightMode === level,
                    run: () => void audio.setNightMode(level),
                })),
            },
            {
                kind: "submenu",
                label: "Equalizer",
                children: [
                    {
                        kind: "action",
                        label: "Enabled",
                        checked: audio.state.eqEnabled,
                        run: () => void audio.setEqEnabled(!audio.state.eqEnabled),
                    },
                    sep,
                    ...EQ_PRESETS.map((preset) => ({
                        kind: "action" as const,
                        label: preset.name,
                        checked: audio.state.eqPreset === preset.id,
                        run: () => void audio.applyEqPreset(preset.id),
                    })),
                ],
            },
            {
                kind: "action",
                label: "Audio Panel…",
                shortcut: key("toggleAudioPanel"),
                run: options.openAudioPanel,
            },
            sep,
            {
                kind: "submenu",
                label: "Audio Delay",
                disabled: !loaded,
                children: [
                    {
                        kind: "action",
                        label: "Earlier (−0.1s)",
                        shortcut: key("audioDelayDown"),
                        run: () => options.adjustAudioDelay(-0.1),
                    },
                    {
                        kind: "action",
                        label: "Later (+0.1s)",
                        shortcut: key("audioDelayUp"),
                        run: () => options.adjustAudioDelay(0.1),
                    },
                    { kind: "action", label: "Reset", run: options.resetAudioDelay },
                ],
            },
        ];
    };

    const buildVideo = (): MenuNode[] => {
        const loaded = options.isFileLoaded();
        const local = options.isLocalMedia();
        const enh = options.enhancements;
        const qualities: { id: QualityPreset; label: string }[] = [
            { id: "fast", label: "Fast" },
            { id: "balanced", label: "Balanced" },
            { id: "high", label: "High" },
        ];
        const upscales: { id: AiUpscaleMode; label: string }[] = [
            { id: "off", label: "Off" },
            { id: "anime", label: "Anime" },
            { id: "live", label: "Live-action" },
        ];
        return [
            {
                kind: "action",
                label: "Fullscreen",
                shortcut: key("toggleFullscreen"),
                checked: options.isFullscreen(),
                run: options.toggleFullscreen,
            },
            {
                kind: "action",
                label: "Always on Top",
                shortcut: key("toggleAlwaysOnTop"),
                checked: options.isAlwaysOnTop(),
                run: options.toggleAlwaysOnTop,
            },
            sep,
            {
                kind: "action",
                label: `Aspect Ratio · ${options.aspectLabel()}`,
                shortcut: key("cycleAspectRatio"),
                disabled: !loaded,
                run: options.cycleAspectRatio,
            },
            {
                kind: "submenu",
                label: "Crop",
                disabled: !loaded,
                children: [
                    {
                        kind: "action",
                        label: "Auto-Crop Now",
                        shortcut: key("autoCropNow"),
                        run: options.autoCropNow,
                    },
                    {
                        kind: "action",
                        label: "Clear Crop",
                        shortcut: key("clearCrop"),
                        run: options.clearCrop,
                    },
                ],
            },
            {
                kind: "submenu",
                label: `Zoom · ${options.transform.zoomPercent.value}%`,
                disabled: !loaded,
                children: [
                    {
                        kind: "action",
                        label: "Zoom In",
                        shortcut: key("zoomIn"),
                        run: options.transform.zoomIn,
                    },
                    {
                        kind: "action",
                        label: "Zoom Out",
                        shortcut: key("zoomOut"),
                        run: options.transform.zoomOut,
                    },
                    {
                        kind: "action",
                        label: "Reset Zoom",
                        run: () => void options.transform.resetZoom(),
                    },
                ],
            },
            {
                kind: "submenu",
                label: "Rotate",
                disabled: !loaded,
                children: [
                    {
                        kind: "action",
                        label: "Rotate 90°",
                        shortcut: key("rotateVideo"),
                        run: () => void options.transform.rotateBy(90),
                    },
                    {
                        kind: "action",
                        label: "Rotate 180°",
                        run: () => void options.transform.rotateBy(180),
                    },
                    sep,
                    {
                        kind: "action",
                        label: "Reset Zoom & Rotation",
                        shortcut: key("resetTransform"),
                        run: () => void options.transform.reset(),
                    },
                ],
            },
            sep,
            {
                kind: "action",
                label: "Fit Window to Video",
                shortcut: key("fitWindowToVideo"),
                disabled: !loaded,
                run: options.fitWindowToVideo,
            },
            {
                kind: "submenu",
                label: "Window Size",
                children: [
                    {
                        kind: "action",
                        label: "Grow",
                        shortcut: key("windowSizeUp"),
                        run: () => options.stepWindowSize(1.1),
                    },
                    {
                        kind: "action",
                        label: "Shrink",
                        shortcut: key("windowSizeDown"),
                        run: () => options.stepWindowSize(0.9),
                    },
                ],
            },
            sep,
            {
                kind: "submenu",
                label: "Quality",
                children: qualities.map((entry) => ({
                    kind: "action" as const,
                    label: entry.label,
                    checked: enh.state.qualityPreset === entry.id,
                    run: () => void enh.setQualityPreset(entry.id),
                })),
            },
            {
                kind: "submenu",
                label: "AI Upscale",
                children: upscales.map((entry) => ({
                    kind: "action" as const,
                    label: entry.label,
                    checked: enh.state.aiUpscale === entry.id,
                    run: () => void enh.setAiUpscale(entry.id),
                })),
            },
            {
                kind: "submenu",
                label: "Enhance",
                disabled: !loaded,
                children: [
                    {
                        kind: "action",
                        label: "Denoise",
                        checked: enh.state.denoise,
                        run: () => void enh.setDenoise(!enh.state.denoise),
                    },
                    {
                        kind: "action",
                        label: "Deinterlace",
                        checked: enh.state.deinterlace,
                        run: () => void enh.setDeinterlace(!enh.state.deinterlace),
                    },
                ],
            },
            {
                kind: "submenu",
                label: "Look Presets",
                disabled: !loaded,
                children: options.videoPresets.allPresets.value.length
                    ? options.videoPresets.allPresets.value.map((preset) => ({
                          kind: "action" as const,
                          label: preset.name,
                          run: () => void options.videoPresets.apply(preset),
                      }))
                    : [
                          {
                              kind: "action",
                              label: "No presets",
                              disabled: true,
                              run: () => {},
                          },
                      ],
            },
            {
                kind: "submenu",
                label: "Deband",
                disabled: !loaded,
                children: (["off", "light", "medium", "strong"] as const).map(
                    (level) => ({
                        kind: "action" as const,
                        label: level[0].toUpperCase() + level.slice(1),
                        checked: enh.state.deband === level,
                        run: () => void enh.setDeband(level),
                    }),
                ),
            },
            {
                kind: "action",
                label: "Old Film Restore",
                disabled: !loaded,
                run: () => void enh.applyOldFilmRestore(),
            },
            {
                kind: "action",
                label: "Before / after split view",
                shortcut: key("toggleSplitCompare"),
                disabled: !loaded,
                run: options.toggleSplitCompare,
            },
            {
                kind: "action",
                label: "Curves…",
                shortcut: key("toggleCurves"),
                disabled: !loaded,
                run: options.openCurves,
            },
            {
                kind: "action",
                label: "Auto Enhance",
                disabled: !loaded,
                run: options.autoEnhance,
            },
            {
                kind: "action",
                label: "Reset Video Settings",
                disabled: !loaded,
                run: options.resetVideoSettings,
            },
            sep,
            {
                kind: "action",
                label: "Snapshot",
                shortcut: key("screenshotWithSubtitles"),
                disabled: !loaded,
                run: () => options.takeScreenshot(true),
            },
            {
                kind: "action",
                label: "Snapshot without Subtitles",
                shortcut: key("screenshotVideoOnly"),
                disabled: !loaded,
                run: () => options.takeScreenshot(false),
            },
            {
                kind: "action",
                label: "Regenerate Seek Thumbnails",
                disabled: !local,
                run: options.regenerateThumbnails,
            },
        ];
    };

    const buildSubtitle = (): MenuNode[] => {
        const loaded = options.isFileLoaded();
        return [
            {
                kind: "submenu",
                label: "Subtitle Track",
                disabled: !loaded,
                children: trackItems(options.subtitleTracks()),
            },
            {
                kind: "action",
                label: "Add Subtitle File…",
                disabled: !loaded,
                run: options.addExternalSubtitle,
            },
            {
                kind: "action",
                label: "Find Online Subtitles…",
                disabled: !loaded,
                run: options.findOnlineSubtitles,
            },
            sep,
            {
                kind: "action",
                label: "Show Subtitles",
                shortcut: key("toggleSubtitleVisibility"),
                checked: options.areSubtitlesVisible(),
                disabled: !loaded,
                run: options.toggleSubtitleVisibility,
            },
            {
                kind: "action",
                label: "Dual Subtitles",
                checked: options.isDualSubEnabled(),
                disabled: !loaded,
                run: () => options.setDualSubEnabled(!options.isDualSubEnabled()),
            },
            sep,
            {
                kind: "submenu",
                label: "Subtitle Delay",
                disabled: !loaded,
                children: [
                    {
                        kind: "action",
                        label: "Earlier (−0.1s)",
                        shortcut: key("subtitleDelayDown"),
                        run: () => options.adjustSubtitleDelay(-0.1),
                    },
                    {
                        kind: "action",
                        label: "Later (+0.1s)",
                        shortcut: key("subtitleDelayUp"),
                        run: () => options.adjustSubtitleDelay(0.1),
                    },
                    { kind: "action", label: "Reset", run: options.resetSubtitleDelay },
                ],
            },
            {
                kind: "action",
                label: "Sync to this moment",
                shortcut: key("syncSubtitlesByEar"),
                disabled: !loaded,
                run: options.syncSubtitlesByEar,
            },
            {
                kind: "action",
                label: "Advanced Subtitle Settings…",
                disabled: !loaded,
                run: options.openSubtitleAdvanced,
            },
        ];
    };

    const buildTools = (): MenuNode[] => [
        { kind: "action", label: "Preferences…", run: options.openSettings },
        {
            kind: "action",
            label: "Keyboard Shortcuts…",
            run: options.toggleShortcutsHelp,
        },
        sep,
        {
            kind: "action",
            label: "Media Information",
            shortcut: key("toggleInfo"),
            disabled: !options.isFileLoaded(),
            run: options.toggleInfo,
        },
        {
            kind: "action",
            label: "Open Export Folder",
            run: options.openExportFolder,
        },
    ];

    const buildView = (): MenuNode[] => [
        {
            kind: "action",
            label: "Playlist",
            shortcut: key("togglePlaylist"),
            run: options.togglePlaylist,
        },
        { kind: "action", label: "Home", run: () => options.gotoPanel("home") },
        { kind: "action", label: "History", run: () => options.gotoPanel("history") },
        {
            kind: "action",
            label: "Favourites",
            run: () => options.gotoPanel("favorites"),
        },
        { kind: "action", label: "Network", run: () => options.gotoPanel("network") },
        sep,
        {
            kind: "action",
            label: "Curves Panel",
            shortcut: key("toggleCurves"),
            disabled: !options.isFileLoaded(),
            run: options.openCurves,
        },
        {
            kind: "action",
            label: "Audio Panel",
            shortcut: key("toggleAudioPanel"),
            run: options.openAudioPanel,
        },
        sep,
        {
            kind: "action",
            label: "Show Position",
            shortcut: key("showProgress"),
            disabled: !options.isFileLoaded(),
            run: options.showProgress,
        },
        { kind: "action", label: "Hide Menu Bar", run: options.hideMenuBar },
    ];

    const buildHelp = (): MenuNode[] => [
        {
            kind: "action",
            label: "Keyboard Shortcuts",
            run: options.toggleShortcutsHelp,
        },
        { kind: "action", label: "Check for Updates", run: options.checkForUpdates },
        sep,
        { kind: "action", label: "About Lumo", run: options.showAbout },
    ];

    // Rebuilt on every dependency change; MenuBar also asks for a refresh as a
    // menu opens so track lists are never stale.
    const menus = computed<MenuTopLevel[]>(() => [
        { label: "Media", children: buildMedia() },
        { label: "Playback", children: buildPlayback() },
        { label: "Audio", children: buildAudio() },
        { label: "Video", children: buildVideo() },
        { label: "Subtitle", children: buildSubtitle() },
        { label: "Tools", children: buildTools() },
        { label: "View", children: buildView() },
        { label: "Help", children: buildHelp() },
    ]);

    return { menus };
};

/** Formats a stored chord for display in a menu row. */
export const menuShortcut = (chord: string | undefined): string | undefined => {
    if (!chord) return undefined;
    const label = formatChord(chord);
    return label === "—" ? undefined : label;
};
