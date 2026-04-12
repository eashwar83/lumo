<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import PlayerControls from "./components/PlayerControls.vue";
import PlayerHeader from "./components/PlayerHeader.vue";
import MainPanels from "./components/MainPanels.vue";
import SideActionsNav from "./components/SideActionsNav.vue";
import PlaybackOverlays from "./components/PlaybackOverlays.vue";
import PlaylistPeekButton from "./components/PlaylistPeekButton.vue";
import PlaylistDrawer from "./components/PlaylistDrawer.vue";
import ConfirmDialog from "./components/ConfirmDialog.vue";
import { usePlaybackShortcuts } from "./composables/usePlaybackShortcuts";
import { usePlaybackFlow } from "./composables/usePlaybackFlow";
import { useAppUiPersistence } from "./composables/useAppUiPersistence";
import { useAppRuntimeBindings } from "./composables/useAppRuntimeBindings";
import { useAppPlaybackEvents } from "./composables/useAppPlaybackEvents";
import { useAppUiActions } from "./composables/useAppUiActions";
import { useAppBootstrap } from "./composables/useAppBootstrap";
import { getMockMediaInfo } from "./mock/mediaInfo";
import { applyThemeFromSettingGroups } from "./constants/theme";
import { MEDIA_FILE_EXTENSIONS } from "./constants/media";
import { normalizePlaybackKey } from "./utils/playbackSource";
import type { PlaylistScrollState } from "./types/playlist";

const {
    isMacOS,
    player,
    tracks,
    speed,
    adjustments,
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

const playbackFlow = usePlaybackFlow({
    isMacOS,
    player,
    tracks,
    history,
    playlistState,
    nowPlaying,
    hideAllMenus,
    isInfoOpen,
    onPlaybackIntent: () => {
        clearNavSelectionDuringLoad.value = true;
    },
});

const {
    isLoading,
    loadingUrl,
    pendingResume,
    hideHistory,
    isLoadingForCurrentUrl,
    playPath,
    onLoadFile,
    onPlayHistory,
    onPlayNetwork,
    onUpdateUrl,
    onStopPlayback,
    requestOpenFilePicker,
    openSelectedPaths,
    playbackTitleMode,
    compactModeEnabled,
    wallpaperModeEnabled,
} = playbackFlow;

const isWindowsPlatform =
    typeof navigator !== "undefined" && /\bwindows\b/i.test(navigator.userAgent);
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
const shouldShowPlaybackLoadingOverlay = computed(
    () =>
        isLoading.value ||
        (player.state.media.isFileLoaded && player.state.playback.isBuffering),
);
const loadingDownloadSpeedBps = computed(() => {
    const speed = player.state.playback.downloadSpeedBps;
    if (!Number.isFinite(speed) || speed <= 0) return null;
    return speed;
});
const isLoadingOverlayVisible = ref(false);
let loadingOverlayDelayTimer: ReturnType<typeof setTimeout> | null = null;
const seekOverlayLeftText = ref("");
const seekOverlayRightText = ref("");
const seekOverlayLeftTimelineText = ref("");
const seekOverlayLeftPulseToken = ref(0);
const seekOverlayRightPulseToken = ref(0);
let seekOverlayTimer: ReturnType<typeof setTimeout> | null = null;
let seekOverlayAccumulatedDelta = 0;
let seekOverlayBaseTime = 0;

const clearLoadingOverlayDelayTimer = () => {
    if (loadingOverlayDelayTimer !== null) {
        window.clearTimeout(loadingOverlayDelayTimer);
        loadingOverlayDelayTimer = null;
    }
};

const clearSeekOverlayTimer = () => {
    if (seekOverlayTimer !== null) {
        window.clearTimeout(seekOverlayTimer);
        seekOverlayTimer = null;
    }
};

const formatSeekDeltaSeconds = (delta: number) => {
    const seconds = Math.abs(delta);
    const rounded = Math.round(seconds * 10) / 10;
    return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1);
};

const clampNumber = (value: number, min: number, max: number) =>
    Math.min(Math.max(value, min), max);

