<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { saveJsonFile, openJsonFile } from "./composables/useBackupIo";
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
import YtPlayerDrawer from "./components/youtube/YtPlayerDrawer.vue";
import YtSponsorToast from "./components/youtube/YtSponsorToast.vue";
import YtDownloadDialog from "./components/youtube/YtDownloadDialog.vue";
import {
    useYouTubeDownloads,
    type DownloadOptions,
} from "./composables/useYouTubeDownloads";
import PlaylistPeekButton from "./components/PlaylistPeekButton.vue";
import PlaylistDrawer from "./components/PlaylistDrawer.vue";
import PlaylistCreationDialog from "./components/PlaylistCreationDialog.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import ContextMenu from "./components/ContextMenu.vue";
import OnlineSubtitleDialog from "./components/OnlineSubtitleDialog.vue";
import MergeDialog from "./components/MergeDialog.vue";
import SplitDialog from "./components/SplitDialog.vue";
import SubtitleAiDialog from "./components/SubtitleAiDialog.vue";
import SubtitleTranslateDialog from "./components/SubtitleTranslateDialog.vue";
import SubtitleSyncDialog from "./components/SubtitleSyncDialog.vue";
import ClipDescribeDialog from "./components/ClipDescribeDialog.vue";
import WindowResizeRegions from "./components/WindowResizeRegions.vue";
import { usePlaybackShortcuts } from "./composables/usePlaybackShortcuts";
import { useUltraSlomo } from "./composables/useUltraSlomo";
import { useAutoCrop } from "./composables/useAutoCrop";
import { useVideoGeometry } from "./composables/useVideoGeometry";
import { useVideoTransform } from "./composables/useVideoTransform";
import { useWindowSizeLock } from "./composables/useWindowSizeLock";
import { useVideoPresets } from "./composables/useVideoPresets";
import ClipBar from "./components/ClipBar.vue";
import MenuBar from "./components/menu/MenuBar.vue";
import { useAppMenu, menuShortcut } from "./composables/useAppMenu";
import { useCommandRegistry } from "./composables/useCommandRegistry";
import { useCustomShortcuts } from "./composables/useCustomShortcuts";
import CommandPalette from "./components/CommandPalette.vue";
import SplitCompareOverlay from "./components/SplitCompareOverlay.vue";
import { useSplitCompare } from "./composables/useSplitCompare";
import { useEnhancementHistory } from "./composables/useEnhancementHistory";
import { useSubtitleSyncByEar } from "./composables/useSubtitleSyncByEar";
import { useSceneIndex } from "./composables/useSceneIndex";
import { useYouTubeWatch } from "./composables/useYouTubeWatch";
import { useYouTubeSettings } from "./composables/useYouTubeSettings";
import {
    createDebouncedUiStateSaver,
    loadUiState,
    saveUiState,
} from "./composables/useUiStateStore";
import CurvesPanel from "./components/CurvesPanel.vue";
import { serializeCurves } from "./utils/curves";
import { useAiConfig } from "./composables/useAiConfig";
import { AI_PROVIDERS } from "./constants/aiModels";
import EqualizerPanel from "./components/EqualizerPanel.vue";
import SkipPrompt from "./components/SkipPrompt.vue";
import { useAbRange } from "./composables/useAbRange";
import { useSkipMarkers } from "./composables/useSkipMarkers";
import { useAudioEnhancements } from "./composables/useAudioEnhancements";
import SettingsPanel from "./panels/SettingsPanel.vue";
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
const sideNavActivePanel = computed(() => {
    if (isSettingsOpen.value) return "settings";
    return isLoading.value && clearNavSelectionDuringLoad.value
        ? null
        : navActivePanel.value;
});
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
    panel:
        | "home"
        | "history"
        | "favorites"
        | "youtube"
        | "network"
        | "settings",
) => {
    clearNavSelectionDuringLoad.value = false;
    // Settings opens as a modal over live playback — it must not stop the video
    // or swap out the main panel like the other nav destinations do.
    if (panel === "settings") {
        openSettings();
        return;
    }
    isSettingsOpen.value = false;
    await onNavAction(panel);
};

const openSettingsFromPlaybackContextMenu = async () => {
    clearNavSelectionDuringLoad.value = false;
    openSettings();
};

const { onSeek, onSeekRelative } = usePlaybackSeekActions({
    player,
    isLoading,
    loadingUrl,
});

const isShortcutsHelpOpen = ref(false);
const isCurvesOpen = ref(false);
const isAudioPanelOpen = ref(false);
const isSettingsOpen = ref(false);
const audio = useAudioEnhancements();
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

const onToggleYoutubeFavorite = (payload: {
    url: string;
    title: string;
    thumbnailUrl?: string | null;
}) => {
    const wasFavorite = playlistState.isFavorite(payload.url);
    playlistState.toggleFavorite({
        path: payload.url,
        title: payload.title,
        iconUrl: payload.thumbnailUrl ?? undefined,
    });
    showMessageOverlay(
        wasFavorite ? "Removed from Favourites" : "Added to Favourites",
    );
};

const onPlayYoutube = async (payload: { url: string; title?: string }) => {
    clearNavSelectionDuringLoad.value = false;
    try {
        await playPath(payload.url, payload.title?.trim() || undefined);
    } catch (error) {
        // Unresolvable video (age/region-locked, removed…): stop the
        // spinner and say why instead of loading forever.
        isLoading.value = false;
        showMessageOverlay(
            String(error).replace(/^Error:\s*/, "").slice(0, 200),
            4200,
        );
    }
};

// --- YouTube per-video quality override ----------------------------------
const mainPanelsRef = ref<{ closeYoutubeBrowseView: () => boolean } | null>(
    null,
);
const ytQualityOverride = ref<number | null>(null);
const isYoutubePlayback = computed(() =>
    /(^|\.)((youtube\.com)|(youtu\.be))\//i.test(
        (player.state.media.url || "").replace(/^https?:\/\//i, ""),
    ),
);
const youtubeQualityLabel = computed(() => {
    if (!isYoutubePlayback.value || !player.state.media.isFileLoaded) {
        return null;
    }
    if (ytQualityOverride.value) return `${ytQualityOverride.value}p`;
    // No per-video override: show the configured default.
    return youtubeSettings.qualityMaxHeight
        ? `${youtubeSettings.qualityMaxHeight}p`
        : "Auto";
});
watch(
    () => player.state.media.url,
    () => {
        ytQualityOverride.value = null;
    },
);
// YouTube slow-starts anonymous streams (anti-bot ramp, occasionally a
// minute); without a status line users read the wait as a dead player.
let ytLoadingNoticeTimers: number[] = [];
watch(
    () => isLoading.value && isYoutubePlayback.value,
    (loadingYoutube) => {
        ytLoadingNoticeTimers.forEach((timer) => window.clearTimeout(timer));
        ytLoadingNoticeTimers = [];
        if (!loadingYoutube) return;
        ytLoadingNoticeTimers.push(
            window.setTimeout(() => {
                if (isLoading.value && isYoutubePlayback.value) {
                    showMessageOverlay(
                        "YouTube is slow-starting this stream — hang on…",
                        3600,
                    );
                }
            }, 10_000),
            window.setTimeout(() => {
                if (isLoading.value && isYoutubePlayback.value) {
                    showMessageOverlay(
                        "Still throttled by YouTube — first plays can take up to a minute",
                        5000,
                    );
                }
            }, 32_000),
        );
    },
);

// YouTube stream URLs are tied to the IP that resolved them, so a VPN hop
// (or an expiry) turns them into 403s and mpv reports an error end-file.
// Re-resolve once and resume where we were.
let ytStreamRetryUrl = "";

const retryYoutubeWithFreshStream = async (): Promise<boolean> => {
    const url = player.state.media.url;
    if (!isYoutubePlayback.value || !url) return false;
    if (ytStreamRetryUrl === url) return false;
    ytStreamRetryUrl = url;
    showMessageOverlay("Stream expired — refreshing…", 2600);
    try {
        await player.loadFile(
            pendingResume.value?.position ?? 0,
            true,
            ytQualityOverride.value ?? undefined,
            true,
        );
        return true;
    } catch (error) {
        showMessageOverlay(
            String(error).replace(/^Error:\s*/, "").slice(0, 160),
            3600,
        );
        return false;
    }
};

watch(
    () => player.state.media.url,
    () => {
        ytStreamRetryUrl = "";
    },
);

// --- download the video that's playing ------------------------------------
const youtubeDownloads = useYouTubeDownloads();
const playerDownloadItem = ref<{
    kind: "video";
    id: string;
    url: string;
    title: string;
    channel?: string | null;
    durationText?: string | null;
} | null>(null);
const isPlayerDownloadDialogOpen = ref(false);

const onDownloadCurrentYoutube = () => {
    const url = player.state.media.url;
    if (!url || !isYoutubePlayback.value) return;
    playerDownloadItem.value = {
        kind: "video",
        id: url,
        url,
        title: player.state.media.title?.trim() || "YouTube video",
        channel: null,
        durationText: player.formatTime(player.state.playback.duration),
    };
    isPlayerDownloadDialogOpen.value = true;
};

const onPlayerDownloadConfirm = async (options: DownloadOptions) => {
    const target = playerDownloadItem.value;
    isPlayerDownloadDialogOpen.value = false;
    if (!target) return;
    try {
        await youtubeDownloads.add(
            { url: target.url, title: target.title },
            options,
        );
        showMessageOverlay(
            options.front ? "Downloading…" : "Added to download queue",
            2600,
        );
    } catch (error) {
        showMessageOverlay(
            String(error).replace(/^Error:\s*/, "").slice(0, 160),
            3600,
        );
    }
};

const onSetYoutubeQuality = async (height: number | null) => {
    if (!isYoutubePlayback.value) return;
    if (height === ytQualityOverride.value) return;
    ytQualityOverride.value = height;
    const position = player.state.playback.currentTime;
    const autoPlay = player.state.playback.isPlaying;
    showMessageOverlay(
        height ? `Switching to ${height}p…` : "Switching to default quality…",
        2200,
    );
    try {
        await player.loadFile(position, autoPlay, height ?? undefined);
    } catch (error) {
        showMessageOverlay(
            String(error).replace(/^Error:\s*/, "").slice(0, 160),
            3600,
        );
    }
};

const onRemoveFavorite = (entry: PlaylistEntry) => {
    playlistState.removeFromFavorites(entry.path);
};

const onClearFavorites = () => {
    playlistState.clearFavorites();
};

