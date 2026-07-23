import { computed } from "vue";
import type { AdjustSpec, CommandDef } from "../types/commands";
import type { AudioEnhancementsController } from "./useAudioEnhancements";
import { EQ_PRESETS, NIGHT_MODE_LABELS } from "./useAudioEnhancements";
import type { VideoEnhancementsController } from "./useVideoEnhancements";
import type { VideoPresetsController } from "./useVideoPresets";
import type { VideoTransformController } from "./useVideoTransform";

// The bindable command registry.
//
// This deliberately covers what the built-in shortcut list does NOT: the panels
// and popovers, the presets, and above all every numeric setting. Sliders can't
// live in a menu, which is exactly why they need to be bindable here — a menu
// harvest alone would leave sharpness and grain unreachable.
//
// Adding a new slider anywhere in the app means adding one `adjust` entry here,
// and it immediately becomes bindable (and palette-searchable) with no changes
// to the shortcut code.

type ColorAdjustments = {
    brightness: () => number;
    contrast: () => number;
    saturation: () => number;
    gamma: () => number;
    hue: () => number;
    setBrightness: (v: number) => void | Promise<void>;
    setContrast: (v: number) => void | Promise<void>;
    setSaturation: (v: number) => void | Promise<void>;
    setGamma: (v: number) => void | Promise<void>;
    setHue: (v: number) => void | Promise<void>;
    audioDelay: () => number;
    setAudioDelay: (v: number) => void | Promise<void>;
    subDelay: () => number;
    setSubDelay: (v: number) => void | Promise<void>;
};

export type CommandRegistryOptions = {
    enhancements: VideoEnhancementsController;
    audio: AudioEnhancementsController;
    transform: VideoTransformController;
    videoPresets: VideoPresetsController;
    adjustments: ColorAdjustments;

    // UI surfaces
    openVideoPanel: () => void;
    openAudioTrackMenu: () => void;
    openSubtitleMenu: () => void;
    openSpeedMenu: () => void;
    openCurves: () => void;
    openAudioPanel: () => void;
    openPlaylist: () => void;
    openSettings: () => void;
    openMediaInfo: () => void;
    openShortcutsHelp: () => void;
    closePanels: () => void;

    // Playback numerics
    getSpeed: () => number;
    setSpeed: (value: number) => void | Promise<void>;
    getVolume: () => number;
    setVolume: (value: number) => void | Promise<void>;

    // Misc discrete actions worth binding
    autoEnhance: () => void;
    resetVideoSettings: () => void;
    cycleAspectRatio: () => void;
    autoCropNow: () => void;
    clearCrop: () => void;

    toggleSplitCompare: () => void;
    getSplitPosition: () => number;
    setSplitPosition: (value: number) => void;
    oldFilmRestore: () => void;
    setDeband: (level: "off" | "light" | "medium" | "strong") => void;
    syncSubtitlesByEar: () => void;
    nextScene: () => void;
    previousScene: () => void;
    rescanScenes: () => void;
};

