import { computed, onMounted, onUnmounted, ref } from "vue";
import {
    SEEK_STEP_SETTING_LABEL,
    SETTINGS_UPDATED_EVENT,
} from "../mock/settings";
import {
    buildDefaultShortcutMap,
    chordFromEvent,
    parseShortcutBindings,
    UNBOUND_CHORD,
    type ShortcutActionId,
} from "../constants/shortcuts";
import { loadUiState } from "./useUiStateStore";

type PlaybackShortcutApi = {
    state: {
        media: {
            isFileLoaded: boolean;
            isLivePlayback: boolean;
        };
        playback: {
            duration: number;
            volume: number;
        };
        window: {
            isFullscreen: boolean;
        };
    };
    togglePlayPause: () => Promise<void>;
    seekRelative: (position: number, exact?: boolean) => Promise<void>;
    setVolume: (volume: number) => Promise<void>;
};

export type PlaybackShortcutActions = {
    toggleFullscreen: () => Promise<void> | void;
    toggleInfo: () => void;
    seekOverlay?: (deltaSeconds: number) => void;
    volumeOverlay?: (volume: number) => void;
    toggleMuted?: () => Promise<void> | void;
    seekAbsolute?: (positionSeconds: number) => Promise<void> | void;
    frameStep?: (direction: 1 | -1) => Promise<void> | void;
    stepPlaybackSpeed?: (direction: 1 | -1) => Promise<void> | void;
    resetPlaybackSpeed?: () => Promise<void> | void;
    cycleSubtitleTrack?: (direction: 1 | -1) => Promise<void> | void;
    toggleSubtitleVisibility?: () => Promise<void> | void;
    cycleAudioTrack?: () => Promise<void> | void;
    adjustSubtitleDelay?: (deltaSeconds: number) => Promise<void> | void;
    adjustAudioDelay?: (deltaSeconds: number) => Promise<void> | void;
    takeScreenshot?: (includeSubtitles: boolean) => Promise<void> | void;
    previousTrack?: () => Promise<void> | void;
    nextTrack?: () => Promise<void> | void;
    toggleLoop?: () => Promise<void> | void;
    autoCropNow?: () => Promise<void> | void;
    clearCrop?: () => Promise<void> | void;
    togglePlaylist?: () => void;
    toggleAlwaysOnTop?: () => Promise<void> | void;
    toggleFavorite?: () => Promise<void> | void;
    cycleAspectRatio?: () => Promise<void> | void;
    fitWindowToVideo?: () => Promise<void> | void;
    toggleCurves?: () => void;
    cycleNightMode?: () => Promise<void> | void;
    toggleAudioPanel?: () => void;
    zoomIn?: () => void;
    zoomOut?: () => void;
    rotateVideo?: () => Promise<void> | void;
    resetTransform?: () => Promise<void> | void;
    abRangeCycle?: () => Promise<void> | void;
    abRangeClear?: () => Promise<void> | void;
    skipIntro?: () => void;
    openFile?: () => void;
    openFileOrFolder?: () => void;
    openNetworkStream?: () => void;
    showRecent?: () => void;
    showFavourites?: () => void;
    addToPlaylist?: () => void;
    saveContactSheet?: () => void;
    exportClip?: () => void;
    exportGif?: () => void;
    openExportFolder?: () => void;
    quitApp?: () => void;
    commandPalette?: () => void;
    toggleSplitCompare?: () => void;
    syncSubtitlesByEar?: () => void;
    nextScene?: () => void;
    previousScene?: () => void;
    /** Runs a user-defined shortcut; true when it claimed the chord. */
    runCustomShortcut?: (chord: string) => boolean;
    /** Gates the export entries the same way the clip bar does. */
    canExportRange?: () => boolean;
    canExportClip?: () => boolean;
    isLocalMedia?: () => boolean;
    windowSizeUp?: () => Promise<void> | void;
    windowSizeDown?: () => Promise<void> | void;
    startUltraSlomo?: () => void;
    stopUltraSlomo?: () => void;
    showProgress?: () => void;
    toggleShortcutsHelp?: () => void;
    closeShortcutsHelp?: () => boolean;
    /** Dismiss the topmost open panel/dialog; false when nothing was open. */
    closeTopOverlay?: () => boolean;
    isShortcutsHelpOpen?: () => boolean;
};

type StoredSettingGroup = {
    title: string;
    items: Array<{ label: string; value: string }>;
};