// --- favourite folders + export / import --------------------------------
const onSelectFavoriteFolder = (id: string | null) => {
    playlistState.setActiveFavoriteFolder(id);
};
const onCreateFavoriteFolder = (name: string) => {
    const id = playlistState.createFavoriteFolder(name);
    if (id) playlistState.setActiveFavoriteFolder(id);
};
const onRenameFavoriteFolder = (payload: { id: string; name: string }) => {
    playlistState.renameFavoriteFolder(payload.id, payload.name);
};
const onDeleteFavoriteFolder = (id: string) => {
    playlistState.deleteFavoriteFolder(id);
};
const onMoveFavoriteToFolder = (payload: { path: string; folderId: string }) => {
    playlistState.moveFavoriteToFolder(payload.path, payload.folderId);
};
const onMoveManyFavorites = (payload: { paths: string[]; folderId: string }) => {
    for (const path of payload.paths) {
        playlistState.moveFavoriteToFolder(path, payload.folderId);
    }
    showMessageOverlay(
        `Moved ${payload.paths.length} to ${
            playlistState.favoriteFolderList.value.find(
                (f) => f.id === payload.folderId,
            )?.name ?? "folder"
        }`,
    );
};
const onRemoveManyFavorites = (paths: string[]) => {
    for (const path of paths) playlistState.removeFromFavorites(path);
    showMessageOverlay(
        `Removed ${paths.length} favourite${paths.length === 1 ? "" : "s"}`,
    );
};

// "Current folder" for quick-favourites only applies while the Favourites view
// is open; anywhere else a new favourite goes to the default folder.
watch(activePanel, (panel) => {
    if (panel !== "favorites") playlistState.setActiveFavoriteFolder(null);
});

const onExportFavorites = async () => {
    try {
        const saved = await saveJsonFile(
            "lumo-favorites.json",
            playlistState.exportFavoritesData(),
        );
        if (saved) showMessageOverlay("Favourites exported");
    } catch (error) {
        showMessageOverlay(
            String(error).replace(/^Error:\s*/, "").slice(0, 160),
            3600,
        );
    }
};

const onImportFavorites = async () => {
    try {
        const data = await openJsonFile();
        if (!data) return;
        if (
            typeof data !== "object" ||
            (data as { kind?: string }).kind !== "lumo-favorites"
        ) {
            showMessageOverlay("Not a Lumo favourites file", 3600);
            return;
        }
        const mode = await confirmImportMode("favourites");
        if (!mode) return;
        const added = playlistState.importFavoritesData(data, mode);
        showMessageOverlay(`Imported ${added} favourite${added === 1 ? "" : "s"}`);
    } catch (error) {
        showMessageOverlay(
            String(error).replace(/^Error:\s*/, "").slice(0, 160),
            3600,
        );
    }
};

// --- settings + keyboard-shortcuts export / import ----------------------
// Ask (at export time) whether to include the AI API keys, which live in the
// aiConfig slice. Everything else (preferences, shortcuts) is non-secret.
const settingsExportPrompt = ref<{
    resolve: (value: { includeKeys: boolean } | null) => void;
} | null>(null);
const askSettingsExportOptions = (): Promise<{ includeKeys: boolean } | null> =>
    new Promise((resolve) => {
        settingsExportPrompt.value = { resolve };
    });
const resolveSettingsExport = (value: { includeKeys: boolean } | null) => {
    settingsExportPrompt.value?.resolve(value);
    settingsExportPrompt.value = null;
};

// Merge imported setting groups over the existing ones (match by group title,
// then by item label); groups/items not present in the import are kept.
const mergeSettingsGroups = (existing: unknown, incoming: unknown): unknown => {
    type Group = { title: string; items: Array<{ label: string; value: string }> };
    const ex = (existing as { groups?: Group[] })?.groups ?? [];
    const inc = (incoming as { groups?: Group[] })?.groups ?? [];
    const byTitle = new Map<string, Group>();
    for (const g of ex) byTitle.set(g.title, { title: g.title, items: [...(g.items ?? [])] });
    for (const g of inc) {
        const target = byTitle.get(g.title) ?? { title: g.title, items: [] };
        const items = [...target.items];
        for (const item of g.items ?? []) {
            const idx = items.findIndex((i) => i.label === item.label);
            if (idx >= 0) items[idx] = { ...items[idx], value: item.value };
            else items.push(item);
        }
        byTitle.set(g.title, { title: g.title, items });
    }
    return { ...(incoming as object), groups: [...byTitle.values()] };
};

const onExportSettings = async () => {
    try {
        const opts = await askSettingsExportOptions();
        if (!opts) return;
        const stored = await loadUiState<{
            settings?: unknown;
            aiConfig?: Record<string, unknown>;
        }>();
        const payload: Record<string, unknown> = {
            kind: "lumo-settings",
            version: 1,
            exportedAt: Date.now(),
            settings: stored?.settings ?? null,
        };
        if (stored?.aiConfig) {
            payload.aiConfig = opts.includeKeys
                ? stored.aiConfig
                : { ...stored.aiConfig, keys: {} };
        }
        const saved = await saveJsonFile("lumo-settings.json", payload);
        if (saved) {
            showMessageOverlay(
                opts.includeKeys
                    ? "Settings exported (with API keys)"
                    : "Settings exported",
            );
        }
    } catch (error) {
        showMessageOverlay(
            String(error).replace(/^Error:\s*/, "").slice(0, 160),
            3600,
        );
    }
};

const onImportSettings = async () => {
    try {
        const data = await openJsonFile();
        if (!data) return;
        if (
            typeof data !== "object" ||
            (data as { kind?: string }).kind !== "lumo-settings"
        ) {
            showMessageOverlay("Not a Lumo settings file", 3600);
            return;
        }
        const mode = await confirmImportMode("settings");
        if (!mode) return;
        const incoming = data as {
            settings?: unknown;
            aiConfig?: Record<string, unknown>;
        };
        const stored = await loadUiState<{
            settings?: unknown;
            aiConfig?: Record<string, unknown>;
        }>();
        const patch: Record<string, unknown> = {
            settings:
                mode === "merge"
                    ? mergeSettingsGroups(stored?.settings, incoming.settings)
                    : incoming.settings,
        };
        if (incoming.aiConfig) {
            patch.aiConfig =
                mode === "merge"
                    ? { ...(stored?.aiConfig ?? {}), ...incoming.aiConfig }
                    : incoming.aiConfig;
        }
        await saveUiState(patch);
        showMessageOverlay("Settings imported — reloading…", 1600);
        window.setTimeout(() => window.location.reload(), 700);
    } catch (error) {
        showMessageOverlay(
            String(error).replace(/^Error:\s*/, "").slice(0, 160),
            3600,
        );
    }
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

// Ultra Slo-Mo: hold a key (default X) to ramp into smooth, interpolated slow
// motion; release to ramp back. The factor picker lives in the video menu.
const ultraSlomo = useUltraSlomo({
    isFileLoaded: () => player.state.media.isFileLoaded,
});

const { onKeydown, onKeyup, onDoubleClick, bindings: shortcutBindings } = usePlaybackShortcuts(
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
        toggleCurves: () => toggleCurves(),
        cycleNightMode: () => audio.cycleNightMode(),
        toggleAudioPanel: () => toggleAudioPanel(),
        zoomIn: () => transform.zoomIn(),
        zoomOut: () => transform.zoomOut(),
        rotateVideo: () => transform.rotateBy(90),
        resetTransform: () => transform.reset(),
        abRangeCycle: () => abRange.cycle(),
        abRangeClear: () => abRange.clear(),
        skipIntro: () => {
            if (skipMarkers.promptKind.value === "credits") {
                void skipMarkers.skipCredits();
            } else if (skipMarkers.promptKind.value === "intro") {
                void skipMarkers.skipIntro();
            }
        },
        windowSizeUp: () => stepWindowSize(1.1),
        windowSizeDown: () => stepWindowSize(0.9),
        startUltraSlomo: () => void ultraSlomo.start(),
        stopUltraSlomo: () => void ultraSlomo.stop(),
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
        closeTopOverlay: () => closeTopOverlay(),

        // File & Export — the same handlers the Media menu uses.
        openFile: () => requestOpenFilePicker(),
        openFileOrFolder: () => void openFileOrFolderPicker(),
        openNetworkStream: () => void onSideNavNavigate("network"),
        showRecent: () => void onSideNavNavigate("history"),
        showFavourites: () => void onSideNavNavigate("favorites"),
        addToPlaylist: () => requestAddPlaylistItem(),
        saveContactSheet: () => void onExportContactSheet(),
        exportClip: () => exportClipFromRange(false),
        exportGif: () => exportClipFromRange(true),
        openExportFolder: () => void openExportFolder(),
        quitApp: () => quitApp(),
        commandPalette: () => {
            isPaletteOpen.value = !isPaletteOpen.value;
        },
        toggleSplitCompare: () => void splitCompare.toggle(),
        syncSubtitlesByEar: () => void subtitleSync.syncNow(),
        nextScene: () => void sceneIndex.next(player.state.playback.currentTime),
        previousScene: () =>
            void sceneIndex.previous(player.state.playback.currentTime),
        viewOriginal: () => enhancementHistory.toggleBypass(),
        undoEnhancement: () => void enhancementHistory.undo(),
        redoEnhancement: () => void enhancementHistory.redo(),
        runCustomShortcut: (chord) => customShortcuts.runChord(chord),
        canExportRange: () => abRange.hasRange.value && isLocalMediaPath.value,
        canExportClip: () => isClipExportAvailable.value,
        isLocalMedia: () => isLocalMediaPath.value,
    },
);

const currentMediaKey = () => player.state.media.url;
const geometry = useVideoGeometry();
const transform = useVideoTransform();
transform.setMessageHandler(showMessageOverlay);

// A-B range: loops the selection, and supplies the source range for clip/GIF
// export and for saving a series' intro/credits markers.
const abRange = useAbRange({
    getPosition: () => player.state.playback.currentTime,
    getDuration: () => player.state.playback.duration,
    isFileLoaded: () => player.state.media.isFileLoaded,
    onMessage: showMessageOverlay,
});

const skipMarkers = useSkipMarkers({
    getPosition: () => player.state.playback.currentTime,
    getDuration: () => player.state.playback.duration,
    onMessage: showMessageOverlay,
    seekTo: (seconds) => onSeek(seconds),
    playNext: () => onNextTrack(),
});

// Re-check the intro/credits windows as playback advances. Cheap: it bails
// immediately when the current folder has no markers.
watch(
    () => player.state.playback.currentTime,
    () => skipMarkers.evaluate(),
);

