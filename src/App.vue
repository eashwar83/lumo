<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
    currentMonitor,
    getCurrentWindow,
    PhysicalSize,
} from "@tauri-apps/api/window";
import type { MediaTrack } from "./types/media";
import type { PlaylistEntry } from "./types/playlist";
import { FAVORITES_PLAYLIST_ID } from "./types/playlist";
import ShortcutsHelpOverlay from "./components/ShortcutsHelpOverlay.vue";
import PlayerControls from "./components/PlayerControls.vue";
import PlayerHeader from "./components/PlayerHeader.vue";
import MainPanels from "./components/MainPanels.vue";
import SideActionsNav from "./components/SideActionsNav.vue";
import PlaybackOverlays from "./components/PlaybackOverlays.vue";
import PlaylistPeekButton from "./components/PlaylistPeekButton.vue";
import PlaylistDrawer from "./components/PlaylistDrawer.vue";
import PlaylistCreationDialog from "./components/PlaylistCreationDialog.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ContextMenu from "./components/ContextMenu.vue";
import OnlineSubtitleDialog from "./components/OnlineSubtitleDialog.vue";
import WindowResizeRegions from "./components/WindowResizeRegions.vue";
import { usePlaybackShortcuts } from "./composables/usePlaybackShortcuts";
import { useAutoCrop } from "./composables/useAutoCrop";
import { useVideoGeometry } from "./composables/useVideoGeometry";
import { useWindowSizeLock } from "./composables/useWindowSizeLock";
import { useAutoloadFolder } from "./composables/useAutoloadFolder";
import { usePlaybackFlow } from "./composables/usePlaybackFlow";
import { useAppUiPersistence } from "./composables/useAppUiPersistence";
import { useAppRuntimeBindings } from "./composables/useAppRuntimeBindings";
import { useAppPlaybackEvents } from "./composables/useAppPlaybackEvents";
import { useAppUiActions } from "./composables/useAppUiActions";
import { useAppBootstrap } from "./composables/useAppBootstrap";
import { useManualWindowStatePersistence } from "./composables/useManualWindowStatePersistence";
import { usePlaybackOverlays } from "./composables/usePlaybackOverlays";
import { usePlaylistDrawerUi } from "./composables/usePlaylistDrawerUi";
import { useUpdateNotePrompt } from "./composables/useUpdateNotePrompt";
import { useWindowDragRegion } from "./composables/useWindowDragRegion";
import { useMediaInfo } from "./composables/useMediaInfo";
import { usePlaylistEntriesWithProgress } from "./composables/usePlaylistEntriesWithProgress";
import { useAppStartupBindings } from "./composables/useAppStartupBindings";
import { usePlaybackSeekActions } from "./composables/usePlaybackSeekActions";
import { usePlaybackLoadingState } from "./composables/usePlaybackLoadingState";
import { usePlaybackNavigation } from "./composables/usePlaybackNavigation";
import { usePlaybackVolumePersistence } from "./composables/usePlaybackVolumePersistence";
import { usePlaylistCreationPrompt } from "./composables/usePlaylistCreationPrompt";
import { usePlaybackContextMenu } from "./composables/usePlaybackContextMenu";

const {
    isMacOS,
    player,
    tracks,
    speed,
    adjustments,
    enhancements,
    subtitleAppearance,
    history,
    playlistState,
    playlists,
    activePlaylistId,
    activePlaylist,
    playlist,
    loopMode,
    sortMode,
    isLoopOne,
    orderedPlaylist,
    isInfoOpen,
    isPlaylistOpen,
    activePanel,
    clearConfirmTarget,
    isPointerNearLeft,
    isPointerOverUi,
    isPipEnabled,
    schedulePointerRefresh,
    shouldKeepControlsVisible,
    hideAllMenus,
    toggleMenu,
    closeAllMenus,
    isFullscreenTransitioning,
    triggerFullscreenTransition,
    resetFullscreenTransition,
    onToggleFullscreen,
    playerHeaderRef,
    nowPlaying,
    ui,
    isPlaybackActive,
    navActivePanel,
    hasAudioTracks,
    hasSubTracks,
    setWindowControlsVisible,
    normalizeStoredPanel,
} = useAppBootstrap();

const clearNavSelectionDuringLoad = ref(false);
const playbackLoadingState = usePlaybackLoadingState();
const { isLoading, loadingUrl } = playbackLoadingState;
const playlistCreationPrompt = usePlaylistCreationPrompt();
const {
    persistCurrentManualWindow,
    restorePersistedManualWindow,
    schedulePersistManualWindow,
    persistBeforeUnload,
} = useManualWindowStatePersistence({
    isLoading,
    isPlaybackActive,
    isFileLoaded: () => player.state.media.isFileLoaded,
});

const playbackFlow = usePlaybackFlow({
    isMacOS,
    player,
    tracks,
    history,
    playlistState,
    nowPlaying,
    hideAllMenus,
    isInfoOpen,
    currentSpeed: speed.currentSpeed,
    loadingState: playbackLoadingState,
    onPlaybackIntent: async () => {
        await persistCurrentManualWindow();
        clearNavSelectionDuringLoad.value = true;
    },
    requestPlaylistCreation: playlistCreationPrompt.requestPlaylistCreation,
    onPlaylistCreated: () => {
        isPlaylistOpen.value = true;
    },
});

const {
    pendingResume,
    hideHistory,
    isLoadingForCurrentUrl,
    playPath,
    onLoadFile,
    onPlayHistory,
    onPlayNetwork,
    onUpdateUrl,
    updateLivePlaybackForDuration,
    resolveMediaTitle,
    onStopPlayback,
    requestOpenFilePicker,
    openSelectedPaths,
    playbackTitleMode,
    compactModeEnabled,
    wallpaperModeEnabled,
    subtitlesDisabled,
} = playbackFlow;

