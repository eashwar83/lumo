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

    return {
        onSeek,
        onSeekRelative,
    };
};