// Hard ceiling on GIF length; the practical limit is the resolution-aware
// pixel budget the backend enforces (mirrored in ClipBar).
const GIF_MAX_SECONDS = 60;
// Export runs through a headless mpv encode pass, so it needs a real file.
const isLocalMediaPath = computed(
    () =>
        !!player.state.media.url.trim() &&
        !/^(https?|rtsp|rtmp|smb|webdav):\/\//i.test(player.state.media.url),
);
const isExportingClip = ref(false);
// Video clips are re-encoded by a full ffmpeg; the bundled playback ffmpeg has
// no muxers or encoders. GIF export is in-process and always available.
const isClipExportAvailable = ref(false);
const refreshClipExportAvailability = async () => {
    try {
        isClipExportAvailable.value = await invoke<boolean>("clip_export_available");
    } catch {
        isClipExportAvailable.value = false;
    }
};
// Detection spawns `ffmpeg -version`, so don't do it at launch — wait until a
// range exists and the export buttons could actually be used.
watch(
    () => abRange.hasRange.value,
    (hasRange) => {
        if (hasRange) void refreshClipExportAvailability();
    },
);
const onExportClip = async (payload: { asGif: boolean; gifWidth: number }) => {
    if (isExportingClip.value) return;
    const { asGif, gifWidth } = payload;
    const path = player.state.media.url.trim();
    const start = abRange.pointA.value;
    const end = abRange.pointB.value;
    if (!path || start === null || end === null) return;
    isExportingClip.value = true;
    // A full-resolution GIF can take a while to quantize, so keep the notice up.
    showMessageOverlay(asGif ? "Rendering GIF…" : "Exporting clip…", 20000);
    try {
        const result = await invoke<{ path: string; fileName: string }>(
            "export_clip",
            {
                path,
                title: player.state.media.title,
                start,
                end,
                gif: asGif,
                gifWidth,
                gifFps: 12,
            },
        );
        showMessageOverlay(`Saved · ${result.fileName}`, 3200);
    } catch (error) {
        console.warn("[clip] export failed", error);
        showMessageOverlay(`Export failed: ${error}`, 3600);
    } finally {
        isExportingClip.value = false;
    }
};

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
// Flip the Global toggle: switch colour adjustments AND the enhancement look
// between per-file and global for the current video in one step.
const onSetGlobalColorAdjustments = async (enabled: boolean) => {
    await adjustments.setGlobalColorAdjustmentsEnabled(enabled);
    await enhancements.reapplyLook();
};

// --- zoom & pan interaction ------------------------------------------------
// Ctrl+wheel zooms about the cursor; once zoomed, a left-drag on the video pans
// (window dragging is suppressed for the duration — see useWindowDragRegion).

// Anything in this list is real UI, not the video surface.
const isVideoSurfaceTarget = (target: EventTarget | null): boolean => {
    if (!(target instanceof Element)) return false;
    return !target.closest(
        [
            "button",
            "input",
            "textarea",
            "select",
            "a[href]",
            "[role='menu']",
            "[role='menuitem']",
            "[role='slider']",
            "[contenteditable='true']",
            "[data-window-no-drag]",
            ".top-bar",
            ".player-controls-content",
            ".playlist-drawer",
            ".side-actions-nav",
            ".track-menu-container",
        ].join(", "),
    );
};

const onVideoWheel = (event: WheelEvent) => {
    if (!event.ctrlKey) return;
    if (!player.state.media.isFileLoaded) return;
    if (!isVideoSurfaceTarget(event.target)) return;
    event.preventDefault();
    // Anchor the zoom on the cursor so the point under it stays put.
    const anchorX = event.clientX / Math.max(1, window.innerWidth);
    const anchorY = event.clientY / Math.max(1, window.innerHeight);
    const steps = event.deltaY > 0 ? -1 : 1;
    transform.zoomBy(steps * transform.ZOOM_STEP, anchorX, anchorY);
    showMessageOverlay(`Zoom ${transform.zoomPercent.value}%`);
};

let panPointerId: number | null = null;
let panLastX = 0;
let panLastY = 0;

const onVideoPanPointerMove = (event: PointerEvent) => {
    if (event.pointerId !== panPointerId) return;
    // Pan is expressed in window-size fractions, matching mpv's video-pan-*.
    transform.panBy(
        (event.clientX - panLastX) / Math.max(1, window.innerWidth),
        (event.clientY - panLastY) / Math.max(1, window.innerHeight),
    );
    panLastX = event.clientX;
    panLastY = event.clientY;
};

const endVideoPan = () => {
    panPointerId = null;
    window.removeEventListener("pointermove", onVideoPanPointerMove);
    window.removeEventListener("pointerup", endVideoPan);
    window.removeEventListener("pointercancel", endVideoPan);
};

const onVideoPanPointerDown = (event: PointerEvent) => {
    if (event.button !== 0) return;
    if (!transform.isZoomed.value) return;
    if (!isVideoSurfaceTarget(event.target)) return;
    panPointerId = event.pointerId;
    panLastX = event.clientX;
    panLastY = event.clientY;
    window.addEventListener("pointermove", onVideoPanPointerMove);
    window.addEventListener("pointerup", endVideoPan);
    window.addEventListener("pointercancel", endVideoPan);
};

// Curves panel and the playlist drawer share the right edge, so they're
// mutually exclusive — opening one closes the other.
const openCurves = () => {
    closePlaylist();
    isSettingsOpen.value = false;
    isAudioPanelOpen.value = false;
    isCurvesOpen.value = true;
};
const toggleCurves = () => {
    if (isCurvesOpen.value) {
        isCurvesOpen.value = false;
        return;
    }
    if (!player.state.media.isFileLoaded) return;
    openCurves();
};
watch(isPlaylistOpen, (open) => {
    if (open) {
        isCurvesOpen.value = false;
        isAudioPanelOpen.value = false;
        isSettingsOpen.value = false;
    }
});

// The audio panel shares the right edge with curves and the playlist drawer.
const openAudioPanel = () => {
    closePlaylist();
    isSettingsOpen.value = false;
    isCurvesOpen.value = false;
    isAudioPanelOpen.value = true;
};
const toggleAudioPanel = () => {
    if (isAudioPanelOpen.value) {
        isAudioPanelOpen.value = false;
        return;
    }
    openAudioPanel();
};

// Settings is a modal overlay that floats over live playback (it never stops
// the video). It's mutually exclusive with the curves panel and playlist.
const openSettings = () => {
    closePlaylist();
    isCurvesOpen.value = false;
    isAudioPanelOpen.value = false;
    isSettingsOpen.value = true;
};
const closeSettings = () => {
    isSettingsOpen.value = false;
    // The ffmpeg path may have just been set, which enables clip export and the
    // merge/split tools — re-probe unconditionally (those tools don't need an
    // A-B range, so a range-gated check would leave them stuck disabled).
    void refreshClipExportAvailability();
};
// The update-note "install" flow sets activePanel to the settings id to reveal
// the update UI. Settings is no longer a main panel, so translate that into
// opening the overlay and keep the main panel state valid.
watch(activePanel, (panel) => {
    if (panel === "settings") {
        activePanel.value = "home";
        openSettings();
    }
});

// Reset the current video's full look: colour adjustments + colour grade +
// sharpen/denoise/deinterlace. Quality preset & AI Upscale (global) are kept.
const onResetVideoSettings = async () => {
    await adjustments.setBrightness(0);
    await adjustments.setContrast(0);
    await adjustments.setSaturation(0);
    await adjustments.setGamma(0);
    await adjustments.setHue(0);
    await enhancements.resetLook();
    showMessageOverlay("Video settings reset");
};

// Video-look presets (built-in + custom). Applying sets the current video's
// colour look; saving snapshots it into a reusable named preset.
const videoPresets = useVideoPresets({
    applyValues: async (v) => {
        await adjustments.setBrightness(v.brightness);
        await adjustments.setContrast(v.contrast);
        await adjustments.setSaturation(v.saturation);
        await adjustments.setGamma(v.gamma);
        await adjustments.setHue(v.hue);
        enhancements.setColorGrade("exposure", v.exposure);
        enhancements.setColorGrade("temperature", v.temperature);
        enhancements.setColorGrade("tint", v.tint);
        enhancements.setColorGrade("highlights", v.highlights);
        enhancements.setColorGrade("shadows", v.shadows);
    },
    readCurrentValues: () => ({
        brightness: adjustments.brightness.value,
        contrast: adjustments.contrast.value,
        saturation: adjustments.saturation.value,
        gamma: adjustments.gamma.value,
        hue: adjustments.hue.value,
        exposure: enhancements.state.exposure,
        temperature: enhancements.state.temperature,
        tint: enhancements.state.tint,
        highlights: enhancements.state.highlights,
        shadows: enhancements.state.shadows,
    }),
    onApplied: (name) => showMessageOverlay(`Preset: ${name}`),
});

// Cloud AI correction — shared by the Video-popover "AI Enhance" button and the
// Curves-panel "AI Correct" button. Applies per-channel curves plus a
// saturation / temperature / tint nudge and sharpen / grain, all as one
// undoable step.
type AiCurveResult = {
    rgb: [number, number][];
    r: [number, number][];
    g: [number, number][];
    b: [number, number][];
    saturation: number;
    temperature: number;
    tint: number;
    sharpen: number;
    sharpenRadius: number;
    grain: number;
    notes: string;
};
// Merge / split modals (file operations in the Media menu).
const mergeOpen = ref(false);
const splitOpen = ref(false);