const onStopPlaybackWithWindowRestore = async () => {
    await onStopPlayback();
    await restorePersistedManualWindow();
};

const playbackNavigation = usePlaybackNavigation({
    player,
    playlistState,
    playPath,
});

const isWindowsPlatform =
    typeof navigator !== "undefined" && /\bwindows\b/i.test(navigator.userAgent);
const isLinuxPlatform =
    typeof navigator !== "undefined" && /\blinux\b/i.test(navigator.userAgent);
const playerHeaderCompactModeEnabled = computed(
    () => compactModeEnabled.value || (isWindowsPlatform && isPipEnabled.value),
);
const shouldKeepPlaybackBackgroundOpaque = computed(
    () =>
        (isMacOS && isPipEnabled.value) ||
        (isWindowsPlatform && wallpaperModeEnabled.value),
);
const shouldUseTransparentVideoMode = computed(
    () =>
        player.state.media.isFileLoaded &&
        !shouldKeepPlaybackBackgroundOpaque.value,
);
const sideNavActivePanel = computed(() =>
    isLoading.value && clearNavSelectionDuringLoad.value
        ? null
        : navActivePanel.value,
);
const {
    isLoadingOverlayVisible,
    loadingDownloadSpeedBps,
    seekOverlayLeftText,
    seekOverlayRightText,
    seekOverlayLeftTimelineText,
    volumeOverlayText,
    messageOverlayText,
    seekOverlayLeftPulseToken,
    seekOverlayRightPulseToken,
    showSeekOverlay,
    showVolumeOverlay,
    showMessageOverlay,
} = usePlaybackOverlays({
    player,
    isLoading,
});

const playbackVolume = usePlaybackVolumePersistence(player);

const onSetVolume = async (volume: number) => {
    await playbackVolume.setVolume(volume);
    showVolumeOverlay(player.state.playback.volume);
};

const onToggleMuted = async () => {
    await playbackVolume.toggleMuted();
    showVolumeOverlay(player.state.playback.volume);
};

const {
    isClearConfirmOpen,
    clearConfirmTitle,
    clearConfirmMessage,
    toggleInfo,
    togglePlaylist,
    closePlaylist,
    onNavAction,
    requestAddPlaylistItem,
    onClearHistory,
    onRemoveHistory,
    onTogglePinHistory,
    onClearPlaylist,
    closeClearConfirm,
    onConfirmClear,
    onRemovePlaylistItem,
    onPlayPlaylist,
    onEnterPlaylist,
    onBackToPlaylists,
    onRenamePlaylist,
    onDeletePlaylist,
    onMovePlaylist,
    onPrevTrack,
    onNextTrack,
    toggleLoopOne,
    onTogglePlaylistLoop,
} = useAppUiActions({
    isMacOS,
    player,
    playlistState,
    history,
    historyEntries: history.history,
    activePanel,
    hideHistory,
    isInfoOpen,
    isPlaylistOpen,
    clearConfirmTarget,
    playlist,
    hideAllMenus,
    schedulePointerRefresh,
    onStopPlayback: onStopPlaybackWithWindowRestore,
    playPath,
    playPreviousTrack: playbackNavigation.playPreviousTrack,
    playNextTrack: playbackNavigation.playNextTrack,
});

const onSideNavNavigate = async (
    panel: "home" | "history" | "favorites" | "network" | "settings",
) => {
    clearNavSelectionDuringLoad.value = false;
    await onNavAction(panel);
};

const openSettingsFromPlaybackContextMenu = async () => {
    clearNavSelectionDuringLoad.value = false;
    await onNavAction("settings");
};

const { onSeek, onSeekRelative } = usePlaybackSeekActions({
    player,
    isLoading,
    loadingUrl,
});

const isShortcutsHelpOpen = ref(false);
const isAlwaysOnTop = ref(false);
const areSubtitlesVisible = ref(true);

const runMpvShortcutCommand = async (args: Array<string | number>) => {
    try {
        await invoke("mpv_run_command", { args });
    } catch (error) {
        console.warn("[shortcuts] mpv command failed", { args, error });
    }
};

const formatSignedSeconds = (value: number) =>
    `${value > 0 ? "+" : ""}${value.toFixed(1)} s`;

const describeTrack = (track: MediaTrack) => {
    const title = track.title?.trim();
    const lang = track.lang?.trim();
    if (title && lang) return `${title} (${lang})`;
    return title || lang || `Track ${track.id}`;
};

const stepPlaybackSpeed = async (direction: 1 | -1) => {
    const rates = [...speed.playbackRates].sort((a, b) => a - b);
    const current = speed.currentSpeed.value;
    let index = rates.findIndex((rate) => Math.abs(rate - current) < 0.001);
    if (index === -1) {
        index = rates.reduce(
            (best, rate, rateIndex) =>
                Math.abs(rate - current) < Math.abs(rates[best] - current)
                    ? rateIndex
                    : best,
            0,
        );
    }
    const next =
        rates[Math.min(rates.length - 1, Math.max(0, index + direction))];
    if (Math.abs(next - current) >= 0.001) {
        await speed.setSpeed(next);
    }
    showMessageOverlay(`Speed ${next}×`);
};

const resetPlaybackSpeed = async () => {
    await speed.setSpeed(1.0);
    showMessageOverlay("Speed 1×");
};

const frameStep = async (direction: 1 | -1) => {
    // frame-step pauses playback; suppress the play/pause status icon so
    // stepping doesn't flash the pause overlay each time.
    nowPlaying.suppressStatusOverlay();
    await runMpvShortcutCommand([
        direction === 1 ? "frame-step" : "frame-back-step",
    ]);
};