type ActionHandler = {
    enabled: () => boolean;
    /** Screenshots opt out so a held key doesn't spray files; most repeat. */
    allowRepeat?: boolean;
    run: (event: KeyboardEvent) => void | Promise<void>;
};

const DEFAULT_SEEK_STEP_SECONDS = 5;
const LONG_SEEK_STEP_SECONDS = 60;
const EXACT_SEEK_STEP_SECONDS = 1;
const VOLUME_STEP = 5;
const SUB_DELAY_STEP_SECONDS = 0.1;
const AUDIO_DELAY_STEP_SECONDS = 0.1;

const parseSeekStepSeconds = (groups?: StoredSettingGroup[]): number => {
    const rawValue = groups
        ?.flatMap((group) => group.items)
        .find((item) => item.label === SEEK_STEP_SETTING_LABEL)?.value;
    const parsed = Number.parseFloat(rawValue ?? "");
    if (!Number.isFinite(parsed) || parsed <= 0) return DEFAULT_SEEK_STEP_SECONDS;
    return parsed;
};

export const usePlaybackShortcuts = (
    player: PlaybackShortcutApi,
    actions: PlaybackShortcutActions,
) => {
    let clickTimer: number | null = null;
    const seekStepSeconds = ref(DEFAULT_SEEK_STEP_SECONDS);
    const bindings = ref<Record<ShortcutActionId, string>>(
        buildDefaultShortcutMap(),
    );

    // Reverse index: chord -> action. Rebuilt whenever bindings change.
    const chordToAction = computed(() => {
        const map = new Map<string, ShortcutActionId>();
        (Object.entries(bindings.value) as Array<[ShortcutActionId, string]>).forEach(
            ([id, chord]) => {
                if (chord && chord !== UNBOUND_CHORD) {
                    map.set(chord, id);
                }
            },
        );
        return map;
    });

    const applyStoredGroups = (groups?: StoredSettingGroup[]) => {
        seekStepSeconds.value = parseSeekStepSeconds(groups);
        bindings.value = parseShortcutBindings(groups);
    };

    const refreshFromSettings = async () => {
        const stored = await loadUiState<{
            settings?: {
                groups?: StoredSettingGroup[];
            };
        }>();
        applyStoredGroups(stored?.settings?.groups);
    };

    const onSettingsUpdated = (event: Event) => {
        const customEvent = event as CustomEvent<{ groups?: StoredSettingGroup[] }>;
        applyStoredGroups(customEvent.detail?.groups);
    };

    onMounted(() => {
        void refreshFromSettings();
        window.addEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    onUnmounted(() => {
        window.removeEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    const isNonUiTarget = (target: HTMLElement | null) => {
        if (!target) return false;
        if (target.closest(".player-controls")) return false;
        if (target.closest(".top-bar")) return false;
        if (target.closest(".main-panels")) return false;
        return true;
    };

    const onDoubleClick = async (event: MouseEvent) => {
        if (!isNonUiTarget(event.target as HTMLElement | null)) return;
        if (clickTimer !== null) {
            window.clearTimeout(clickTimer);
            clickTimer = null;
        }
        await actions.toggleFullscreen();
    };

    const isEditableTarget = (target: HTMLElement | null) => {
        const tag = target?.tagName?.toLowerCase();
        return (
            tag === "input" ||
            tag === "textarea" ||
            (target !== null && target.isContentEditable)
        );
    };

    const isFileLoaded = () => player.state.media.isFileLoaded;

    const canSeek = () =>
        player.state.media.isFileLoaded &&
        !player.state.media.isLivePlayback &&
        player.state.playback.duration > 0;

    const doSeek = async (delta: number, exact = false) => {
        actions.seekOverlay?.(delta);
        await player.seekRelative(delta, exact);
    };

    const doVolume = async (delta: number) => {
        await player.setVolume(player.state.playback.volume + delta);
        actions.volumeOverlay?.(player.state.playback.volume);
    };

    // Each action's guard (`enabled`) mirrors the original hardcoded behavior:
    // playback-affecting actions require a loaded file, seeks require a seekable
    // timeline, and window/UI toggles are always available. Optional callbacks
    // are treated as disabled when absent so their chord isn't consumed.
    const handlers: Partial<Record<ShortcutActionId, ActionHandler>> = {
        togglePlayPause: {
            enabled: () => true,
            run: () => player.togglePlayPause(),
        },
        seekBackward: {
            enabled: canSeek,
            run: () => doSeek(-seekStepSeconds.value),
        },
        seekForward: {
            enabled: canSeek,
            run: () => doSeek(seekStepSeconds.value),
        },
        seekBackwardExact: {
            enabled: canSeek,
            run: () => doSeek(-EXACT_SEEK_STEP_SECONDS, true),
        },
        seekForwardExact: {
            enabled: canSeek,
            run: () => doSeek(EXACT_SEEK_STEP_SECONDS, true),
        },
        seekBackwardLong: {
            enabled: canSeek,
            run: () => doSeek(-LONG_SEEK_STEP_SECONDS),
        },
        seekForwardLong: {
            enabled: canSeek,
            run: () => doSeek(LONG_SEEK_STEP_SECONDS),
        },
        seekToStart: {
            enabled: () => Boolean(actions.seekAbsolute) && canSeek(),
            run: () => actions.seekAbsolute?.(0),
        },
        frameStepBackward: {
            enabled: () => Boolean(actions.frameStep) && isFileLoaded(),
            run: () => actions.frameStep?.(-1),
        },
        frameStepForward: {
            enabled: () => Boolean(actions.frameStep) && isFileLoaded(),
            run: () => actions.frameStep?.(1),
        },
        speedDown: {
            enabled: () => Boolean(actions.stepPlaybackSpeed) && isFileLoaded(),
            run: () => actions.stepPlaybackSpeed?.(-1),
        },
        speedUp: {
            enabled: () => Boolean(actions.stepPlaybackSpeed) && isFileLoaded(),
            run: () => actions.stepPlaybackSpeed?.(1),
        },
        resetSpeed: {
            enabled: () => Boolean(actions.resetPlaybackSpeed) && isFileLoaded(),
            run: () => actions.resetPlaybackSpeed?.(),
        },
        toggleLoop: {
            enabled: () => Boolean(actions.toggleLoop) && isFileLoaded(),
            run: () => actions.toggleLoop?.(),
        },
        autoCropNow: {
            enabled: () => Boolean(actions.autoCropNow) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.autoCropNow?.(),
        },
        clearCrop: {
            enabled: () => Boolean(actions.clearCrop) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.clearCrop?.(),
        },
        volumeUp: {
            enabled: isFileLoaded,
            run: () => doVolume(VOLUME_STEP),
        },
        volumeDown: {
            enabled: isFileLoaded,
            run: () => doVolume(-VOLUME_STEP),
        },
        toggleMute: {
            enabled: () => Boolean(actions.toggleMuted) && isFileLoaded(),
            run: () => actions.toggleMuted?.(),
        },
        cycleSubtitleForward: {
            enabled: () => Boolean(actions.cycleSubtitleTrack) && isFileLoaded(),
            run: () => actions.cycleSubtitleTrack?.(1),
        },
        cycleSubtitleBackward: {
            enabled: () => Boolean(actions.cycleSubtitleTrack) && isFileLoaded(),
            run: () => actions.cycleSubtitleTrack?.(-1),
        },
        toggleSubtitleVisibility: {
            enabled: () =>
                Boolean(actions.toggleSubtitleVisibility) && isFileLoaded(),
            run: () => actions.toggleSubtitleVisibility?.(),
        },
        subtitleDelayDown: {
            enabled: () => Boolean(actions.adjustSubtitleDelay) && isFileLoaded(),
            run: () => actions.adjustSubtitleDelay?.(-SUB_DELAY_STEP_SECONDS),
        },
        subtitleDelayUp: {
            enabled: () => Boolean(actions.adjustSubtitleDelay) && isFileLoaded(),
            run: () => actions.adjustSubtitleDelay?.(SUB_DELAY_STEP_SECONDS),
        },
        cycleAudioTrack: {
            enabled: () => Boolean(actions.cycleAudioTrack) && isFileLoaded(),
            run: () => actions.cycleAudioTrack?.(),
        },
        audioDelayDown: {
            enabled: () => Boolean(actions.adjustAudioDelay) && isFileLoaded(),
            run: () => actions.adjustAudioDelay?.(-AUDIO_DELAY_STEP_SECONDS),
        },
        audioDelayUp: {
            enabled: () => Boolean(actions.adjustAudioDelay) && isFileLoaded(),
            run: () => actions.adjustAudioDelay?.(AUDIO_DELAY_STEP_SECONDS),
        },
        screenshotWithSubtitles: {
            enabled: () => Boolean(actions.takeScreenshot) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.takeScreenshot?.(true),
        },
        screenshotVideoOnly: {
            enabled: () => Boolean(actions.takeScreenshot) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.takeScreenshot?.(false),
        },
        toggleFullscreen: {
            enabled: () => true,
            run: () => actions.toggleFullscreen(),
        },
        toggleInfo: {
            enabled: isFileLoaded,
            run: () => actions.toggleInfo(),
        },
        showProgress: {
            enabled: () => Boolean(actions.showProgress) && isFileLoaded(),
            run: () => actions.showProgress?.(),
        },
        togglePlaylist: {
            enabled: () => Boolean(actions.togglePlaylist),
            run: () => actions.togglePlaylist?.(),
        },
        previousTrack: {
            enabled: () => Boolean(actions.previousTrack),
            run: () => actions.previousTrack?.(),
        },
        nextTrack: {
            enabled: () => Boolean(actions.nextTrack),
            run: () => actions.nextTrack?.(),
        },
        toggleAlwaysOnTop: {
            enabled: () => Boolean(actions.toggleAlwaysOnTop),
            run: () => actions.toggleAlwaysOnTop?.(),
        },
        toggleFavorite: {
            enabled: () => Boolean(actions.toggleFavorite) && isFileLoaded(),
            run: () => actions.toggleFavorite?.(),
        },
        cycleAspectRatio: {
            enabled: () => Boolean(actions.cycleAspectRatio) && isFileLoaded(),
            run: () => actions.cycleAspectRatio?.(),
        },
        fitWindowToVideo: {
            enabled: () => Boolean(actions.fitWindowToVideo) && isFileLoaded(),
            run: () => actions.fitWindowToVideo?.(),
        },
        toggleCurves: {
            enabled: () => Boolean(actions.toggleCurves) && isFileLoaded(),
            run: () => actions.toggleCurves?.(),
        },
        cycleNightMode: {
            enabled: () => Boolean(actions.cycleNightMode),
            allowRepeat: false,
            run: () => actions.cycleNightMode?.(),
        },
        toggleAudioPanel: {
            enabled: () => Boolean(actions.toggleAudioPanel),
            allowRepeat: false,
            run: () => actions.toggleAudioPanel?.(),
        },
        zoomIn: {
            enabled: () => Boolean(actions.zoomIn) && isFileLoaded(),
            run: () => actions.zoomIn?.(),
        },
        zoomOut: {
            enabled: () => Boolean(actions.zoomOut) && isFileLoaded(),
            run: () => actions.zoomOut?.(),
        },
        rotateVideo: {
            enabled: () => Boolean(actions.rotateVideo) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.rotateVideo?.(),
        },
        resetTransform: {
            enabled: () => Boolean(actions.resetTransform) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.resetTransform?.(),
        },
        abRangeCycle: {
            enabled: () => Boolean(actions.abRangeCycle) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.abRangeCycle?.(),
        },
        abRangeClear: {
            enabled: () => Boolean(actions.abRangeClear) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.abRangeClear?.(),
        },
        skipIntro: {
            enabled: () => Boolean(actions.skipIntro) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.skipIntro?.(),
        },

        // --- File & Export -------------------------------------------------
        // These stay available with no file loaded; that's the point of them.
        openFile: {
            enabled: () => Boolean(actions.openFile),
            allowRepeat: false,
            run: () => actions.openFile?.(),
        },
        openFileOrFolder: {
            enabled: () => Boolean(actions.openFileOrFolder),
            allowRepeat: false,
            run: () => actions.openFileOrFolder?.(),
        },
        openNetworkStream: {
            enabled: () => Boolean(actions.openNetworkStream),
            allowRepeat: false,
            run: () => actions.openNetworkStream?.(),
        },
        showRecent: {
            enabled: () => Boolean(actions.showRecent),
            allowRepeat: false,
            run: () => actions.showRecent?.(),
        },
        showFavourites: {
            enabled: () => Boolean(actions.showFavourites),
            allowRepeat: false,
            run: () => actions.showFavourites?.(),
        },
        addToPlaylist: {
            enabled: () => Boolean(actions.addToPlaylist) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.addToPlaylist?.(),
        },
        saveContactSheet: {
            enabled: () =>
                Boolean(actions.saveContactSheet) &&
                (actions.isLocalMedia?.() ?? false),
            allowRepeat: false,
            run: () => actions.saveContactSheet?.(),
        },
        exportClip: {
            enabled: () =>
                Boolean(actions.exportClip) &&
                (actions.canExportRange?.() ?? false) &&
                (actions.canExportClip?.() ?? false),
            allowRepeat: false,
            run: () => actions.exportClip?.(),
        },
        exportGif: {
            enabled: () =>
                Boolean(actions.exportGif) && (actions.canExportRange?.() ?? false),
            allowRepeat: false,
            run: () => actions.exportGif?.(),
        },
        openExportFolder: {
            enabled: () => Boolean(actions.openExportFolder),
            allowRepeat: false,
            run: () => actions.openExportFolder?.(),
        },
        quitApp: {
            enabled: () => Boolean(actions.quitApp),
            allowRepeat: false,
            run: () => actions.quitApp?.(),
        },
        commandPalette: {
            enabled: () => Boolean(actions.commandPalette),
            allowRepeat: false,
            run: () => actions.commandPalette?.(),
        },
        toggleSplitCompare: {
            enabled: () => Boolean(actions.toggleSplitCompare) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.toggleSplitCompare?.(),
        },
        syncSubtitlesByEar: {
            enabled: () => Boolean(actions.syncSubtitlesByEar) && isFileLoaded(),
            allowRepeat: false,
            run: () => actions.syncSubtitlesByEar?.(),
        },
        nextScene: {
            enabled: () => Boolean(actions.nextScene) && isFileLoaded(),
            run: () => actions.nextScene?.(),
        },
        previousScene: {
            enabled: () => Boolean(actions.previousScene) && isFileLoaded(),
            run: () => actions.previousScene?.(),
        },
        windowSizeUp: {
            enabled: () => Boolean(actions.windowSizeUp),
            run: () => actions.windowSizeUp?.(),
        },
        windowSizeDown: {
            enabled: () => Boolean(actions.windowSizeDown),
            run: () => actions.windowSizeDown?.(),
        },
        ultraSlomoHold: {
            enabled: () => Boolean(actions.startUltraSlomo) && isFileLoaded(),
            // Hold semantics: start once on the initial keydown; auto-repeat
            // keydowns are ignored, and release is handled by onKeyup below.
            allowRepeat: false,
            run: () => actions.startUltraSlomo?.(),
        },
    };

    const onKeydown = async (event: KeyboardEvent) => {
        // Escape is reserved: peel off one UI layer per press — the help
        // overlay, then whatever panel or dialog is on top — and only exit
        // fullscreen once nothing else is open.
        if (event.code === "Escape") {
            if (actions.closeShortcutsHelp?.()) {
                event.preventDefault();
                return;
            }
            // Typing in a field? Escape leaves the field first — a second press
            // then closes the panel that contains it.
            const escapeTarget = event.target as HTMLElement | null;
            if (isEditableTarget(escapeTarget)) {
                escapeTarget?.blur();
                event.preventDefault();
                return;
            }
            if (actions.closeTopOverlay?.()) {
                event.preventDefault();
                return;
            }
            if (!player.state.window.isFullscreen) {
                return;
            }
            event.preventDefault();
            await actions.toggleFullscreen();
            return;
        }

        if (isEditableTarget(event.target as HTMLElement | null)) {
            return;
        }

        // `?` / F1 are reserved for the shortcuts help overlay so it can always
        // be opened regardless of how the other bindings are customized.
        if (event.key === "?" || event.code === "F1") {
            if (actions.toggleShortcutsHelp) {
                event.preventDefault();
                actions.toggleShortcutsHelp();
            }
            return;
        }

        if (actions.isShortcutsHelpOpen?.()) {
            return;
        }

        const chord = chordFromEvent(event);
        if (!chord) return;

        const actionId = chordToAction.value.get(chord);
        if (!actionId) {
            // Fall through to user-defined shortcuts. Built-ins win, so a
            // custom binding can never shadow a documented key.
            if (actions.runCustomShortcut?.(chord)) {
                event.preventDefault();
            }
            return;
        }

        const handler = handlers[actionId];
        if (!handler || !handler.enabled()) return;
        if (event.repeat && handler.allowRepeat === false) return;

        event.preventDefault();
        await handler.run(event);
    };

    // Hold-to-slo-mo release: end slo-mo when the main key of the ultraSlomoHold
    // binding is released (modifiers are ignored — releasing the letter ends the
    // hold). stopUltraSlomo is a no-op unless slo-mo is currently active.
    const onKeyup = (event: KeyboardEvent) => {
        if (!actions.stopUltraSlomo) return;
        const chord = bindings.value.ultraSlomoHold;
        if (!chord || chord === UNBOUND_CHORD) return;
        const mainKey = chord.split("+").pop();
        if (event.code === mainKey) {
            actions.stopUltraSlomo();
        }
    };

    return {
        onDoubleClick,
        onKeydown,
        onKeyup,
        bindings,
    };
};