// AI subtitle generation modal.
const subtitleAiOpen = ref(false);
const onGenerateAiSubtitles = () => {
    if (!isLocalMediaPath.value) {
        showMessageOverlay("AI subtitles need a local video file", 3000);
        return;
    }
    subtitleAiOpen.value = true;
};
// AI subtitle translation modal (translate an existing .srt).
const subtitleTranslateOpen = ref(false);
const onTranslateAiSubtitles = () => {
    subtitleTranslateOpen.value = true;
};
// AI subtitle sync modal (re-time an existing .srt to this video).
const subtitleSyncOpen = ref(false);
const onSyncAiSubtitles = () => {
    if (!isLocalMediaPath.value) {
        showMessageOverlay("Subtitle sync needs a local video file", 3000);
        return;
    }
    subtitleSyncOpen.value = true;
};
// Describe the current A–B clip with AI.
const describeClipOpen = ref(false);
const describeClipStart = ref(0);
const describeClipEnd = ref(0);
const onDescribeClip = () => {
    const a = abRange.pointA.value;
    const b = abRange.pointB.value;
    if (a == null || b == null || b <= a) {
        showMessageOverlay("Mark an A–B range first (press K twice)", 3000);
        return;
    }
    if (!isLocalMediaPath.value) {
        showMessageOverlay("Clip description needs a local video file", 3000);
        return;
    }
    describeClipStart.value = a;
    describeClipEnd.value = b;
    describeClipOpen.value = true;
};
const onAiSubtitlesLoaded = async (payload: {
    path: string;
    lineCount: number;
}) => {
    try {
        // Remove any external subtitle track already pointing at this file, so
        // re-running (same output filename) doesn't pile up duplicate tracks.
        const samePath = (a: string, b: string) =>
            a.replace(/\\/g, "/").toLowerCase() ===
            b.replace(/\\/g, "/").toLowerCase();
        try {
            const countRaw = await invoke<string | null>("mpv_get_property_string", {
                name: "track-list/count",
            });
            const count = Number.parseInt(countRaw ?? "0", 10) || 0;
            const staleIds: string[] = [];
            for (let i = 0; i < count; i += 1) {
                const type = await invoke<string | null>("mpv_get_property_string", {
                    name: `track-list/${i}/type`,
                });
                if (type !== "sub") continue;
                const fn = await invoke<string | null>("mpv_get_property_string", {
                    name: `track-list/${i}/external-filename`,
                });
                if (fn && samePath(fn, payload.path)) {
                    const id = await invoke<string | null>("mpv_get_property_string", {
                        name: `track-list/${i}/id`,
                    });
                    if (id) staleIds.push(id);
                }
            }
            for (const id of staleIds) {
                await invoke("mpv_run_command", { args: ["sub-remove", id] });
            }
        } catch {
            /* track-list probe is best-effort */
        }

        await invoke("mpv_run_command", {
            args: ["sub-add", payload.path, "select", "AI Subtitles"],
        });
        // If this file was already loaded (e.g. a previous range), mpv selects
        // the cached copy instead of re-reading it — force a reload so newly
        // merged lines actually appear.
        const sid = await invoke<string | null>("mpv_get_property_string", {
            name: "sid",
        });
        if (sid && sid !== "no" && sid !== "auto") {
            await invoke("mpv_run_command", { args: ["sub-reload", sid] });
        }
    } catch (error) {
        showMessageOverlay(
            String(error).replace(/^Error:\s*/, "").slice(0, 140),
            3200,
        );
    }
};

// Import mode chooser (Merge / Replace / Cancel) shared by favourites & settings
// import. confirmImportMode resolves with the user's choice.
type ImportMode = "merge" | "replace";
const importPrompt = ref<{
    kind: string;
    resolve: (mode: ImportMode | null) => void;
} | null>(null);
const confirmImportMode = (kind: string): Promise<ImportMode | null> =>
    new Promise((resolve) => {
        importPrompt.value = { kind, resolve };
    });
const resolveImportMode = (mode: ImportMode | null) => {
    importPrompt.value?.resolve(mode);
    importPrompt.value = null;
};

const aiConfig = useAiConfig();
const isAiEnhancing = ref(false);
const aiPromptOpen = ref(false);
const aiPromptText = ref("");
const aiFetchStatus = ref("");
const aiFetching = ref(false);
const onPromptFetchModels = async () => {
    if (aiFetching.value) return;
    aiFetching.value = true;
    aiFetchStatus.value = "Fetching…";
    const result = await aiConfig.fetchModels();
    aiFetchStatus.value = result.message;
    aiFetching.value = false;
};
const aiPromptInput = ref<HTMLTextAreaElement | null>(null);
watch(aiPromptOpen, (open) => {
    if (open) void nextTick(() => aiPromptInput.value?.focus());
});

// Reuse a past prompt: refill the text and restore the provider/model it ran on.
const reuseAiPrompt = (entry: {
    prompt: string;
    provider: string;
    model: string;
}) => {
    aiPromptText.value = entry.prompt;
    if (entry.provider) aiConfig.setProvider(entry.provider);
    if (entry.model) aiConfig.setModel(entry.model);
    void nextTick(() => aiPromptInput.value?.focus());
};

const onSelectRecent = (rawIndex: string) => {
    const entry = aiConfig.promptHistory.value[Number(rawIndex)];
    if (entry) reuseAiPrompt(entry);
};

// Reference images that steer the correction toward a target look. Session-only
// (kept while the app is open, cleared on restart); capped at three.
const MAX_REFERENCE_IMAGES = 3;
const aiRefImages = ref<{ path: string; thumb: string }[]>([]);

const onAddReferenceImages = async () => {
    if (aiRefImages.value.length >= MAX_REFERENCE_IMAGES) return;
    const selected = await openFileDialog({
        multiple: true,
        directory: false,
        filters: [
            {
                name: "Images",
                extensions: [
                    "jpg", "jpeg", "png", "webp", "bmp", "gif", "tif", "tiff",
                ],
            },
        ],
    });
    const paths = Array.isArray(selected)
        ? selected
        : selected
          ? [selected]
          : [];
    for (const path of paths) {
        if (aiRefImages.value.length >= MAX_REFERENCE_IMAGES) break;
        if (aiRefImages.value.some((r) => r.path === path)) continue;
        try {
            const thumb = await invoke<string>("ai_reference_thumbnail", { path });
            aiRefImages.value = [...aiRefImages.value, { path, thumb }];
        } catch (error) {
            showMessageOverlay(
                String(error).replace(/^Error:\s*/, "").slice(0, 140),
                3200,
            );
        }
    }
};

const removeReferenceImage = (index: number) => {
    aiRefImages.value = aiRefImages.value.filter((_, i) => i !== index);
};

// The AI buttons open a prompt first so you can steer the correction
// ("warmer, less saturation"); leaving it blank runs a general best-effort pass.
const onAiEnhance = () => {
    if (isAiEnhancing.value) return;
    const path = player.state.media.url.trim();
    if (!path || player.state.playback.duration <= 0) return;
    if (!isLocalMediaPath.value) {
        showMessageOverlay("AI correction needs a local video file", 3000);
        return;
    }
    aiPromptOpen.value = true;
};

const runAiEnhance = async () => {
    const instruction = aiPromptText.value.trim();
    const provider = aiConfig.provider.value;
    const model = aiConfig.currentModel.value;
    aiPromptOpen.value = false;
    if (isAiEnhancing.value) return;
    const path = player.state.media.url.trim();
    const duration = player.state.playback.duration;
    if (!path || duration <= 0) return;
    isAiEnhancing.value = true;
    showMessageOverlay("Analyzing with AI…", 20000);
    try {
        const result = await invoke<AiCurveResult>("ai_curve_correction", {
            path,
            duration,
            provider,
            apiKey: aiConfig.currentKey.value,
            model: model || null,
            baseUrl: aiConfig.currentBaseUrl.value || null,
            instruction: instruction || null,
            referenceImages: aiRefImages.value.map((r) => r.path),
        });
        // Tag the resulting undo checkpoint with the model, the prompt used, and
        // the model's own description, so cycling back to it shows all three.
        enhancementHistory.tagNextChange({
            label: `AI · ${model || provider}`,
            prompt: instruction,
            notes: result.notes,
        });
        // Remember the prompt + model so it can be reused.
        aiConfig.addPromptHistory({ prompt: instruction, provider, model });
        const toPoints = (arr: [number, number][]) =>
            arr.map(([x, y]) => ({ x, y }));
        enhancements.setCurves(
            serializeCurves({
                rgb: toPoints(result.rgb),
                r: toPoints(result.r),
                g: toPoints(result.g),
                b: toPoints(result.b),
            }),
        );
        enhancements.setColorGrade("temperature", result.temperature);
        enhancements.setColorGrade("tint", result.tint);
        await adjustments.setSaturation(result.saturation);
        enhancements.setSharpenAmount(result.sharpen);
        enhancements.setSharpenRadius(result.sharpenRadius);
        enhancements.setGrain(result.grain);
        showMessageOverlay(result.notes || "AI correction applied", 3600);
    } catch (error) {
        const message = String(error)
            .replace(/^Error:\s*/, "")
            .slice(0, 160);
        showMessageOverlay(`AI correction failed: ${message}`, 4500);
        console.warn("[ai-enhance] failed", error);
    } finally {
        isAiEnhancing.value = false;
    }
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
audio.setMessageHandler(showMessageOverlay);

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
} = useWindowDragRegion({
    // While zoomed in, a left-drag on the video pans the image rather than
    // moving the window (see onVideoPanPointerDown).
    shouldSuppress: () => transform.isZoomed.value,
});
const { mediaInfo, statusBadges } = useMediaInfo(player);
const currentOrLastPlaybackUrl = computed(
    () => player.state.media.url || player.state.media.lastLoadedUrl,
);
const playlistEntriesWithProgress = usePlaylistEntriesWithProgress(
    orderedPlaylist,
    history.history,
);
// Force-rebuild the current video's seek-bar thumbnails (e.g. when the cached
// set came out wrong). Bumping the token clears the seek bar's in-memory cache
// so the freshly generated frames are fetched on the next hover.
const thumbReloadToken = ref(0);
const onRegenerateThumbnails = async () => {
    const path = player.state.media.url.trim();
    const duration = player.state.playback.duration;
    if (!path || duration <= 0) return;
    showMessageOverlay("Regenerating thumbnails…");
    try {
        await invoke("regenerate_seek_thumbnails", { path, duration });
        thumbReloadToken.value += 1;
        showMessageOverlay("Thumbnails regenerated");
    } catch (error) {
        console.warn("[thumbnails] regenerate failed", error);
        showMessageOverlay("Thumbnail regeneration failed");
    }
};

// Tile evenly-spaced frames into one timestamped JPEG next to the screenshots.
// Extraction runs a headless mpv pass, so it takes a few seconds on long files.
const isExportingContactSheet = ref(false);
const onExportContactSheet = async () => {
    if (isExportingContactSheet.value) return;
    const path = player.state.media.url.trim();
    const duration = player.state.playback.duration;
    if (!path || duration <= 0) return;
    isExportingContactSheet.value = true;
    showMessageOverlay("Building contact sheet…", 6000);
    try {
        const result = await invoke<{ path: string; fileName: string }>(
            "export_contact_sheet",
            { path, title: player.state.media.title, duration },
        );
        showMessageOverlay(`Contact sheet saved · ${result.fileName}`, 3200);
    } catch (error) {
        console.warn("[contact-sheet] export failed", error);
        showMessageOverlay(`Contact sheet failed: ${error}`, 3600);
    } finally {
        isExportingContactSheet.value = false;
    }
};