const buildSeekTimelineText = (targetTime: number, duration: number) => {
    const safeDuration =
        Number.isFinite(duration) && duration > 0 ? duration : 0;
    const safeTarget = clampNumber(targetTime, 0, safeDuration);
    return `${player.formatTime(safeTarget)} / ${player.formatTime(safeDuration)}`;
};

const showSeekOverlay = (delta: number) => {
    const hasActiveOverlay = seekOverlayTimer !== null;
    const hasSameDirection =
        seekOverlayAccumulatedDelta === 0 ||
        Math.sign(seekOverlayAccumulatedDelta) === Math.sign(delta);
    if (!(hasActiveOverlay && hasSameDirection)) {
        seekOverlayBaseTime = player.state.playback.currentTime;
    }
    seekOverlayAccumulatedDelta =
        hasActiveOverlay && hasSameDirection
            ? seekOverlayAccumulatedDelta + delta
            : delta;
    clearSeekOverlayTimer();
    seekOverlayLeftText.value = "";
    seekOverlayRightText.value = "";
    seekOverlayLeftTimelineText.value = "";
    
    const seekTargetTime = seekOverlayBaseTime + seekOverlayAccumulatedDelta;
    const timelineText = buildSeekTimelineText(
        seekTargetTime,
        player.state.playback.duration,
    );
    if (seekOverlayAccumulatedDelta < 0) {
        seekOverlayLeftText.value = `- ${formatSeekDeltaSeconds(
            seekOverlayAccumulatedDelta,
        )}`;
        seekOverlayLeftTimelineText.value = timelineText;
        seekOverlayLeftPulseToken.value += 1;
    } else if (seekOverlayAccumulatedDelta > 0) {
        seekOverlayRightText.value = `+ ${formatSeekDeltaSeconds(
            seekOverlayAccumulatedDelta,
        )}`;
        seekOverlayLeftTimelineText.value = timelineText;
        seekOverlayRightPulseToken.value += 1;
    }
    seekOverlayTimer = window.setTimeout(() => {
        seekOverlayLeftText.value = "";
        seekOverlayRightText.value = "";
        seekOverlayLeftTimelineText.value = "";
        seekOverlayTimer = null;
        seekOverlayAccumulatedDelta = 0;
        seekOverlayBaseTime = 0;
    }, 700);
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
    onStopPlayback,
    playPath,
});

const onSideNavNavigate = async (
    panel: "home" | "history" | "network" | "settings",
) => {
    clearNavSelectionDuringLoad.value = false;
    await onNavAction(panel);
};

const beginSeekLoading = () => {
    isLoading.value = true;
    loadingUrl.value = player.state.media.url;
    player.state.playback.downloadSpeedBps = 0;
};

const onSeek = async (position: number) => {
    beginSeekLoading();
    try {
        await player.seek(position);
    } catch {
        isLoading.value = false;
        loadingUrl.value = "";
    }
};

const onSeekRelative = async (position: number) => {
    beginSeekLoading();
    try {
        await player.seekRelative(position);
    } catch {
        isLoading.value = false;
        loadingUrl.value = "";
    }
};

const { onKeydown, onDoubleClick } = usePlaybackShortcuts(
    {
        state: player.state,
        togglePlayPause: player.togglePlayPause,
        seekRelative: onSeekRelative,
    },
    onToggleFullscreen,
    toggleInfo,
    showSeekOverlay,
);

const DRAG_THRESHOLD_PX = 4;
const INTERACTIVE_TARGET_SELECTOR = [
    "button",
    "input",
    "textarea",
    "select",
    "option",
    "a[href]",
    "summary",
    "[role='button']",
    "[role='link']",
    "[role='checkbox']",
    "[role='radio']",
    "[role='switch']",
    "[role='menuitem']",
    "[contenteditable='true']",
    "[tabindex]:not([tabindex='-1'])",
    "[data-window-no-drag]",
].join(", ");