const cycleSubtitleTrack = async (direction: 1 | -1) => {
    const available = tracks.subTracks.value;
    if (available.length <= 1) {
        showMessageOverlay("No subtitle tracks");
        return;
    }
    const currentIndex = available.findIndex((track) => track.selected);
    const nextIndex =
        ((currentIndex === -1 ? 0 : currentIndex) +
            direction +
            available.length) %
        available.length;
    const next = available[nextIndex];
    await tracks.selectSubTrack({ target: "primary", track: next });
    showMessageOverlay(`Subtitles: ${describeTrack(next)}`);
};

const toggleSubtitleVisibility = async () => {
    areSubtitlesVisible.value = !areSubtitlesVisible.value;
    const flag = areSubtitlesVisible.value ? "yes" : "no";
    await runMpvShortcutCommand(["set", "sub-visibility", flag]);
    await runMpvShortcutCommand(["set", "secondary-sub-visibility", flag]);
    showMessageOverlay(
        areSubtitlesVisible.value ? "Subtitles shown" : "Subtitles hidden",
    );
};

const cycleAudioTrack = async () => {
    const available = tracks.audioTracks.value;
    if (available.length === 0) {
        showMessageOverlay("No audio tracks");
        return;
    }
    const currentIndex = available.findIndex((track) => track.selected);
    const next =
        available[((currentIndex === -1 ? 0 : currentIndex) + 1) % available.length];
    await tracks.selectAudio(next);
    showMessageOverlay(`Audio: ${describeTrack(next)}`);
};

const adjustSubtitleDelay = async (deltaSeconds: number) => {
    const target = tracks.activeSubTarget.value;
    const current =
        target === "secondary"
            ? adjustments.secondarySubDelay.value
            : adjustments.subDelay.value;
    await adjustments.setSubDelayForTarget({
        target,
        value: Math.round((current + deltaSeconds) * 10) / 10,
    });
    const applied =
        target === "secondary"
            ? adjustments.secondarySubDelay.value
            : adjustments.subDelay.value;
    showMessageOverlay(`Subtitle delay ${formatSignedSeconds(applied)}`);
};

const adjustAudioDelayBy = async (deltaSeconds: number) => {
    await adjustments.setAudioDelay(
        Math.round((adjustments.audioDelay.value + deltaSeconds) * 10) / 10,
    );
    showMessageOverlay(
        `Audio delay ${formatSignedSeconds(adjustments.audioDelay.value)}`,
    );
};

const takeScreenshotShortcut = async (includeSubtitles: boolean) => {
    try {
        const result = await invoke<{ path: string; fileName: string }>(
            "take_screenshot",
            { includeSubtitles },
        );
        showMessageOverlay(`Screenshot saved · ${result.fileName}`, 2400);
    } catch (error) {
        console.error("[shortcuts] screenshot failed", error);
        showMessageOverlay(`Screenshot failed: ${error}`, 3200);
    }
};

const toggleAlwaysOnTop = async () => {
    try {
        const next = !isAlwaysOnTop.value;
        await getCurrentWindow().setAlwaysOnTop(next);
        isAlwaysOnTop.value = next;
        showMessageOverlay(next ? "Always on top: on" : "Always on top: off");
    } catch (error) {
        console.warn("[shortcuts] always-on-top failed", error);
    }
};

const toggleLoopWithFeedback = async () => {
    await toggleLoopOne();
    showMessageOverlay(isLoopOne.value ? "Loop file: on" : "Loop file: off");
};

const showProgressOverlay = () => {
    showMessageOverlay(
        `${player.formatTime(player.state.playback.currentTime)} / ${player.formatTime(player.state.playback.duration)}`,
    );
};

const favorites = playlistState.favorites;
const isCurrentFavorite = computed(() =>
    playlistState.isFavorite(player.state.media.url),
);

const onToggleFavorite = async () => {
    const url = player.state.media.url;
    if (!url) return;
    if (playlistState.isFavorite(url)) {
        playlistState.removeFromFavorites(url);
        showMessageOverlay("Removed from Favourites");
        return;
    }
    // Capture a poster frame for the Favourites grid if we don't have one yet.
    let icon = nowPlaying.nowPlayingArtworkPath.value;
    if (!icon) {
        await nowPlaying.captureNowPlayingArtwork();
        icon = nowPlaying.nowPlayingArtworkPath.value;
    }
    playlistState.addToFavorites({
        path: url,
        title: player.state.media.title?.trim() || undefined,
        iconUrl: icon || undefined,
    });
    showMessageOverlay("Added to Favourites");
};

const onPlayFavorite = async (entry: PlaylistEntry) => {
    clearNavSelectionDuringLoad.value = false;
    await playPath(entry.path, entry.title?.trim() || undefined);
};

const onRemoveFavorite = (entry: PlaylistEntry) => {
    playlistState.removeFromFavorites(entry.path);
};

const onClearFavorites = () => {
    playlistState.clearFavorites();
};

// Remembers which playlist/folder was showing before jumping to Favourites,
// so the second heart press returns there instead of the playlist list.
let playlistBeforeFavorites: string | null = null;

const onToggleFavoritesView = () => {
    if (activePlaylistId.value === FAVORITES_PLAYLIST_ID) {
        const previous = playlistBeforeFavorites;
        playlistBeforeFavorites = null;
        if (previous && previous !== FAVORITES_PLAYLIST_ID) {
            playlistState.enterPlaylist(previous);
        }
        // If there was nothing to restore (or it's gone), fall back to the list.
        if (activePlaylistId.value === FAVORITES_PLAYLIST_ID) {
            playlistState.backToPlaylistList();
        }
    } else {
        playlistBeforeFavorites = activePlaylistId.value;
        playlistState.openFavoritesView();
        showMessageOverlay(`Favourites · ${favorites.value.length}`);
    }
};