const playbackContextMenu = usePlaybackContextMenu({
    isFileLoaded: () => player.state.media.isFileLoaded,
    getCurrentPath: () => player.state.media.url,
    getCurrentTitle: () => player.state.media.title,
    // Route through the same handler as the toolbar so a poster frame is
    // captured; adding directly would leave the Favourites tile blank.
    addToFavorites: () => void onToggleFavorite(),
    isFavorite: () => isCurrentFavorite.value,
    searchOnlineSubtitles: tracks.searchOnlineSubtitleTracks,
    openSubtitleAdvancedSettings: () => {
        tracks.showSubMenu.value = true;
        tracks.showSubtitleAdvancedSettings.value = true;
    },
    openSettings: openSettingsFromPlaybackContextMenu,
    regenerateThumbnails: onRegenerateThumbnails,
    exportContactSheet: onExportContactSheet,
    isMenuBarVisible: () => isMenuBarVisible.value,
    toggleMenuBar: () => setMenuBarVisible(!isMenuBarVisible.value),
    hideAllMenus,
});

// --- application menu bar -------------------------------------------------
// A VLC-style menu row above the header. Purely an additional route to actions
// that already exist; nothing here reimplements behaviour.

const isMenuBarVisible = ref(true);
const menuBarSaver = createDebouncedUiStateSaver(300);
const setMenuBarVisible = (visible: boolean) => {
    isMenuBarVisible.value = visible;
    menuBarSaver.saveDebounced({ menuBarVisible: visible });
};
void loadUiState<{ menuBarVisible?: boolean }>().then((stored) => {
    if (stored?.menuBarVisible === false) isMenuBarVisible.value = false;
});

// The menu bar is an opaque strip over the mpv surface. Reserving a video
// margin to sit it "above" the picture pillarboxes the frame (mpv scales the
// whole video down to keep aspect), so instead the bar fades in and out with
// the rest of the on-screen chrome: while you're watching (mouse idle) the
// video is full and unobstructed, and moving the mouse brings the menu back.
// It stays permanently visible only on the home/browse screens, where there's
// no video to cover.
const isMenuBarShown = computed(() => {
    if (!isMenuBarVisible.value) return false;
    if (player.state.window.isFullscreen) return false;
    if (!player.state.media.isFileLoaded) return true;
    return ui.showControls.value;
});

const openExportFolder = async () => {
    try {
        await invoke("open_export_folder");
    } catch (error) {
        console.warn("[menu] open export folder failed", error);
        showMessageOverlay("Couldn't open the export folder");
    }
};

// Media actions, shared by the menu bar and the keyboard shortcuts so both
// routes call exactly the same code.
const openFileOrFolderPicker = async () => {
    const selected = await player.pickMediaPathsAuto();
    if (selected.length) await openSelectedPaths(selected);
};
const quitApp = () => void getCurrentWindow().close();
const exportClipFromRange = (asGif: boolean) =>
    void onExportClip({ asGif, gifWidth: asGif ? 720 : 0 });

// View Original bypass + per-file undo/redo across every picture and audio
// enhancement.
const enhancementHistory = useEnhancementHistory({
    enhancements,
    audio,
    adjustments: {
        brightness: adjustments.brightness,
        contrast: adjustments.contrast,
        saturation: adjustments.saturation,
        gamma: adjustments.gamma,
        hue: adjustments.hue,
        setBrightness: adjustments.setBrightness,
        setContrast: adjustments.setContrast,
        setSaturation: adjustments.setSaturation,
        setGamma: adjustments.setGamma,
        setHue: adjustments.setHue,
        applyNeutral: adjustments.applyNeutral,
        reapplyColorAdjustments: adjustments.reapplyColorAdjustments,
    },
    // Multi-line readouts (AI steps carry a prompt + result) need longer to read.
    onMessage: (text: string) =>
        showMessageOverlay(text, text.includes("\n") ? 5200 : 1400),
    silenceMessageHandlers: () => {
        enhancements.setMessageHandler(() => {});
        audio.setMessageHandler(() => {});
    },
    restoreMessageHandlers: () => {
        enhancements.setMessageHandler(showMessageOverlay);
        audio.setMessageHandler(showMessageOverlay);
    },
});

// Before/after wipe over the enhancement chain.
const splitCompare = useSplitCompare();

// Tap-to-sync subtitles: works out the offset from the nearest subtitle event.
const subtitleSync = useSubtitleSyncByEar({
    getDelay: () => adjustments.subDelay.value,
    setDelay: (value) =>
        adjustments.setSubDelayForTarget({ target: "primary", value }),
    onMessage: showMessageOverlay,
});

// Chapters when the file has them, detected scenes when it doesn't.
const sceneIndex = useSceneIndex({
    getPath: () => player.state.media.url,
    getDuration: () => player.state.playback.duration,
    seekTo: (seconds) => onSeek(seconds),
    formatTime: player.formatTime,
    onMessage: showMessageOverlay,
});

const { settings: youtubeSettings } = useYouTubeSettings();

const ytWatch = useYouTubeWatch({
    mediaUrl: () => player.state.media.url,
    isFileLoaded: () => player.state.media.isFileLoaded,
    setUpNextQueue: (items) => playlistState.setYoutubeUpNext(items),
    setSceneMarkers: (markers) => {
        sceneIndex.markers.value = markers;
    },
    seekTo: (seconds) => onSeek(seconds),
    settings: youtubeSettings,
});

watch(
    () => player.state.playback.currentTime,
    (currentTime) => {
        if (!player.state.playback.isPlaying) return;
        ytWatch.onPlaybackTick(currentTime);
    },
);

const isPaletteOpen = ref(false);

// Shows a command's bound key in the palette, when the user has given it one.
const chordForCommand = (commandId: string): string | undefined =>
    customShortcuts.shortcuts.value.find(
        (entry) => entry.commandId === commandId && entry.kind === "action",
    )?.chord;

// Every bindable command, including the numeric settings that can't live in a
// menu. The shortcut builder in Settings binds against this.
const commandRegistry = useCommandRegistry({
    enhancements,
    audio,
    transform,
    videoPresets,
    adjustments: {
        brightness: () => adjustments.brightness.value,
        contrast: () => adjustments.contrast.value,
        saturation: () => adjustments.saturation.value,
        gamma: () => adjustments.gamma.value,
        hue: () => adjustments.hue.value,
        setBrightness: adjustments.setBrightness,
        setContrast: adjustments.setContrast,
        setSaturation: adjustments.setSaturation,
        setGamma: adjustments.setGamma,
        setHue: adjustments.setHue,
        audioDelay: () => adjustments.audioDelay.value,
        setAudioDelay: adjustments.setAudioDelay,
        subDelay: () => adjustments.subDelay.value,
        setSubDelay: (value: number) =>
            adjustments.setSubDelayForTarget({ target: "primary", value }),
    },

    openVideoPanel: () => toggleMenu("settings"),
    openAudioTrackMenu: () => toggleMenu("audio"),
    openSubtitleMenu: () => toggleMenu("sub"),
    openSpeedMenu: () => toggleMenu("speed"),
    openCurves: () => openCurves(),
    openAudioPanel: () => openAudioPanel(),
    openPlaylist: () => void togglePlaylist(),
    openSettings: () => openSettings(),
    openYouTube: () => void onSideNavNavigate("youtube"),
    openMediaInfo: () => void toggleInfo(),
    openShortcutsHelp: () => {
        isShortcutsHelpOpen.value = !isShortcutsHelpOpen.value;
    },
    closePanels: () => {
        hideAllMenus();
        isCurvesOpen.value = false;
        isAudioPanelOpen.value = false;
        isSettingsOpen.value = false;
        closePlaylist();
    },

    getSpeed: () => player.state.playback.speed,
    setSpeed: (value) => void speed.setSpeed(value),
    getVolume: () => player.state.playback.volume,
    setVolume: (value) => void playbackVolume.setVolume(value),

    autoEnhance: () => void onAutoEnhance(),
    resetVideoSettings: () => void onResetVideoSettings(),
    cycleAspectRatio: () => void onCycleAspectRatio(),
    autoCropNow: () => void autoCrop.detectNow(),
    clearCrop: () => void autoCrop.clear(),

    toggleSplitCompare: () => void splitCompare.toggle(),
    getSplitPosition: () => splitCompare.position.value,
    setSplitPosition: (value) => {
        splitCompare.setPosition(value);
        splitCompare.commitPosition();
    },
    viewOriginal: () => enhancementHistory.toggleBypass(),
    undoEnhancement: () => void enhancementHistory.undo(),
    redoEnhancement: () => void enhancementHistory.redo(),
    oldFilmRestore: () => void enhancements.applyOldFilmRestore(),
    setDeband: (level) => void enhancements.setDeband(level),
    syncSubtitlesByEar: () => void subtitleSync.syncNow(),
    nextScene: () => void sceneIndex.next(player.state.playback.currentTime),
    previousScene: () => void sceneIndex.previous(player.state.playback.currentTime),
    rescanScenes: () => void sceneIndex.scan(true),
});

const customShortcuts = useCustomShortcuts({
    resolve: (id) => commandRegistry.byId.value.get(id),
    onMessage: showMessageOverlay,
});