let dragStartX = 0;
let dragStartY = 0;
let isDragPending = false;
let unlistenAppOpenFiles: UnlistenFn | null = null;
let unlistenWindowDragDrop: UnlistenFn | null = null;
let unlistenOpenSettingsPanel: UnlistenFn | null = null;
let drainPendingOpenFilesRef: (() => Promise<void>) | null = null;
let mediaSizeQueryId = 0;
const mediaFileSizeBytes = ref<number | null>(null);
const MEDIA_EXTENSION_SET = new Set(
    MEDIA_FILE_EXTENSIONS.map((extension) => extension.toLowerCase()),
);
const playlistScrollState = ref<PlaylistScrollState>({
    list: 0,
    playlists: {},
});
const playlistDrawerWidthRatio = ref<number | null>(null);

const normalizePlaylistDrawerWidthRatio = (value: unknown): number | null => {
    if (typeof value !== "number" || !Number.isFinite(value)) return null;
    if (value <= 0) return null;
    const normalized = Math.min(value, 0.86);
    return Math.round(normalized * 10000) / 10000;
};

const onPlaylistScrollPositionChange = (
    playlistId: string | null,
    scrollTop: number,
) => {
    const nextScrollTop =
        Number.isFinite(scrollTop) && scrollTop > 0 ? Math.round(scrollTop) : 0;
    if (!playlistId) {
        if (playlistScrollState.value.list === nextScrollTop) return;
        playlistScrollState.value = {
            ...playlistScrollState.value,
            list: nextScrollTop,
        };
        return;
    }
    const current = playlistScrollState.value.playlists[playlistId] ?? 0;
    if (current === nextScrollTop) return;
    playlistScrollState.value = {
        ...playlistScrollState.value,
        playlists: {
            ...playlistScrollState.value.playlists,
            [playlistId]: nextScrollTop,
        },
    };
};

const onPlaylistDrawerWidthRatioChange = (ratio: number) => {
    const normalized = normalizePlaylistDrawerWidthRatio(ratio);
    if (playlistDrawerWidthRatio.value === normalized) return;
    playlistDrawerWidthRatio.value = normalized;
};

