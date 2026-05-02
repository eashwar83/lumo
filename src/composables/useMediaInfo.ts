import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { PlayerApi } from "./usePlaybackController";
import { getMockMediaInfo } from "../mock/mediaInfo";

export const useMediaInfo = (player: PlayerApi) => {
    let mediaSizeQueryId = 0;
    const mediaFileSizeBytes = ref<number | null>(null);

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
                typeof fileSize === "number" &&
                Number.isFinite(fileSize) &&
                fileSize > 0
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

    const mediaInfo = computed(() => {
        if (!player.state.media.isFileLoaded) return null;
        return getMockMediaInfo(player.state.media.url, {
            durationSeconds: player.state.playback.duration,
            fileSizeBytes: mediaFileSizeBytes.value,
        });
    });

    const statusBadges = computed(() => mediaInfo.value?.badges ?? []);

    return {
        mediaInfo,
        statusBadges,
    };
};