const { menus: appMenus } = useAppMenu({
    isFileLoaded: () => player.state.media.isFileLoaded,
    isLocalMedia: () => isLocalMediaPath.value,
    isPlaying: () => player.state.playback.isPlaying,
    isFullscreen: () => player.state.window.isFullscreen,
    isAlwaysOnTop: () => isAlwaysOnTop.value,
    isLoopOne: () => isLoopOne.value,
    isFavorite: () => isCurrentFavorite.value,
    areSubtitlesVisible: () => areSubtitlesVisible.value,
    isDualSubEnabled: () => tracks.dualSubEnabled.value,
    currentSpeed: () => player.state.playback.speed,
    playbackRates: () => [...speed.playbackRates],
    aspectLabel: () => geometry.currentAspectLabel.value,
    hasAbRange: () => abRange.hasRange.value,
    clipExportAvailable: () => isClipExportAvailable.value,
    keyFor: (id) => menuShortcut(shortcutBindings.value[id]),

    audio,
    enhancements,
    videoPresets,
    transform,
    ab: abRange,
    skip: skipMarkers,

    audioTracks: () => ({
        tracks: tracks.audioTracks.value,
        select: tracks.selectAudio,
        emptyLabel: "No audio tracks",
    }),
    subtitleTracks: () => ({
        tracks: tracks.subTracks.value,
        select: (track) =>
            tracks.selectSubTrack({ target: "primary", track }),
        emptyLabel: "No subtitle tracks",
    }),

    openFilePicker: requestOpenFilePicker,
    openFileOrFolderPicker: () => void openFileOrFolderPicker(),
    gotoPanel: (panel) => void onSideNavNavigate(panel),
    addToPlaylist: requestAddPlaylistItem,
    exportContactSheet: onExportContactSheet,
    exportClip: exportClipFromRange,
    openExportFolder: () => void openExportFolder(),
    openMergeFiles: () => (mergeOpen.value = true),
    openSplitFile: () => (splitOpen.value = true),
    quit: quitApp,

    togglePlayPause: () => void player.togglePlayPause(),
    seekRelative: (seconds) => void onSeekRelative(seconds),
    seekToStart: () => void onSeek(0),
    previousTrack: () => void onPrevTrack(),
    nextTrack: () => void onNextTrack(),
    frameStep: (forward) => void frameStep(forward ? 1 : -1),
    setSpeed: (rate) => void speed.setSpeed(rate),
    resetSpeed: () => void resetPlaybackSpeed(),
    setSlomoFactor: ultraSlomo.setFactor,
    slomoFactor: () => ultraSlomo.factor.value,
    toggleLoop: () => void toggleLoopWithFeedback(),

    adjustVolume: (delta) => {
        const next = player.state.playback.volume + delta;
        void playbackVolume.setVolume(next);
        showVolumeOverlay(next);
    },
    toggleMuted: () => void onToggleMuted(),
    isMuted: () => player.state.playback.isMuted,
    addExternalAudio: () => void tracks.addExternalAudioTrack(),
    adjustAudioDelay: (delta) => void adjustAudioDelayBy(delta),
    resetAudioDelay: () => void adjustments.setAudioDelay(0),
    openAudioPanel,

    toggleFullscreen: () => void onToggleFullscreen(),
    toggleAlwaysOnTop: () => void toggleAlwaysOnTop(),
    cycleAspectRatio: () => void onCycleAspectRatio(),
    autoCropNow: () => void autoCrop.detectNow(),
    clearCrop: () => void autoCrop.clear(),
    fitWindowToVideo: () => void onFitWindowToVideo(),
    stepWindowSize: (factor) => void stepWindowSize(factor),
    openCurves,
    toggleSplitCompare: () => void splitCompare.toggle(),
    viewOriginal: () => enhancementHistory.toggleBypass(),
    isViewingOriginal: () => enhancementHistory.bypass.value,
    undoEnhancement: () => void enhancementHistory.undo(),
    redoEnhancement: () => void enhancementHistory.redo(),
    canUndoEnhancement: () => enhancementHistory.canUndo.value,
    canRedoEnhancement: () => enhancementHistory.canRedo.value,
    syncSubtitlesByEar: () => void subtitleSync.syncNow(),
    nextScene: () => void sceneIndex.next(player.state.playback.currentTime),
    previousScene: () => void sceneIndex.previous(player.state.playback.currentTime),
    rescanScenes: () => void sceneIndex.scan(true),
    sceneCount: () => sceneIndex.markers.value.length,
    scenesAreChapters: () => sceneIndex.fromChapters.value,
    autoEnhance: () => void onAutoEnhance(),
    resetVideoSettings: () => void onResetVideoSettings(),
    takeScreenshot: (withSubtitles) => void takeScreenshotShortcut(withSubtitles),

    addExternalSubtitle: () => void tracks.addExternalSubtitleTrack(),
    findOnlineSubtitles: () => {
        const url = player.state.media.url.trim();
        if (!url) return;
        void tracks.searchOnlineSubtitleTracks(
            url,
            player.state.media.title || undefined,
        );
    },
    generateAiSubtitles: onGenerateAiSubtitles,
    translateAiSubtitles: onTranslateAiSubtitles,
    syncAiSubtitles: onSyncAiSubtitles,
    describeClip: onDescribeClip,
    toggleSubtitleVisibility: () => void toggleSubtitleVisibility(),
    setDualSubEnabled: (enabled) => void tracks.setDualSubEnabled(enabled),
    adjustSubtitleDelay: (delta) => void adjustSubtitleDelay(delta),
    resetSubtitleDelay: () =>
        void adjustments.setSubDelayForTarget({ target: "primary", value: 0 }),
    openSubtitleAdvanced: () => {
        tracks.showSubMenu.value = true;
        tracks.showSubtitleAdvancedSettings.value = true;
    },

    openSettings,
    toggleShortcutsHelp: () => {
        isShortcutsHelpOpen.value = !isShortcutsHelpOpen.value;
    },
    toggleInfo: () => void toggleInfo(),
    regenerateThumbnails: () => void onRegenerateThumbnails(),
    exportSettings: () => void onExportSettings(),
    importSettings: () => void onImportSettings(),

    togglePlaylist: () => void togglePlaylist(),
    toggleFavorite: () => void onToggleFavorite(),
    showProgress: showProgressOverlay,
    hideMenuBar: () => setMenuBarVisible(false),

    checkForUpdates: () => {
        openSettings();
        showMessageOverlay("Check for updates in Settings → About");
    },
    showAbout: () => {
        openSettings();
        showMessageOverlay("Settings → About");
    },
});

// Escape dismisses one layer at a time, topmost first: modal dialogs, then the
// context menu, then the settings modal and the right-edge panels, then the
// control-bar dropdowns, the playlist drawer and finally the info overlay.
// Returns false when nothing was open, so the caller can fall through to
// "exit fullscreen". Keep this ordered outermost-last.
const closeTopOverlay = (): boolean => {
    if (importPrompt.value) {
        resolveImportMode(null);
        return true;
    }
    if (settingsExportPrompt.value) {
        resolveSettingsExport(null);
        return true;
    }
    if (mergeOpen.value) {
        mergeOpen.value = false;
        return true;
    }
    if (splitOpen.value) {
        splitOpen.value = false;
        return true;
    }
    if (subtitleAiOpen.value) {
        subtitleAiOpen.value = false;
        return true;
    }
    if (subtitleTranslateOpen.value) {
        subtitleTranslateOpen.value = false;
        return true;
    }
    if (subtitleSyncOpen.value) {
        subtitleSyncOpen.value = false;
        return true;
    }
    if (describeClipOpen.value) {
        describeClipOpen.value = false;
        return true;
    }
    if (aiPromptOpen.value) {
        aiPromptOpen.value = false;
        return true;
    }
    if (isPaletteOpen.value) {
        isPaletteOpen.value = false;
        return true;
    }
    if (playlistCreationPrompt.isOpen.value) {
        playlistCreationPrompt.cancelPlaylistCreation();
        return true;
    }
    if (isUpdateNotePromptOpen.value) {
        closeUpdateNotePrompt();
        return true;
    }
    if (isClearConfirmOpen.value) {
        closeClearConfirm();
        return true;
    }
    if (tracks.isOnlineSubtitleDialogOpen.value) {
        tracks.closeOnlineSubtitleDialog();
        return true;
    }
    if (playbackContextMenu.isOpen.value) {
        playbackContextMenu.close();
        return true;
    }
    if (isSettingsOpen.value) {
        closeSettings();
        return true;
    }
    if (isCurvesOpen.value) {
        isCurvesOpen.value = false;
        return true;
    }
    if (isAudioPanelOpen.value) {
        isAudioPanelOpen.value = false;
        return true;
    }
    if (
        tracks.showAudioMenu.value ||
        tracks.showSubMenu.value ||
        speed.showSpeedMenu.value ||
        adjustments.showSettingsMenu.value
    ) {
        hideAllMenus();
        return true;
    }
    if (isPlaylistOpen.value) {
        closePlaylist();
        return true;
    }
    if (ytWatch.closeDrawer()) {
        return true;
    }
    if (mainPanelsRef.value?.closeYoutubeBrowseView()) {
        return true;
    }
    if (isInfoOpen.value) {
        isInfoOpen.value = false;
        return true;
    }
    if (skipMarkers.promptKind.value) {
        skipMarkers.dismissPrompt();
        return true;
    }
    if (abRange.isActive.value) {
        void abRange.clear();
        return true;
    }
    return false;
};

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
        onSourceError: () => retryYoutubeWithFreshStream(),
    });