function extractPathExtension(path: string): string | null {
    const cleanPath = path.trim().split(/[?#]/, 1)[0];
    if (!cleanPath) return null;
    const lastDotIndex = cleanPath.lastIndexOf(".");
    const lastSeparatorIndex = Math.max(
        cleanPath.lastIndexOf("/"),
        cleanPath.lastIndexOf("\\"),
    );
    if (lastDotIndex <= lastSeparatorIndex) return null;
    return cleanPath.slice(lastDotIndex + 1).toLowerCase();
}

function filterDroppedMediaPaths(paths: string[]): string[] {
    const deduped = new Set<string>();
    paths.forEach((path) => {
        const trimmedPath = path.trim();
        if (!trimmedPath) return;
        const extension = extractPathExtension(trimmedPath);
        if (!extension || !MEDIA_EXTENSION_SET.has(extension)) return;
        deduped.add(trimmedPath);
    });
    return [...deduped];
}

function clearWindowDragCandidate() {
    isDragPending = false;
    window.removeEventListener("mousemove", onWindowDragMove);
    window.removeEventListener("mouseup", clearWindowDragCandidate);
}

function onWindowDragMove(event: MouseEvent) {
    if (!isDragPending) return;
    const movedX = Math.abs(event.clientX - dragStartX);
    const movedY = Math.abs(event.clientY - dragStartY);
    if (movedX < DRAG_THRESHOLD_PX && movedY < DRAG_THRESHOLD_PX) return;

    clearWindowDragCandidate();
    void getCurrentWindow().startDragging();
}

function onDragRegionMouseDown(event: MouseEvent) {
    if (event.button !== 0) return;
    if (event.detail > 1) return;

    dragStartX = event.clientX;
    dragStartY = event.clientY;
    isDragPending = true;
    window.addEventListener("mousemove", onWindowDragMove);
    window.addEventListener("mouseup", clearWindowDragCandidate, { once: true });
}

function isInteractiveTarget(target: EventTarget | null): boolean {
    if (!(target instanceof Element)) return false;
    return target.closest(INTERACTIVE_TARGET_SELECTOR) !== null;
}

function onAppMouseDownCapture(event: MouseEvent) {
    if (isInteractiveTarget(event.target)) return;
    onDragRegionMouseDown(event);
}

async function refreshMediaFileSize() {
    const queryId = ++mediaSizeQueryId;
    if (!player.state.media.isFileLoaded || !player.state.media.url.trim()) {
        mediaFileSizeBytes.value = null;
        return;
    }

    try {
        const fileSize = await invoke<number | null>("get_media_file_size", {
            path: player.state.media.url,
        });
        if (queryId !== mediaSizeQueryId) return;
        mediaFileSizeBytes.value =
            typeof fileSize === "number" && Number.isFinite(fileSize) && fileSize > 0
                ? fileSize
                : null;
    } catch {
        if (queryId !== mediaSizeQueryId) return;
        mediaFileSizeBytes.value = null;
    }
}

watch(
    () => [player.state.media.isFileLoaded, player.state.media.url] as const,
    () => {
        void refreshMediaFileSize();
    },
    { immediate: true },
);

watch(shouldShowPlaybackLoadingOverlay, (shouldShow) => {
    if (!shouldShow) {
        clearLoadingOverlayDelayTimer();
        isLoadingOverlayVisible.value = false;
        return;
    }
    if (isLoadingOverlayVisible.value) return;
    clearLoadingOverlayDelayTimer();
    loadingOverlayDelayTimer = window.setTimeout(() => {
        loadingOverlayDelayTimer = null;
        isLoadingOverlayVisible.value = true;
    }, 500);
});

const mediaInfo = computed(() => {
    if (!player.state.media.isFileLoaded) return null;
    return getMockMediaInfo(player.state.media.url, {
        durationSeconds: player.state.playback.duration,
        fileSizeBytes: mediaFileSizeBytes.value,
    });
});

const statusBadges = computed(() => mediaInfo.value?.badges ?? []);
const playlistEntriesWithProgress = computed(() => {
    const ratioByPath = new Map<string, number>();
    history.history.value.forEach((entry) => {
        const duration = entry.duration;
        const position = entry.lastPosition;
        if (!Number.isFinite(duration) || duration <= 0) {
            ratioByPath.set(normalizePlaybackKey(entry.path), 0);
            return;
        }
        const ratio = Math.max(0, Math.min(1, position / duration));
        ratioByPath.set(normalizePlaybackKey(entry.path), ratio);
    });

    return orderedPlaylist.value.map((entry) => ({
        ...entry,
        playedRatio: ratioByPath.get(normalizePlaybackKey(entry.path)) ?? 0,
    }));
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

const { onFileLoaded, onPlaybackRestart, onProgress, onEndFile } =
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
        playPath,
    });

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
    onProgress,
    onEndFile,
    nowPlaying,
    isInfoOpen,
    isPlaylistOpen,
    hideHistory,
    playerHeaderRef,
    closePlaylist,
    shouldKeepControlsVisible,
    schedulePointerRefresh,
});

onMounted(() => {
    applyThemeFromSettingGroups();
    void loadActivePanel();
    void (async () => {
        const drainPendingOpenFiles = async () => {
            const pending = await invoke<string[]>("consume_pending_open_files");
            if (Array.isArray(pending) && pending.length) {
                await openSelectedPaths(pending);
            }
        };
        drainPendingOpenFilesRef = drainPendingOpenFiles;

        try {
            await drainPendingOpenFiles();
        } catch {
            // Ignore if there are no startup-open files.
        }

        try {
            unlistenAppOpenFiles = await listen("app-open-files", () => {
                void (async () => {
                    try {
                        await drainPendingOpenFiles();
                    } catch {
                        // Ignore transient queue-drain errors.
                    }
                })();
                },
            );
        } catch {
            // Ignore event-listener failures in unsupported environments.
        }

        try {
            unlistenOpenSettingsPanel = await listen(
                "soia-open-settings-panel",
                () => {
                    if (player.state.media.isFileLoaded) return;
                    clearNavSelectionDuringLoad.value = false;
                    activePanel.value = "settings";
                    hideHistory.value = false;
                },
            );
        } catch {
            // Ignore event-listener failures in unsupported environments.
        }

        try {
            unlistenWindowDragDrop = await getCurrentWindow().onDragDropEvent(
                ({ payload }) => {
                    if (payload.type !== "drop") return;
                    const droppedPaths = filterDroppedMediaPaths(payload.paths);
                    if (!droppedPaths.length) return;
                    void openSelectedPaths(droppedPaths);
                },
            );
        } catch {
            // Ignore drag-drop binding failures in unsupported environments.
        }
    })();

    window.addEventListener("focus", onWindowFocusDrainPendingFiles);
});