const { onKeydown, onDoubleClick, bindings: shortcutBindings } = usePlaybackShortcuts(
    {
        state: player.state,
        togglePlayPause: player.togglePlayPause,
        seekRelative: onSeekRelative,
        setVolume: playbackVolume.setVolume,
    },
    {
        toggleFullscreen: onToggleFullscreen,
        toggleInfo,
        seekOverlay: showSeekOverlay,
        volumeOverlay: showVolumeOverlay,
        toggleMuted: onToggleMuted,
        seekAbsolute: async (positionSeconds: number) => {
            await onSeek(positionSeconds);
            showMessageOverlay("Jumped to start");
        },
        frameStep,
        stepPlaybackSpeed,
        resetPlaybackSpeed,
        cycleSubtitleTrack,
        toggleSubtitleVisibility,
        cycleAudioTrack,
        adjustSubtitleDelay,
        adjustAudioDelay: adjustAudioDelayBy,
        takeScreenshot: takeScreenshotShortcut,
        previousTrack: onPrevTrack,
        nextTrack: onNextTrack,
        toggleLoop: toggleLoopWithFeedback,
        autoCropNow: () => autoCrop.detectNow(),
        clearCrop: () => autoCrop.clear(),
        togglePlaylist,
        toggleAlwaysOnTop,
        toggleFavorite: onToggleFavorite,
        cycleAspectRatio: () => onCycleAspectRatio(),
        fitWindowToVideo: () => onFitWindowToVideo(),
        windowSizeUp: () => stepWindowSize(1.1),
        windowSizeDown: () => stepWindowSize(0.9),
        showProgress: showProgressOverlay,
        toggleShortcutsHelp: () => {
            isShortcutsHelpOpen.value = !isShortcutsHelpOpen.value;
        },
        closeShortcutsHelp: () => {
            if (!isShortcutsHelpOpen.value) return false;
            isShortcutsHelpOpen.value = false;
            return true;
        },
        isShortcutsHelpOpen: () => isShortcutsHelpOpen.value,
    },
);

const currentMediaKey = () => player.state.media.url;
const geometry = useVideoGeometry();

// Locked window size (persists across videos). A genuine user drag updates the
// lock and drops the current file's per-file "fit" flag it just overrode.
const windowLock = useWindowSizeLock({
    onUserResize: () => geometry.setFitWindow(currentMediaKey(), false),
});

const autoCrop = useAutoCrop({
    isFileLoaded: () => player.state.media.isFileLoaded,
    onMessage: showMessageOverlay,
    mediaKey: currentMediaKey,
    getSavedCrop: geometry.getCrop,
    onCropChanged: geometry.setCrop,
});

const readMpvNumber = async (name: string): Promise<number | null> => {
    try {
        const raw = await invoke<string | null>("mpv_get_property_string", {
            name,
        });
        if (raw == null) return null;
        const parsed = Number.parseFloat(raw);
        return Number.isFinite(parsed) ? parsed : null;
    } catch {
        return null;
    }
};

// Reshape the window to the video's current display aspect (mpv `dwidth`/
// `dheight` already account for crop + aspect override), removing the letterbox/
// pillarbox bars that appear when the window shape no longer matches.
const fitWindowToVideoDisplay = async () => {
    const dw = await readMpvNumber("dwidth");
    const dh = await readMpvNumber("dheight");
    if (!dw || !dh) return;
    try {
        const win = getCurrentWindow();
        if ((await win.isFullscreen()) || (await win.isMaximized())) return;
        const size = await win.innerSize();
        const scale = await win.scaleFactor().catch(() => 1);
        // Base the fit height on the locked size (a settled value) when a lock is
        // active — reading innerSize right after applying the lock can return a
        // mid-transition height, causing a slight misalignment.
        const locked = windowLock.getLockedSize();
        const targetHeight = locked
            ? Math.round(locked.height * scale)
            : size.height;
        if (!targetHeight) return;
        let targetWidth = Math.round(targetHeight * (dw / dh));
        const monitor = await currentMonitor();
        if (monitor) targetWidth = Math.min(targetWidth, monitor.size.width);
        if (targetWidth > 0 && (targetWidth !== size.width || targetHeight !== size.height)) {
            // Programmatic: don't let this count as a user resize of the lock.
            await windowLock.runProgrammatic(async () => {
                await win.setSize(new PhysicalSize(targetWidth, targetHeight));
            });
        }
    } catch (error) {
        console.warn("[window] fit-to-video failed", error);
    }
};

// Explicit "Fit window to video" (default G): reshape the window and remember
// the fit for this file so it's re-applied on reopen.
const onFitWindowToVideo = async () => {
    await fitWindowToVideoDisplay();
    geometry.setFitWindow(currentMediaKey(), true);
    showMessageOverlay("Fit to window");
};

const onCycleAspectRatio = async () => {
    const label = await geometry.cycleAspect();
    await fitWindowToVideoDisplay();
    showMessageOverlay(`Aspect: ${label}`);
};

// Auto Enhance: sample the current frame in Rust, then apply the suggested
// brightness/contrast/saturation (mpv adjustments) and temperature/tint
// (colour-grade shader). Values land in the sliders so they can be tweaked.
type EnhanceSuggestion = {
    brightness: number;
    contrast: number;
    saturation: number;
    temperature: number;
    tint: number;
};
const onAutoEnhance = async () => {
    if (!player.state.media.isFileLoaded) return;
    showMessageOverlay("Auto Enhance…");
    try {
        const s = await invoke<EnhanceSuggestion>("analyze_frame_for_enhance");
        await adjustments.setBrightness(Math.round(s.brightness));
        await adjustments.setContrast(Math.round(s.contrast));
        await adjustments.setSaturation(Math.round(s.saturation));
        enhancements.setColorGrade("temperature", Math.round(s.temperature));
        enhancements.setColorGrade("tint", Math.round(s.tint));
        showMessageOverlay("Auto Enhance applied");
    } catch (error) {
        console.warn("[auto-enhance] failed", error);
        showMessageOverlay("Auto Enhance failed");
    }
};