const onFileLoaded = async () => {
    await onFileLoadedBase();
    if (subtitlesDisabled.value) {
        await tracks.setSubtitlesDisabled(true);
    }
    await adjustments.applyColorAdjustmentsForMedia(player.state.media.url);
    await subtitleAppearance.applySubtitleAppearanceOptions();
    void enhancements.onFileLoaded(currentMediaKey());
    void audio.onFileLoaded();
    await geometry.applyAspectForMedia(currentMediaKey());
    await transform.applyForMedia(currentMediaKey());
    void abRange.onFileLoaded();
    skipMarkers.onFileLoaded(player.state.media.url);
    void splitCompare.onFileLoaded();
    sceneIndex.onFileLoaded();
    enhancementHistory.onFileLoaded(currentMediaKey());
    void applyWindowSizingForMedia();
    // Pre-render seek-bar thumbnails in the background for local files.
    if (player.state.playback.duration > 0) {
        void invoke("generate_seek_thumbnails", {
            path: player.state.media.url,
            duration: player.state.playback.duration,
        }).catch(() => {});
    }
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
    onKeyup,
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
            'cursor-pannable': transform.isZoomed.value && !isPointerOverUi,
            'has-menu-bar': isMenuBarShown,
        }"
        @mousedown.capture="onAppMouseDownCapture"
        @touchstart.capture="onAppTouchStartCapture"
        @pointerdown="onVideoPanPointerDown"
        @wheel="onVideoWheel"
        @contextmenu="playbackContextMenu.onContextMenu"
    >
        <SideActionsNav
            v-show="!player.state.media.isFileLoaded || isPointerNearLeft"
            :is-playback-active="isPlaybackActive"
            :active-panel="activePanel"
            :nav-active-panel="sideNavActivePanel"
            @navigate="onSideNavNavigate"
        />

        <transition name="fade">
            <MenuBar
                v-show="isMenuBarShown"
                :menus="appMenus"
                :show-window-controls="playerHeaderCompactModeEnabled"
                @open="() => void refreshClipExportAvailability()"
            />
        </transition>

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
            :menu-bar-visible="isMenuBarShown"
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

        <YtDownloadDialog
            :open="isPlayerDownloadDialogOpen"
            :item="playerDownloadItem"
            :queue-ahead="youtubeDownloads.activeCount.value"
            @close="isPlayerDownloadDialogOpen = false"
            @confirm="onPlayerDownloadConfirm"
        />

        <YtSponsorToast
            :toast="ytWatch.sponsorToast.value"
            @undo="ytWatch.undoSponsorSkip()"
            @dismiss="ytWatch.hideSponsorToast()"
        />

        <YtPlayerDrawer
            :open="ytWatch.isDrawerOpen.value"
            :active-tab="ytWatch.activeTab.value"
            :related="ytWatch.related.value"
            :chapters="ytWatch.chapters.value"
            :is-loading="ytWatch.isLoadingContext.value"
            :autoplay-next="ytWatch.autoplayNext.value"
            :current-time="player.state.playback.currentTime"
            @close="ytWatch.isDrawerOpen.value = false"
            @set-tab="ytWatch.activeTab.value = $event"
            @set-autoplay="ytWatch.setAutoplayNext"
            @play="onPlayYoutube"
            @seek="onSeek"
        />

        <MainPanels
            ref="mainPanelsRef"
            v-show="!player.state.media.isFileLoaded"
            :is-file-loaded="player.state.media.isFileLoaded"
            :hover="ui.hoverFilePicker.value"
            :history="history.history.value"
            :history-ready="history.isReady.value"
            :hide-history="hideHistory"
            :favorites="favorites"
            :favorite-folders="playlistState.favoriteFolderList.value"
            :favorites-by-folder="playlistState.favoritesByFolder.value"
            :favorite-folder-counts="playlistState.favoriteFolderCounts.value"
            :active-favorite-folder-id="playlistState.activeFavoriteFolderId.value"
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
            @select-favorite-folder="onSelectFavoriteFolder"
            @create-favorite-folder="onCreateFavoriteFolder"
            @rename-favorite-folder="onRenameFavoriteFolder"
            @delete-favorite-folder="onDeleteFavoriteFolder"
            @move-favorite-to-folder="onMoveFavoriteToFolder"
            @move-many-favorites="onMoveManyFavorites"
            @remove-many-favorites="onRemoveManyFavorites"
            @export-favorites="onExportFavorites"
            @import-favorites="onImportFavorites"
            @play-youtube="onPlayYoutube"
            @youtube-notify="(message) => showMessageOverlay(message, 2600)"
            @toggle-youtube-favorite="onToggleYoutubeFavorite"
            @open-youtube-settings="openSettings"
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

        <MergeDialog
            :open="mergeOpen"
            @close="mergeOpen = false"
            @notify="(msg: string) => showMessageOverlay(msg, 4000)"
            @load="(path: string) => void playPath(path)"
        />

        <SplitDialog
            :open="splitOpen"
            :current-path="player.state.media.url"
            :current-duration="player.state.playback.duration"
            :current-position="player.state.playback.currentTime"
            @close="splitOpen = false"
            @notify="(msg: string) => showMessageOverlay(msg, 4000)"
        />

        <SubtitleAiDialog
            :open="subtitleAiOpen"
            :path="player.state.media.url"
            :ab-start="abRange.pointA.value"
            :ab-end="abRange.pointB.value"
            :duration="player.state.playback.duration"
            @close="subtitleAiOpen = false"
            @notify="(msg: string) => showMessageOverlay(msg, 4000)"
            @loaded="onAiSubtitlesLoaded"
        />

        <SubtitleTranslateDialog
            :open="subtitleTranslateOpen"
            @close="subtitleTranslateOpen = false"
            @notify="(msg: string) => showMessageOverlay(msg, 4000)"
            @loaded="onAiSubtitlesLoaded"
        />

        <SubtitleSyncDialog
            :open="subtitleSyncOpen"
            :path="player.state.media.url"
            @close="subtitleSyncOpen = false"
            @notify="(msg: string) => showMessageOverlay(msg, 4000)"
            @loaded="onAiSubtitlesLoaded"
        />

        <ClipDescribeDialog
            :open="describeClipOpen"
            :path="player.state.media.url"
            :start="describeClipStart"
            :end="describeClipEnd"
            @close="describeClipOpen = false"
            @notify="(msg: string) => showMessageOverlay(msg, 3000)"
        />

        <!-- Import mode chooser (favourites / settings) -->
        <transition name="fade-in">
            <div
                v-if="importPrompt"
                class="io-prompt"
                @keydown.esc.stop.prevent="resolveImportMode(null)"
            >
                <div
                    class="io-prompt__backdrop"
                    @click="resolveImportMode(null)"
                />
                <div class="io-prompt__box" role="dialog">
                    <div class="io-prompt__title">
                        Import {{ importPrompt.kind }}
                    </div>
                    <p class="io-prompt__hint">
                        Merge with your existing {{ importPrompt.kind }}, or
                        replace them entirely?
                    </p>
                    <div class="io-prompt__actions">
                        <button
                            class="io-prompt__btn"
                            type="button"
                            @click="resolveImportMode(null)"
                        >
                            Cancel
                        </button>
                        <button
                            class="io-prompt__btn"
                            type="button"
                            @click="resolveImportMode('merge')"
                        >
                            Merge
                        </button>
                        <button
                            class="io-prompt__btn io-prompt__btn--danger"
                            type="button"
                            @click="resolveImportMode('replace')"
                        >
                            Replace
                        </button>
                    </div>
                </div>
            </div>
        </transition>

        <!-- Settings export: include API keys? -->
        <transition name="fade-in">
            <div
                v-if="settingsExportPrompt"
                class="io-prompt"
                @keydown.esc.stop.prevent="resolveSettingsExport(null)"
            >
                <div
                    class="io-prompt__backdrop"
                    @click="resolveSettingsExport(null)"
                />
                <div class="io-prompt__box" role="dialog">
                    <div class="io-prompt__title">Export settings</div>
                    <p class="io-prompt__hint">
                        Include your AI API keys in the file? Keys are stored in
                        plain text — only include them for a personal backup you
                        keep private.
                    </p>
                    <div class="io-prompt__actions">
                        <button
                            class="io-prompt__btn"
                            type="button"
                            @click="resolveSettingsExport(null)"
                        >
                            Cancel
                        </button>
                        <button
                            class="io-prompt__btn"
                            type="button"
                            @click="resolveSettingsExport({ includeKeys: false })"
                        >
                            Without keys
                        </button>
                        <button
                            class="io-prompt__btn io-prompt__btn--primary"
                            type="button"
                            @click="resolveSettingsExport({ includeKeys: true })"
                        >
                            Include keys
                        </button>
                    </div>
                </div>
            </div>
        </transition>

        <PlayerControls
            :is-playing="player.state.playback.isPlaying"
            :current-time="player.state.playback.currentTime"
            :duration="player.state.playback.duration"
            :youtube-quality-label="youtubeQualityLabel"
            @set-youtube-quality="onSetYoutubeQuality"
            @toggle-youtube-drawer="ytWatch.toggleDrawer()"
            @download-youtube="onDownloadCurrentYoutube"
            :media-path="player.state.media.url"
            :thumb-reload-token="thumbReloadToken"
            :ab-point-a="abRange.pointA.value"
            :ab-point-b="abRange.pointB.value"
            :scene-marks="sceneIndex.markers.value.map((m) => m.start)"
            :sponsor-segments="ytWatch.sponsorSegments.value"
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
            :video-presets="videoPresets"
            :slomo-factor="ultraSlomo.factor.value"
            :transform="transform"
            :is-loop-one="isLoopOne"
            :audio-tracks="tracks.audioTracks.value"
            :show-audio-menu="tracks.showAudioMenu.value"
            :audio-enhance-active="audio.isActive.value"
            :ai-enhance-busy="isAiEnhancing"
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
            @set-global-color-adjustments-enabled="onSetGlobalColorAdjustments"
            @auto-enhance="onAutoEnhance"
            @ai-enhance="onAiEnhance"
            @reset-video-settings="onResetVideoSettings"
            @set-slomo-factor="ultraSlomo.setFactor"
            @open-curves="openCurves"
            @open-audio-panel="openAudioPanel"
            @select-audio="tracks.selectAudio"
            @select-sub-track="tracks.selectSubTrack"
            @set-active-sub-target="tracks.setActiveSubTarget"
            @toggle-dual-sub="tracks.setDualSubEnabled"
            @add-external-audio="tracks.addExternalAudioTrack"
            @add-external-sub="tracks.addExternalSubtitleTrack"
            @remove-sub="tracks.removeSubtitleTrack"
            @find-online-sub="player.state.media.url.trim() && tracks.searchOnlineSubtitleTracks(player.state.media.url, player.state.media.title || undefined)"
            @toggle-fullscreen="onToggleFullscreen"
            @update:show-subtitle-advanced-settings="tracks.showSubtitleAdvancedSettings.value = $event"
        >
            <template #above-seek>
                <ClipBar
                    :ab="abRange"
                    :skip="skipMarkers"
                    :format-time="player.formatTime"
                    :exporting="isExportingClip"
                    :gif-max-seconds="GIF_MAX_SECONDS"
                    :can-export="isLocalMediaPath"
                    :clip-available="isClipExportAvailable"
                    @export="onExportClip"
                    @describe="onDescribeClip"
                />
            </template>
        </PlayerControls>

        <PlaylistPeekButton
            v-show="
                !isPlaylistOpen &&
                !isCurvesOpen &&
                !isAudioPanelOpen &&
                !isSettingsOpen
            "
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

        <CurvesPanel
            :enhancements="enhancements"
            :visible="isCurvesOpen"
            :media-path="player.state.media.url"
            :duration="player.state.playback.duration"
            :ai-busy="isAiEnhancing"
            @close="isCurvesOpen = false"
            @request-ai="onAiEnhance"
        />

        <EqualizerPanel
            :audio="audio"
            :visible="isAudioPanelOpen"
            @close="isAudioPanelOpen = false"
        />

        <SettingsPanel
            :visible="isSettingsOpen"
            :commands="commandRegistry.commands.value"
            :custom-shortcuts="customShortcuts"
            @close="closeSettings"
        />

        <SkipPrompt :skip="skipMarkers" />

        <SplitCompareOverlay :compare="splitCompare" />

        <CommandPalette
            :open="isPaletteOpen"
            :commands="commandRegistry.commands.value"
            :chord-for="chordForCommand"
            @close="isPaletteOpen = false"
        />

        <transition name="fade">
            <div
                v-if="aiPromptOpen"
                class="ai-prompt"
                data-window-no-drag
                @keydown.esc.stop.prevent="aiPromptOpen = false"
            >
                <div
                    class="ai-prompt__backdrop"
                    @click="aiPromptOpen = false"
                ></div>
                <div class="ai-prompt__box" role="dialog" aria-label="AI correction">
                    <div class="ai-prompt__title">AI Correction</div>

                    <div class="ai-prompt__grid">
                        <span class="ai-prompt__field-label">Provider</span>
                        <select
                            :value="aiConfig.provider.value"
                            class="ai-prompt__select"
                            @change="
                                aiConfig.setProvider(
                                    ($event.target as HTMLSelectElement).value,
                                )
                            "
                        >
                            <option v-for="p in AI_PROVIDERS" :key="p" :value="p">
                                {{ p }}
                            </option>
                        </select>

                        <span class="ai-prompt__field-label">Model</span>
                        <div class="ai-prompt__model">
                            <select
                                :value="aiConfig.currentModel.value"
                                class="ai-prompt__select"
                                @change="
                                    aiConfig.setModel(
                                        ($event.target as HTMLSelectElement).value,
                                    )
                                "
                            >
                                <option
                                    v-for="m in aiConfig.modelOptions.value"
                                    :key="m"
                                    :value="m"
                                >
                                    {{ m }}
                                </option>
                            </select>
                            <button
                                class="ai-prompt__fetch"
                                type="button"
                                :disabled="aiFetching"
                                title="Fetch this provider's latest models"
                                @click="onPromptFetchModels"
                            >
                                {{ aiFetching ? "…" : "↻" }}
                            </button>
                        </div>
                    </div>

                    <p
                        v-if="!aiConfig.currentKey.value"
                        class="ai-prompt__warn"
                    >
                        No API key set for {{ aiConfig.provider.value }} — add one
                        in Settings → Advanced → AI Enhance.
                    </p>
                    <p v-else-if="aiFetchStatus" class="ai-prompt__status">
                        {{ aiFetchStatus }}
                    </p>

                    <p class="ai-prompt__hint">
                        Describe the correction you want, or leave blank for a
                        general best-effort pass.
                    </p>
                    <textarea
                        ref="aiPromptInput"
                        v-model="aiPromptText"
                        class="ai-prompt__input"
                        rows="3"
                        placeholder="e.g. warmer, slightly less saturation, lift the shadows"
                        @keydown.enter.exact.prevent="runAiEnhance"
                        @keydown.stop
                    ></textarea>

                    <div
                        v-if="aiConfig.promptHistory.value.length"
                        class="ai-prompt__grid"
                    >
                        <span class="ai-prompt__field-label">Recent</span>
                        <select
                            class="ai-prompt__select"
                            :value="''"
                            @change="onSelectRecent(($event.target as HTMLSelectElement).value)"
                        >
                            <option value="" disabled>
                                Reuse a previous prompt…
                            </option>
                            <option
                                v-for="(entry, i) in aiConfig.promptHistory.value"
                                :key="i"
                                :value="String(i)"
                            >
                                {{ (entry.prompt || "General best-effort pass") }}
                                — {{ entry.model || entry.provider }}
                            </option>
                        </select>
                    </div>

                    <div class="ai-prompt__refs">
                        <div class="ai-prompt__refs-head">
                            <span class="ai-prompt__field-label">Reference</span>
                            <button
                                class="ai-prompt__ref-add"
                                type="button"
                                :disabled="aiRefImages.length >= MAX_REFERENCE_IMAGES"
                                @click="onAddReferenceImages"
                            >
                                + Add image
                            </button>
                        </div>
                        <div v-if="aiRefImages.length" class="ai-prompt__ref-chips">
                            <div
                                v-for="(refImg, i) in aiRefImages"
                                :key="refImg.path"
                                class="ai-prompt__ref-chip"
                                :title="refImg.path"
                            >
                                <img :src="refImg.thumb" alt="reference" />
                                <button
                                    class="ai-prompt__ref-remove"
                                    type="button"
                                    aria-label="Remove reference image"
                                    @click="removeReferenceImage(i)"
                                >
                                    ×
                                </button>
                            </div>
                        </div>
                        <p v-else class="ai-prompt__ref-empty">
                            Optional — add up to {{ MAX_REFERENCE_IMAGES }} stills
                            of a look you want, and the AI grades the video toward
                            them.
                        </p>
                    </div>

                    <div class="ai-prompt__actions">
                        <button
                            class="ai-prompt__btn"
                            type="button"
                            @click="aiPromptOpen = false"
                        >
                            Cancel
                        </button>
                        <button
                            class="ai-prompt__btn ai-prompt__btn--primary"
                            type="button"
                            :disabled="!aiConfig.currentKey.value"
                            @click="runAiEnhance"
                        >
                            Enhance
                        </button>
                    </div>
                </div>
            </div>
        </transition>

        <transition name="slomo-badge-fade">
            <div v-if="ultraSlomo.isActive.value" class="slomo-badge">
                <span class="slomo-badge__dot"></span>
                {{ ultraSlomo.label.value }}
            </div>
        </transition>

        <transition name="slomo-badge-fade">
            <div v-if="enhancementHistory.bypass.value" class="original-badge">
                <span class="original-badge__dot"></span>
                ORIGINAL
            </div>
        </transition>

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

