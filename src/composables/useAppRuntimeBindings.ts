import { onMounted, onUnmounted, watch, type Ref } from "vue";
import { useAppEventBindings } from "./useAppEventBindings";

type UseAppEventBindingsOptions = Parameters<typeof useAppEventBindings>[0];
type RuntimeUiApi = UseAppEventBindingsOptions["ui"] & {
    stopInactivityTimer: () => void;
};

type NowPlayingApi = {
    clearNowPlaying: () => void;
    clearStatusOverlay: () => void;
    updateNowPlayingStatus: (position?: number) => void;
    triggerStatusOverlayFromPlayback: () => void;
    updateNowPlayingMetadata: () => void;
};

type UseAppRuntimeBindingsOptions = Omit<UseAppEventBindingsOptions, "ui"> & {
    ui: RuntimeUiApi;
    nowPlaying: NowPlayingApi;
    isInfoOpen: Ref<boolean>;
    isPlaylistOpen: Ref<boolean>;
    hideHistory: Ref<boolean>;
    playerHeaderRef: Ref<{ $el?: HTMLElement } | null>;
    closePlaylist: () => void;
    shouldKeepControlsVisible: () => boolean;
    schedulePointerRefresh: () => void;
};

export const useAppRuntimeBindings = ({
    player,
    tracks,
    ui,
    onFullscreenTransition,
    onFullscreenTransitionEnd,
    onCloseAllMenus,
    onKeydown,
    onDoubleClick,
    setWindowControlsVisible,
    onFileLoaded,
    onPlaybackRestart,
    onProgress,
    onEndFile,
    resolveMediaTitle,
    nowPlaying,
    isInfoOpen,
    isPlaylistOpen,
    hideHistory,
    playerHeaderRef,
    closePlaylist,
    shouldKeepControlsVisible,
    schedulePointerRefresh,
}: UseAppRuntimeBindingsOptions) => {
    useAppEventBindings({
        player,
        tracks,
        ui,
        onFullscreenTransition,
        onFullscreenTransitionEnd,
        onCloseAllMenus,
        onKeydown,
        onDoubleClick,
        setWindowControlsVisible,
        onFileLoaded,
        onPlaybackRestart,
        onProgress,
        onEndFile,
        resolveMediaTitle,
    });

    watch(
        () => ui.showControls.value,
        (visible) => {
            void setWindowControlsVisible(visible);
            if (!visible && !shouldKeepControlsVisible()) {
                isInfoOpen.value = false;
                closePlaylist();
            }
        },
    );

    watch(
        () => player.state.media.isFileLoaded,
        (loaded) => {
            if (!loaded) {
                isInfoOpen.value = false;
                nowPlaying.clearNowPlaying();
                ui.stopInactivityTimer();
                void setWindowControlsVisible(true);
                nowPlaying.clearStatusOverlay();
            }
            schedulePointerRefresh();
        },
    );

    watch(
        () => player.state.playback.isPlaying,
        () => {
            if (!player.state.media.isFileLoaded) return;
            nowPlaying.updateNowPlayingStatus(player.state.playback.currentTime);
            nowPlaying.triggerStatusOverlayFromPlayback();
        },
    );

    watch(
        () => player.state.playback.duration,
        () => {
            if (!player.state.media.isFileLoaded) return;
            nowPlaying.updateNowPlayingMetadata();
        },
    );

    watch([hideHistory, isPlaylistOpen], () => {
        schedulePointerRefresh();
    });

    const handleGlobalPointerDown = (event: MouseEvent) => {
        if (!isInfoOpen.value) return;
        const headerEl = playerHeaderRef.value?.$el as HTMLElement | undefined;
        if (headerEl && !headerEl.contains(event.target as Node)) {
            isInfoOpen.value = false;
        }
    };

    onMounted(() => {
        window.addEventListener("mousedown", handleGlobalPointerDown);
    });

    onUnmounted(() => {
        window.removeEventListener("mousedown", handleGlobalPointerDown);
    });
};
