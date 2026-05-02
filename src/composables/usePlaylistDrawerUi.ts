import { ref } from "vue";
import type { PlaylistScrollState } from "../types/playlist";

const normalizePlaylistDrawerWidthRatio = (value: unknown): number | null => {
    if (typeof value !== "number" || !Number.isFinite(value)) return null;
    if (value <= 0) return null;
    const normalized = Math.min(value, 0.86);
    return Math.round(normalized * 10000) / 10000;
};

export const usePlaylistDrawerUi = () => {
    const playlistScrollState = ref<PlaylistScrollState>({
        list: 0,
        playlists: {},
    });
    const playlistDrawerWidthRatio = ref<number | null>(null);

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

    return {
        playlistScrollState,
        playlistDrawerWidthRatio,
        onPlaylistScrollPositionChange,
        onPlaylistDrawerWidthRatioChange,
    };
};