/* Ultra Slo-Mo active badge */
.slomo-badge {
    position: fixed;
    top: 54px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 118;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 14px;
    border-radius: 999px;
    background: rgba(10, 12, 16, 0.72);
    border: 1px solid rgba(143, 179, 255, 0.5);
    color: #eaf0ff;
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.06em;
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
    pointer-events: none;
}

.slomo-badge__dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #8fb3ff;
    box-shadow: 0 0 8px #8fb3ff;
    animation: slomo-pulse 1.1s ease-in-out infinite;
}

/* View Original bypass badge */
/* AI correction prompt modal */
.io-prompt {
    position: fixed;
    inset: 0;
    z-index: 220;
    display: flex;
    align-items: center;
    justify-content: center;
}
.io-prompt__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}
.io-prompt__box {
    position: relative;
    width: min(440px, 92vw);
    padding: 20px;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    color: #fff;
}
.io-prompt__title {
    font-size: 15px;
    font-weight: 700;
    text-transform: capitalize;
}
.io-prompt__hint {
    margin: 8px 0 16px;
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.6);
}
.io-prompt__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
}
.io-prompt__btn {
    padding: 8px 16px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}
.io-prompt__btn:hover {
    background: rgba(255, 255, 255, 0.12);
}
.io-prompt__btn--primary {
    border-color: rgba(196, 160, 255, 0.5);
    background: rgba(170, 130, 255, 0.28);
}
.io-prompt__btn--primary:hover {
    background: rgba(170, 130, 255, 0.42);
}
.io-prompt__btn--danger {
    border-color: rgba(220, 90, 90, 0.5);
    background: rgba(220, 70, 70, 0.24);
}
.io-prompt__btn--danger:hover {
    background: rgba(220, 70, 70, 0.4);
}

.ai-prompt {
    position: fixed;
    inset: 0;
    z-index: 210;
    display: flex;
    align-items: center;
    justify-content: center;
}

.ai-prompt__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}

.ai-prompt__box {
    position: relative;
    width: min(520px, 92vw);
    padding: 20px;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    color: #fff;
}

.ai-prompt__title {
    font-size: 15px;
    font-weight: 700;
}

.ai-prompt__grid {
    display: grid;
    grid-template-columns: 72px 1fr;
    align-items: center;
    gap: 8px 12px;
    margin: 12px 0 8px;
}

.ai-prompt__field-label {
    font-size: 13px;
    font-weight: 600;
}

.ai-prompt__select {
    width: 100%;
    padding: 8px 10px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.28);
    color: #fff;
    font-size: 13px;
    color-scheme: dark;
}

.ai-prompt__select option {
    background-color: #1a1c21;
    color: #f2f2f2;
}

.ai-prompt__model {
    display: flex;
    gap: 6px;
}

.ai-prompt__fetch {
    flex: none;
    width: 38px;
    border: 1px solid rgba(196, 160, 255, 0.45);
    border-radius: 8px;
    background: rgba(170, 130, 255, 0.16);
    color: #e7dcff;
    font-size: 15px;
    cursor: pointer;
}

.ai-prompt__fetch:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.3);
}

.ai-prompt__fetch:disabled {
    opacity: 0.55;
    cursor: default;
}

.ai-prompt__status {
    margin: 0 0 6px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.55);
}

.ai-prompt__warn {
    margin: 0 0 6px;
    font-size: 12px;
    color: #ffb454;
}

.ai-prompt__btn--primary:disabled {
    opacity: 0.5;
    cursor: default;
}

.ai-prompt__hint {
    margin: 6px 0 12px;
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.55);
}

.ai-prompt__input {
    width: 100%;
    box-sizing: border-box;
    padding: 10px 12px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 9px;
    background: rgba(0, 0, 0, 0.28);
    color: #fff;
    font-size: 13.5px;
    font-family: inherit;
    resize: vertical;
}

.ai-prompt__refs {
    margin-top: 12px;
}

.ai-prompt__refs-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
}

.ai-prompt__ref-add {
    padding: 5px 12px;
    border: 1px solid rgba(196, 160, 255, 0.45);
    border-radius: 7px;
    background: rgba(170, 130, 255, 0.16);
    color: #e7dcff;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}

.ai-prompt__ref-add:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.3);
}

.ai-prompt__ref-add:disabled {
    opacity: 0.45;
    cursor: default;
}

.ai-prompt__ref-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
}

.ai-prompt__ref-chip {
    position: relative;
    width: 68px;
    height: 68px;
    border-radius: 8px;
    overflow: hidden;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: rgba(0, 0, 0, 0.3);
}

.ai-prompt__ref-chip img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
}

.ai-prompt__ref-remove {
    position: absolute;
    top: 2px;
    right: 2px;
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.65);
    color: #fff;
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
}

.ai-prompt__ref-remove:hover {
    background: rgba(220, 60, 60, 0.9);
}

.ai-prompt__ref-empty {
    margin: 8px 0 0;
    font-size: 12px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.5);
}

.ai-prompt__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
}

.ai-prompt__btn {
    padding: 8px 16px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}

.ai-prompt__btn:hover {
    background: rgba(255, 255, 255, 0.13);
}

.ai-prompt__btn--primary {
    background: rgba(170, 130, 255, 0.9);
    border-color: transparent;
    color: #14101f;
}

.ai-prompt__btn--primary:hover {
    background: #b79bff;
}

.original-badge {
    position: fixed;
    top: 54px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 118;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 14px;
    border-radius: 999px;
    background: rgba(10, 12, 16, 0.72);
    border: 1px solid rgba(255, 255, 255, 0.4);
    color: #fff;
    font-size: 12.5px;
    font-weight: 800;
    letter-spacing: 0.1em;
    -webkit-backdrop-filter: blur(10px);
    backdrop-filter: blur(10px);
    pointer-events: none;
}

.original-badge__dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #fff;
}

@keyframes slomo-pulse {
    0%,
    100% {
        opacity: 1;
    }
    50% {
        opacity: 0.35;
    }
}

.slomo-badge-fade-enter-active,
.slomo-badge-fade-leave-active {
    transition: opacity 0.18s ease;
}

.slomo-badge-fade-enter-from,
.slomo-badge-fade-leave-to {
    opacity: 0;
}
</style>