export const useCommandRegistry = (options: CommandRegistryOptions) => {
    const commands = computed<CommandDef[]>(() => {
        const list: CommandDef[] = [];
        const enh = options.enhancements;
        const audio = options.audio;
        const adj = options.adjustments;

        const action = (
            id: string,
            label: string,
            group: string,
            run: () => void | Promise<void>,
        ) => list.push({ kind: "action", id, label, group, run });

        const adjust = (
            id: string,
            label: string,
            group: string,
            spec: AdjustSpec,
            get: () => number,
            set: (value: number) => void | Promise<void>,
        ) => list.push({ kind: "adjust", id, label, group, spec, get, set });

        // --- Panels & popovers ------------------------------------------------
        action("ui.videoPanel", "Open Video panel", "Panels", options.openVideoPanel);
        action("ui.audioTrackMenu", "Open Audio menu", "Panels", options.openAudioTrackMenu);
        action("ui.subtitleMenu", "Open Subtitle menu", "Panels", options.openSubtitleMenu);
        action("ui.speedMenu", "Open Speed menu", "Panels", options.openSpeedMenu);
        action("ui.curves", "Open Curves panel", "Panels", options.openCurves);
        action("ui.audioPanel", "Open Audio panel", "Panels", options.openAudioPanel);
        action("ui.playlist", "Open Playlist", "Panels", options.openPlaylist);
        action("ui.settings", "Open Settings", "Panels", options.openSettings);
        action("ui.mediaInfo", "Open Media information", "Panels", options.openMediaInfo);
        action("ui.shortcutsHelp", "Open Keyboard shortcuts", "Panels", options.openShortcutsHelp);
        action("ui.closePanels", "Close all panels", "Panels", options.closePanels);

        // --- Video adjustables ------------------------------------------------
        const pct = { min: 0, max: 100, step: 10, unit: "%", precision: 0 };
        const bipolar = { min: -100, max: 100, step: 10, unit: "%", precision: 0 };

        adjust("video.sharpness", "Sharpness", "Video", pct,
            () => enh.state.sharpenAmount, (v) => enh.setSharpenAmount(v));
        adjust("video.sharpenRadius", "Sharpen radius", "Video",
            { min: 1, max: 30, step: 1, unit: "px", precision: 0 },
            () => enh.state.sharpenRadius, (v) => enh.setSharpenRadius(v));
        adjust("video.grain", "Film grain", "Video", pct,
            () => enh.state.grain, (v) => enh.setGrain(v));

        adjust("video.brightness", "Brightness", "Video", bipolar,
            adj.brightness, adj.setBrightness);
        adjust("video.contrast", "Contrast", "Video", bipolar,
            adj.contrast, adj.setContrast);
        adjust("video.saturation", "Saturation", "Video", bipolar,
            adj.saturation, adj.setSaturation);
        adjust("video.gamma", "Gamma", "Video", bipolar, adj.gamma, adj.setGamma);
        adjust("video.hue", "Hue", "Video", bipolar, adj.hue, adj.setHue);

        adjust("grade.exposure", "Exposure", "Colour grade", bipolar,
            () => enh.state.exposure, (v) => enh.setColorGrade("exposure", v));
        adjust("grade.temperature", "Temperature", "Colour grade", bipolar,
            () => enh.state.temperature, (v) => enh.setColorGrade("temperature", v));
        adjust("grade.tint", "Tint", "Colour grade", bipolar,
            () => enh.state.tint, (v) => enh.setColorGrade("tint", v));
        adjust("grade.highlights", "Highlights", "Colour grade", bipolar,
            () => enh.state.highlights, (v) => enh.setColorGrade("highlights", v));
        adjust("grade.shadows", "Shadows", "Colour grade", bipolar,
            () => enh.state.shadows, (v) => enh.setColorGrade("shadows", v));

        adjust("video.zoom", "Zoom", "Video",
            { min: 50, max: 800, step: 10, unit: "%", precision: 0 },
            () => options.transform.zoomPercent.value,
            (v) => {
                // The transform stores log2 zoom; convert from the percentage
                // the user thinks in.
                const target = Math.log2(Math.max(1, v) / 100);
                options.transform.zoomBy(target - options.transform.state.zoom);
            });

        // --- Audio adjustables ------------------------------------------------
        adjust("audio.volume", "Volume", "Audio",
            { min: 0, max: 100, step: 5, unit: "%", precision: 0 },
            options.getVolume, options.setVolume);
        adjust("audio.dialogueBoost", "Dialogue boost", "Audio", pct,
            () => audio.state.dialogueBoost, (v) => audio.setDialogueBoost(v));
        adjust("audio.gain", "Volume boost", "Audio",
            { min: 100, max: 300, step: 25, unit: "%", precision: 0 },
            () => audio.state.gain, (v) => audio.setGain(v));
        adjust("audio.delay", "Audio delay", "Audio",
            { min: -10, max: 10, step: 0.1, unit: "s", precision: 1 },
            adj.audioDelay, adj.setAudioDelay);
        adjust("subtitle.delay", "Subtitle delay", "Audio",
            { min: -10, max: 10, step: 0.1, unit: "s", precision: 1 },
            adj.subDelay, adj.setSubDelay);

        // --- Playback ---------------------------------------------------------
        adjust("playback.speed", "Playback speed", "Playback",
            { min: 0.25, max: 4, step: 0.25, unit: "×", precision: 2 },
            options.getSpeed, options.setSpeed);

        action("video.autoEnhance", "Auto Enhance", "Video", options.autoEnhance);
        action("video.resetSettings", "Reset video settings", "Video", options.resetVideoSettings);
        action("video.cycleAspect", "Cycle aspect ratio", "Video", options.cycleAspectRatio);
        action("video.autoCrop", "Auto-crop now", "Video", options.autoCropNow);
        action("video.clearCrop", "Clear crop", "Video", options.clearCrop);
        action("video.denoiseToggle", "Toggle denoise", "Video",
            () => void enh.setDenoise(!enh.state.denoise));
        action("video.deinterlaceToggle", "Toggle deinterlace", "Video",
            () => void enh.setDeinterlace(!enh.state.deinterlace));

        action("video.splitCompare", "Before / after split view", "Video",
            options.toggleSplitCompare);
        adjust("video.splitPosition", "Split position", "Video",
            { min: 0, max: 100, step: 5, unit: "%", precision: 0 },
            () => options.getSplitPosition() * 100,
            (v) => options.setSplitPosition(v / 100));

        (["off", "light", "medium", "strong"] as const).forEach((level) =>
            action(`video.deband.${level}`, `Deband · ${level}`, "Video",
                () => options.setDeband(level)));
        action("video.oldFilmRestore", "Old Film Restore", "Video",
            options.oldFilmRestore);

        action("subtitle.syncByEar", "Sync subtitles to this moment", "Subtitles",
            options.syncSubtitlesByEar);

        action("scene.next", "Next scene", "Scenes", options.nextScene);
        action("scene.previous", "Previous scene", "Scenes", options.previousScene);
        action("scene.rescan", "Scan for scenes", "Scenes", options.rescanScenes);

        // --- Presets ----------------------------------------------------------
        (["fast", "balanced", "high"] as const).forEach((preset) =>
            action(`video.quality.${preset}`, `Quality · ${preset}`, "Presets",
                () => void enh.setQualityPreset(preset)));
        (["off", "anime", "live"] as const).forEach((mode) =>
            action(`video.upscale.${mode}`, `AI Upscale · ${mode}`, "Presets",
                () => void enh.setAiUpscale(mode)));
        options.videoPresets.allPresets.value.forEach((preset) =>
            action(`video.preset.${preset.id}`, `Look · ${preset.name}`, "Presets",
                () => void options.videoPresets.apply(preset)));
        (["off", "light", "medium", "strong"] as const).forEach((level) =>
            action(`audio.night.${level}`, `Night mode · ${NIGHT_MODE_LABELS[level]}`,
                "Presets", () => void audio.setNightMode(level)));
        EQ_PRESETS.forEach((preset) =>
            action(`audio.eq.${preset.id}`, `Equalizer · ${preset.name}`, "Presets",
                () => void audio.applyEqPreset(preset.id)));

        return list;
    });

    const byId = computed(() => {
        const map = new Map<string, CommandDef>();
        commands.value.forEach((command) => map.set(command.id, command));
        return map;
    });

    return { commands, byId };
};

export type CommandRegistry = ReturnType<typeof useCommandRegistry>;