onBeforeUnmount(() => {
    clearLoadingOverlayDelayTimer();
    clearSeekOverlayTimer();
    clearWindowDragCandidate();
    unlistenAppOpenFiles?.();
    unlistenAppOpenFiles = null;
    unlistenWindowDragDrop?.();
    unlistenWindowDragDrop = null;
    unlistenOpenSettingsPanel?.();
    unlistenOpenSettingsPanel = null;
    drainPendingOpenFilesRef = null;
    window.removeEventListener("focus", onWindowFocusDrainPendingFiles);
});

function onWindowFocusDrainPendingFiles() {
    if (!drainPendingOpenFilesRef) return;
    void (async () => {
        try {
            await drainPendingOpenFilesRef?.();
        } catch {
            // Ignore focus-triggered drain failures.
        }
    })();
}
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
            @url-input-mousedown="onDragRegionMouseDown"
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
            :mode="activePanel"
            @update:hover="ui.hoverFilePicker.value = $event"
            @open-file-picker="requestOpenFilePicker"
            @play-history="onPlayHistory"
            @play-network="onPlayNetwork"
            @clear-history="onClearHistory"
            @remove-history="onRemoveHistory"
            @toggle-pin-history="onTogglePinHistory"
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
            @back="onBackToPlaylists"
            @rename-playlist="onRenamePlaylist"
            @delete-playlist="onDeletePlaylist"
            @move-playlist="onMovePlaylist"
            @width-ratio-change="onPlaylistDrawerWidthRatioChange"
            @scroll-position-change="onPlaylistScrollPositionChange"
        />

        <PlayerControls
            :is-playing="player.state.playback.isPlaying"
            :current-time="player.state.playback.currentTime"
            :duration="player.state.playback.duration"
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
            :is-loop-one="isLoopOne"
            :audio-tracks="tracks.audioTracks.value"
            :show-audio-menu="tracks.showAudioMenu.value"
            :sub-tracks="tracks.subTracks.value"
            :dual-sub-enabled="tracks.dualSubEnabled.value"
            :secondary-sub-id="tracks.secondarySubId.value"
            :active-sub-target="tracks.activeSubTarget.value"
            :show-sub-menu="tracks.showSubMenu.value"
            :has-audio-tracks="hasAudioTracks"
            :has-sub-tracks="hasSubTracks"
            :is-fullscreen="player.state.window.isFullscreen"
            @seek="onSeek"
            @prev-track="onPrevTrack"
            @toggle-play-pause="player.togglePlayPause"
            @stop-playback="onStopPlayback"
            @next-track="onNextTrack"
            @toggle-menu="toggleMenu"
            @toggle-loop-one="toggleLoopOne"
            @set-speed="speed.setSpeed"
            @set-audio-delay="adjustments.setAudioDelay"
            @set-sub-delay-for-target="adjustments.setSubDelayForTarget"
            @set-brightness="adjustments.setBrightness"
            @set-contrast="adjustments.setContrast"
            @set-saturation="adjustments.setSaturation"
            @set-gamma="adjustments.setGamma"
            @set-hue="adjustments.setHue"
            @select-audio="tracks.selectAudio"
            @select-sub-track="tracks.selectSubTrack"
            @set-active-sub-target="tracks.setActiveSubTarget"
            @toggle-dual-sub="tracks.setDualSubEnabled"
            @add-external-audio="tracks.addExternalAudioTrack"
            @add-external-sub="tracks.addExternalSubtitleTrack"
            @toggle-fullscreen="onToggleFullscreen"
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
    </main>
</template>

<style src="./styles/app-theme.css"></style>
<style scoped src="./styles/app-shell.css"></style>
<style src="./styles/player.css"></style>