// Grow/shrink the window by a step, keeping its aspect, clamped to a sane
// minimum and the current monitor. No-op while fullscreen or maximized. Counts
// as a user size change: updates the lock and drops any per-file fit flag.
const stepWindowSize = async (factor: number) => {
    try {
        const win = getCurrentWindow();
        if ((await win.isFullscreen()) || (await win.isMaximized())) return;
        const size = await win.innerSize();
        const scale = await win.scaleFactor().catch(() => 1);
        let width = Math.round(size.width * factor);
        let height = Math.round(size.height * factor);
        width = Math.max(320, width);
        height = Math.max(180, height);
        const monitor = await currentMonitor();
        if (monitor) {
            width = Math.min(width, monitor.size.width);
            height = Math.min(height, monitor.size.height);
        }
        await windowLock.runProgrammatic(async () => {
            await win.setSize(new PhysicalSize(width, height));
        });
        windowLock.setLocked(width / scale, height / scale);
        geometry.setFitWindow(currentMediaKey(), false);
    } catch (error) {
        console.warn("[window] resize step failed", error);
    }
};

// Called when a newly-sized video loads (resize_window event). Take over window
// sizing so the locked size (or a per-file fit) is honoured instead of resizing
// to the video's native pixels. Returns true when handled (skips the native
// auto-resize). Dimensions are already known here, so no dwidth/dheight wait.
const onVideoAutoResize = async (): Promise<boolean> => {
    if (geometry.isFitWindow(currentMediaKey())) {
        await windowLock.applyLocked();
        await fitWindowToVideoDisplay();
        return true;
    }
    return windowLock.applyLocked();
};

// Apply window sizing on every file load — the reliable path. The resize_window
// event only fires when the video's dimensions CHANGE, so reopening the same
// file (or another with identical dimensions) wouldn't otherwise re-apply the
// locked size or a remembered fit. dwidth/dheight can lag a moment after load /
// aspect apply, so retry briefly before fitting.
const applyWindowSizingForMedia = async () => {
    await windowLock.applyLocked();
    if (!geometry.isFitWindow(currentMediaKey())) return;
    for (let attempt = 0; attempt < 6; attempt += 1) {
        const dw = await readMpvNumber("dwidth");
        const dh = await readMpvNumber("dheight");
        if (dw && dh) {
            await fitWindowToVideoDisplay();
            return;
        }
        await new Promise((resolve) => window.setTimeout(resolve, 80));
    }
};

enhancements.setMessageHandler(showMessageOverlay);

const autoloadFolder = useAutoloadFolder({ playlist: playlistState });

watch(
    subtitlesDisabled,
    (disabled) => {
        void tracks.setSubtitlesDisabled(disabled);
    },
    { immediate: true },
);

const {
    playlistScrollState,
    playlistDrawerWidthRatio,
    onPlaylistScrollPositionChange,
    onPlaylistDrawerWidthRatioChange,
} = usePlaylistDrawerUi();
const {
    isUpdateNotePromptOpen,
    updateNotePromptTitle,
    updateNotePromptBlocks,
    showUpdateNotePrompt,
    closeUpdateNotePrompt,
    onConfirmUpdateNotePrompt,
} = useUpdateNotePrompt({
    activePanel,
    hideHistory,
    clearNavSelectionDuringLoad,
    settingsPanelId: "settings",
});
const {
    onAppMouseDownCapture,
    onAppTouchStartCapture,
    onDragRegionMouseDown,
    onDragRegionTouchStart,
} = useWindowDragRegion();
const { mediaInfo, statusBadges } = useMediaInfo(player);
const currentOrLastPlaybackUrl = computed(
    () => player.state.media.url || player.state.media.lastLoadedUrl,
);
const playlistEntriesWithProgress = usePlaylistEntriesWithProgress(
    orderedPlaylist,
    history.history,
);
const playbackContextMenu = usePlaybackContextMenu({
    isFileLoaded: () => player.state.media.isFileLoaded,
    getCurrentPath: () => player.state.media.url,
    getCurrentTitle: () => player.state.media.title,
    addToFavorites: playlistState.addToFavorites,
    searchOnlineSubtitles: tracks.searchOnlineSubtitleTracks,
    openSubtitleAdvancedSettings: () => {
        tracks.showSubMenu.value = true;
        tracks.showSubtitleAdvancedSettings.value = true;
    },
    openSettings: openSettingsFromPlaybackContextMenu,
    hideAllMenus,
});

const { hasLoadedPanel, loadActivePanel } = useAppUiPersistence({
    activePanel,
    playlists,
    activePlaylistId,
    playlistScrollState,
    playlistDrawerWidthRatio,
    loopMode,
    sortMode,
    playlistState,
    schedulePointerRefresh,
    normalizeStoredPanel,
});

const {
    onFileLoaded: onFileLoadedBase,
    onPlaybackRestart,
    onProgress,
    onEndFile,
} =
    useAppPlaybackEvents({
        player,
        tracks,
        playlistState,
        history,
        nowPlaying,
        pendingResume,
        isLoopOne,
        isLoading,
        loadingUrl,
        playNextAfterEnd: playbackNavigation.playNextAfterEnd,
    });

