import { invoke } from "@tauri-apps/api/core";
import { onUnmounted, ref } from "vue";
import { getPathDisplayName } from "../utils/getPathDisplayName";

type UseNowPlayingStateOptions = {
    isFileLoaded: () => boolean;
    mediaUrl: () => string;
    mediaTitle: () => string;
    isPlaying: () => boolean;
    duration: () => number;
};

const NOW_PLAYING_UPDATE_MS = 1500;
const STATUS_OVERLAY_MS = 1000;

export const useNowPlayingState = ({
    isFileLoaded,
    mediaUrl,
    mediaTitle,
    isPlaying,
    duration,
}: UseNowPlayingStateOptions) => {
    const nowPlayingArtworkPath = ref("");
    const showStatusOverlay = ref(false);
    const statusOverlayMode = ref<"play" | "pause">("play");
    const lastNowPlayingUpdate = ref(0);
    let statusOverlayTimer: number | null = null;
    let statusOverlaySuppressedUntil = 0;

    const clearArtwork = () => {
        nowPlayingArtworkPath.value = "";
    };

    const clearStatusOverlay = () => {
        showStatusOverlay.value = false;
        if (statusOverlayTimer) {
            window.clearTimeout(statusOverlayTimer);
            statusOverlayTimer = null;
        }
    };

    const triggerStatusOverlay = (mode: "play" | "pause") => {
        statusOverlayMode.value = mode;
        showStatusOverlay.value = true;
        if (statusOverlayTimer) {
            window.clearTimeout(statusOverlayTimer);
        }
        statusOverlayTimer = window.setTimeout(() => {
            showStatusOverlay.value = false;
            statusOverlayTimer = null;
        }, STATUS_OVERLAY_MS);
    };

    // Suppress the play/pause status icon for a short window. Used when a pause
    // is a side effect of another action (e.g. frame stepping) rather than an
    // explicit play/pause by the user.
    const suppressStatusOverlay = (durationMs = 1000) => {
        statusOverlaySuppressedUntil = performance.now() + durationMs;
    };

    const triggerStatusOverlayFromPlayback = () => {
        if (performance.now() < statusOverlaySuppressedUntil) return;
        triggerStatusOverlay(isPlaying() ? "play" : "pause");
    };

    const updateNowPlayingMetadata = () => {
        if (!isFileLoaded()) return;
        const mediaDuration = duration();
        const title = mediaTitle().trim() || getPathDisplayName(mediaUrl());
        void invoke("set_now_playing_metadata", {
            title,
            duration: mediaDuration > 0 ? mediaDuration : null,
            artworkPath: nowPlayingArtworkPath.value || null,
        });
    };

    const updateNowPlayingStatus = (position?: number) => {
        void invoke("set_now_playing_status", {
            isPlaying: isPlaying(),
            position: typeof position === "number" ? position : null,
        });
    };

    const maybeUpdateNowPlayingStatus = (position?: number) => {
        const now = performance.now();
        if (now - lastNowPlayingUpdate.value < NOW_PLAYING_UPDATE_MS) return;
        lastNowPlayingUpdate.value = now;
        updateNowPlayingStatus(position);
    };

    const clearNowPlaying = () => {
        lastNowPlayingUpdate.value = 0;
        clearArtwork();
        void invoke("clear_now_playing");
    };

    const captureNowPlayingArtwork = async () => {
        if (!isFileLoaded()) return;
        const path = await invoke<string | null>("capture_now_playing_artwork", {
            url: mediaUrl(),
        });
        if (!path) return;
        nowPlayingArtworkPath.value = path;
        updateNowPlayingMetadata();
    };

    onUnmounted(() => {
        clearStatusOverlay();
    });

    return {
        nowPlayingArtworkPath,
        showStatusOverlay,
        statusOverlayMode,
        clearArtwork,
        clearStatusOverlay,
        suppressStatusOverlay,
        triggerStatusOverlayFromPlayback,
        updateNowPlayingMetadata,
        updateNowPlayingStatus,
        maybeUpdateNowPlayingStatus,
        clearNowPlaying,
        captureNowPlayingArtwork,
    };
};
