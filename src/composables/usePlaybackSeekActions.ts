import type { Ref } from "vue";
import type { PlayerApi } from "./usePlaybackController";

type PlaybackSeekActionsOptions = {
    player: PlayerApi;
    isLoading: Ref<boolean>;
    loadingUrl: Ref<string>;
};

export const usePlaybackSeekActions = ({
    player,
    isLoading,
    loadingUrl,
}: PlaybackSeekActionsOptions) => {
    const beginSeekLoading = () => {
        if (player.state.media.isLivePlayback) return false;
        isLoading.value = true;
        loadingUrl.value = player.state.media.url;
        player.state.playback.downloadSpeedBps = 0;
        return true;
    };

    const onSeek = async (position: number) => {
        if (!beginSeekLoading()) return;
        try {
            await player.seek(position);
        } catch {
            isLoading.value = false;
            loadingUrl.value = "";
        }
    };

    const onSeekRelative = async (position: number, exact = false) => {
        if (!beginSeekLoading()) return;
        try {
            await player.seekRelative(position, exact);
        } catch {
            isLoading.value = false;
            loadingUrl.value = "";
        }
    };

    return {
        onSeek,
        onSeekRelative,
    };
};