const onFileLoaded = async () => {
    await onFileLoadedBase();
    if (subtitlesDisabled.value) {
        await tracks.setSubtitlesDisabled(true);
    }
    await adjustments.applyColorAdjustmentsForMedia(player.state.media.url);
    await subtitleAppearance.applySubtitleAppearanceOptions();
    void enhancements.onFileLoaded();
    await geometry.applyAspectForMedia(currentMediaKey());
    void applyWindowSizingForMedia();
    void autoCrop.onFileLoaded();
    void autoloadFolder.onFileLoaded(player.state.media.url);
};

const onProgressWithLivePlaybackUpdate = (
    payload: Parameters<typeof onProgress>[0],
) => {
    updateLivePlaybackForDuration(payload.duration);
    onProgress(payload);
};

useAppRuntimeBindings({
    player,
    tracks,
    ui,
    onFullscreenTransition: triggerFullscreenTransition,
    onFullscreenTransitionEnd: resetFullscreenTransition,
    onCloseAllMenus: closeAllMenus,
    onKeydown,
    onDoubleClick,
    setWindowControlsVisible,
    onFileLoaded,
    onPlaybackRestart,
    onProgress: onProgressWithLivePlaybackUpdate,
    onEndFile,
    resolveMediaTitle,
    interceptAutoResize: () => onVideoAutoResize(),
    nowPlaying,
    isInfoOpen,
    isPlaylistOpen,
    hideHistory,
    playerHeaderRef,
    closePlaylist,
    shouldKeepControlsVisible,
    schedulePointerRefresh,
});

useAppStartupBindings({
    activePanel,
    hideHistory,
    clearNavSelectionDuringLoad,
    settingsPanelId: "settings",
    isFileLoaded: () => player.state.media.isFileLoaded,
    openSelectedPaths,
    loadActivePanel,
    restorePersistedManualWindow,
    schedulePersistManualWindow,
    persistBeforeUnload,
    showUpdateNotePrompt,
});
</script>

<template>
    <main
        class="soia-container"
        :class="{
            'video-mode': shouldUseTransparentVideoMode,
            'cursor-hidden':
                player.state.media.isFileLoaded &&
                !ui.showControls.value &&
                !isPointerOverUi,
        }"
        @mousedown.capture="onAppMouseDownCapture"
        @touchstart.capture="onAppTouchStartCapture"
        @contextmenu="playbackContextMenu.onContextMenu"
    >
        <SideActionsNav
            v-show="!player.state.media.isFileLoaded || isPointerNearLeft"
            :is-playback-active="isPlaybackActive"
            :active-panel="activePanel"
            :nav-active-panel="sideNavActivePanel"
            @navigate="onSideNavNavigate"
        />

        <PlayerHeader
            ref="playerHeaderRef"
            v-show="ui.showControls.value && !isFullscreenTransitioning"
            :is-mac-os="isMacOS"
            :url="player.state.media.url"
            :media-title="player.state.media.title"
            :is-url-modified="player.isUrlModified.value"
            :is-file-loaded="player.state.media.isFileLoaded"
            :is-info-open="isInfoOpen"
            :is-playlist-open="isPlaylistOpen"
            :is-favorite="isCurrentFavorite"
            :is-loading="isLoadingForCurrentUrl"
            :playback-title-mode="playbackTitleMode"
            :compact-mode-enabled="playerHeaderCompactModeEnabled"
            :is-fullscreen="player.state.window.isFullscreen"
            :info="mediaInfo"
            :current-time="player.state.playback.currentTime"
            :duration="player.state.playback.duration"
            :is-playing="player.state.playback.isPlaying"
            :video-bitrate="player.state.playback.videoBitrate"
            :hwdec-current="player.state.playback.hwdecCurrent"
            :playback-speed="speed.currentSpeed.value"
            :video-tracks="tracks.videoTracks.value"
            :audio-tracks="tracks.audioTracks.value"
            :sub-tracks="tracks.subTracks.value"
            @update:url="onUpdateUrl"
            @load-file="onLoadFile"
            @open-file-picker="requestOpenFilePicker"
            @toggle-info="toggleInfo"
            @toggle-playlist="togglePlaylist"
            @toggle-favorite="onToggleFavorite"
            @url-input-mousedown="onDragRegionMouseDown"
            @url-input-touchstart="onDragRegionTouchStart"
        />

        <PlaybackOverlays
            :is-loading="isLoadingOverlayVisible"
            :loading-speed-bps="loadingDownloadSpeedBps"
            :show-status-overlay="
                player.state.media.isFileLoaded &&
                nowPlaying.showStatusOverlay.value
            "
            :status-overlay-mode="nowPlaying.statusOverlayMode.value"
            :seek-overlay-left-text="seekOverlayLeftText"
            :seek-overlay-right-text="seekOverlayRightText"
            :seek-overlay-left-timeline-text="seekOverlayLeftTimelineText"
            :volume-overlay-text="volumeOverlayText"
            :message-overlay-text="messageOverlayText"
            :hide-seek-timeline="ui.showControls.value"
            :seek-overlay-left-pulse-token="seekOverlayLeftPulseToken"
            :seek-overlay-right-pulse-token="seekOverlayRightPulseToken"
        />

        <MainPanels
            v-show="!player.state.media.isFileLoaded"
            :is-file-loaded="player.state.media.isFileLoaded"
            :hover="ui.hoverFilePicker.value"
            :history="history.history.value"
            :history-ready="history.isReady.value"
            :hide-history="hideHistory"
            :favorites="favorites"
            :mode="activePanel"
            :current-url="currentOrLastPlaybackUrl"
            @update:hover="ui.hoverFilePicker.value = $event"
            @open-file-picker="requestOpenFilePicker"
            @play-history="onPlayHistory"
            @play-network="onPlayNetwork"
            @clear-history="onClearHistory"
            @remove-history="onRemoveHistory"
            @toggle-pin-history="onTogglePinHistory"
            @play-favorite="onPlayFavorite"
            @remove-favorite="onRemoveFavorite"
            @clear-favorites="onClearFavorites"
        />

        <PlaylistDrawer
            :open="isPlaylistOpen"
            :playlists="playlists"
            :active-playlist-id="activePlaylistId"
            :active-playlist-name="activePlaylist?.name ?? ''"
            :width-ratio="playlistDrawerWidthRatio"
            :scroll-state="playlistScrollState"
            :entries="playlistEntriesWithProgress"
            :is-ready="hasLoadedPanel"
            :current-url="player.state.media.url"
            :loop-mode="loopMode"
            :sort-mode="sortMode"
            :is-loop-one="isLoopOne"
            @close="closePlaylist"
            @add="requestAddPlaylistItem"
            @toggle-loop="onTogglePlaylistLoop"
            @toggle-sort="playlistState.cycleSortMode"
            @clear="onClearPlaylist"
            @remove="onRemovePlaylistItem"
            @play="onPlayPlaylist"
            @enter-playlist="onEnterPlaylist"
            @toggle-favorites-view="onToggleFavoritesView"
            @back="onBackToPlaylists"
            @rename-playlist="onRenamePlaylist"
            @delete-playlist="onDeletePlaylist"
            @move-playlist="onMovePlaylist"
            @width-ratio-change="onPlaylistDrawerWidthRatioChange"
            @scroll-position-change="onPlaylistScrollPositionChange"
        />

        <ContextMenu
            :open="playbackContextMenu.isOpen.value"
            :x="playbackContextMenu.position.value.x"
            :y="playbackContextMenu.position.value.y"
            :items="playbackContextMenu.items.value"
            @select="playbackContextMenu.onSelect"
            @close="playbackContextMenu.close"
        />

        <OnlineSubtitleDialog
            :open="tracks.isOnlineSubtitleDialogOpen.value"
            :provider-tabs="tracks.onlineSubtitleProviderTabs.value"
            :active-provider-id="tracks.activeOnlineSubtitleProviderId.value"
            :results="tracks.onlineSubtitleResults.value"
            :loading="tracks.isSearchingOnlineSubtitles.value"
            :applying="tracks.isLoadingOnlineSubtitle.value"
            :error-message="tracks.onlineSubtitleErrorMessage.value"
            @close="tracks.closeOnlineSubtitleDialog"
            @provider-change="tracks.setOnlineSubtitleProvider"
            @select="tracks.addSelectedOnlineSubtitleTrack"
        />

        <PlayerControls
            :is-playing="player.state.playback.isPlaying"
            :current-time="player.state.playback.currentTime"
            :duration="player.state.playback.duration"
            :is-live-playback="player.state.media.isLivePlayback"
            :volume="player.state.playback.volume"
            :progress-percent="player.progressPercent.value"
            :buffered-percent="player.bufferedPercent.value"
            :format-time="player.formatTime"
            :controls-visible="ui.showControls.value"
            :is-hidden="
                !player.state.media.isFileLoaded || !ui.showControls.value
            "
            :status-badges="statusBadges"
            :current-speed="speed.currentSpeed.value"
            :playback-rates="speed.playbackRates"
            :show-speed-menu="speed.showSpeedMenu.value"
            :show-settings-menu="adjustments.showSettingsMenu.value"
            :audio-delay="adjustments.audioDelay.value"
            :sub-delay="adjustments.subDelay.value"
            :secondary-sub-delay="adjustments.secondarySubDelay.value"
            :brightness="adjustments.brightness.value"
            :contrast="adjustments.contrast.value"
            :saturation="adjustments.saturation.value"
            :gamma="adjustments.gamma.value"
            :hue="adjustments.hue.value"
            :global-color-adjustments-enabled="
                adjustments.globalColorAdjustmentsEnabled.value
            "
            :enhancements="enhancements"
            :is-loop-one="isLoopOne"
            :audio-tracks="tracks.audioTracks.value"
            :show-audio-menu="tracks.showAudioMenu.value"
            :sub-tracks="tracks.subTracks.value"
            :dual-sub-enabled="tracks.dualSubEnabled.value"
            :secondary-sub-id="tracks.secondarySubId.value"
            :active-sub-target="tracks.activeSubTarget.value"
            :primary-sub-font-family="subtitleAppearance.primaryFontFamily.value"
            :secondary-sub-font-family="subtitleAppearance.secondaryFontFamily.value"
            :primary-sub-font-size="subtitleAppearance.primaryFontSize.value"
            :secondary-sub-font-size="subtitleAppearance.secondaryFontSize.value"
            :primary-sub-font-color="subtitleAppearance.primaryFontColor.value"
            :secondary-sub-font-color="subtitleAppearance.secondaryFontColor.value"
            :primary-sub-pos="subtitleAppearance.primarySubPos.value"
            :secondary-sub-pos="subtitleAppearance.secondarySubPos.value"
            :show-sub-menu="tracks.showSubMenu.value"
            :show-subtitle-advanced-settings="tracks.showSubtitleAdvancedSettings.value"
            :has-audio-tracks="hasAudioTracks"
            :has-sub-tracks="hasSubTracks"
            :is-fullscreen="player.state.window.isFullscreen"
            @seek="onSeek"
            @prev-track="onPrevTrack"
            @toggle-play-pause="player.togglePlayPause"
            @stop-playback="onStopPlaybackWithWindowRestore"
            @next-track="onNextTrack"
            @toggle-menu="toggleMenu"
            @toggle-loop-one="toggleLoopOne"
            @set-speed="speed.setSpeed"
            @set-volume="onSetVolume"
            @toggle-muted="onToggleMuted"
            @set-audio-delay="adjustments.setAudioDelay"
            @set-sub-delay-for-target="adjustments.setSubDelayForTarget"
            @set-sub-font-family="subtitleAppearance.setSubtitleFontFamily"
            @set-sub-font-size="subtitleAppearance.setSubtitleFontSize"
            @set-sub-font-color="subtitleAppearance.setSubtitleFontColor"
            @set-sub-position="subtitleAppearance.setSubtitlePosition"
            @reset-sub-appearance="subtitleAppearance.resetSubtitleAppearance"
            @set-brightness="adjustments.setBrightness"
            @set-contrast="adjustments.setContrast"
            @set-saturation="adjustments.setSaturation"
            @set-gamma="adjustments.setGamma"
            @set-hue="adjustments.setHue"
            @set-global-color-adjustments-enabled="
                adjustments.setGlobalColorAdjustmentsEnabled
            "
            @auto-enhance="onAutoEnhance"
            @select-audio="tracks.selectAudio"
            @select-sub-track="tracks.selectSubTrack"
            @set-active-sub-target="tracks.setActiveSubTarget"
            @toggle-dual-sub="tracks.setDualSubEnabled"
            @add-external-audio="tracks.addExternalAudioTrack"
            @add-external-sub="tracks.addExternalSubtitleTrack"
            @find-online-sub="player.state.media.url.trim() && tracks.searchOnlineSubtitleTracks(player.state.media.url, player.state.media.title || undefined)"
            @toggle-fullscreen="onToggleFullscreen"
            @update:show-subtitle-advanced-settings="tracks.showSubtitleAdvancedSettings.value = $event"
        />

        <PlaylistPeekButton
            v-show="!isPlaylistOpen"
            :disabled="
                tracks.showAudioMenu.value ||
                tracks.showSubMenu.value ||
                speed.showSpeedMenu.value ||
                adjustments.showSettingsMenu.value
            "
            @toggle="togglePlaylist"
        />

        <ConfirmDialog
            :open="isClearConfirmOpen"
            :title="clearConfirmTitle"
            :message="clearConfirmMessage"
            confirm-text="Clear"
            @cancel="closeClearConfirm"
            @confirm="onConfirmClear"
        />

        <ConfirmDialog
            :open="isUpdateNotePromptOpen"
            :title="updateNotePromptTitle"
            message=""
            confirm-text="Update"
            cancel-text="Cancel"
            confirm-variant="primary"
            size="wide"
            @cancel="closeUpdateNotePrompt"
            @confirm="onConfirmUpdateNotePrompt"
        >
            <div class="update-note">
                <template v-if="updateNotePromptBlocks.length">
                    <template
                        v-for="(block, blockIndex) in updateNotePromptBlocks"
                        :key="blockIndex"
                    >
                        <div
                            v-if="block.type === 'heading'"
                            class="update-note__heading"
                            :class="{
                                'update-note__heading--section': block.level <= 3,
                            }"
                        >
                            {{ block.text }}
                        </div>
                        <p
                            v-else-if="block.type === 'paragraph'"
                            class="update-note__paragraph"
                        >
                            {{ block.text }}
                        </p>
                        <ol
                            v-else-if="block.ordered"
                            class="update-note__list update-note__list--ordered"
                        >
                            <li
                                v-for="(item, itemIndex) in block.items"
                                :key="itemIndex"
                            >
                                {{ item }}
                            </li>
                        </ol>
                        <ul v-else class="update-note__list">
                            <li
                                v-for="(item, itemIndex) in block.items"
                                :key="itemIndex"
                            >
                                {{ item }}
                            </li>
                        </ul>
                    </template>
                </template>
                <p v-else class="update-note__paragraph">
                    A new version is ready to install.
                </p>
            </div>
        </ConfirmDialog>

        <PlaylistCreationDialog
            :open="playlistCreationPrompt.isOpen.value"
            :message="playlistCreationPrompt.message.value"
            v-model:name-draft="playlistCreationPrompt.nameDraft.value"
            @cancel="playlistCreationPrompt.cancelPlaylistCreation"
            @confirm="playlistCreationPrompt.confirmPlaylistCreation"
        />

        <ShortcutsHelpOverlay
            :open="isShortcutsHelpOpen"
            :bindings="shortcutBindings"
            @close="isShortcutsHelpOpen = false"
        />

        <WindowResizeRegions
            v-if="isLinuxPlatform && !player.state.window.isFullscreen"
        />
    </main>
</template>

<style src="./styles/app-theme.css"></style>
<style scoped src="./styles/app-shell.css"></style>
<style src="./styles/player.css"></style>
<style scoped>
.update-note {
    display: grid;
    gap: 10px;
    max-height: min(46vh, 360px);
    overflow: auto;
    padding-right: 2px;
}

.update-note__heading {
    margin-top: 2px;
    color: rgba(255, 255, 255, 0.74);
    font-size: 12px;
    font-weight: 650;
}

.update-note__heading--section {
    color: rgba(255, 255, 255, 0.92);
    font-size: 13px;
}

.update-note__paragraph {
    margin: 0;
}

.update-note__list {
    margin: 0;
    padding-left: 18px;
}

.update-note__list li + li {
    margin-top: 6px;
}

:global(:root[data-theme="light"]) .update-note__heading {
    color: rgba(33, 45, 60, 0.72);
}

:global(:root[data-theme="light"]) .update-note__heading--section {
    color: rgba(33, 45, 60, 0.92);
}

:global(:root[data-theme="graphite"]) .update-note__heading {
    color: rgba(220, 226, 234, 0.78);
}

:global(:root[data-theme="graphite"]) .update-note__heading--section {
    color: rgba(237, 241, 246, 0.95);
}
</style>
