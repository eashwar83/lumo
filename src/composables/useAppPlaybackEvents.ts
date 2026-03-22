import type { Ref } from "vue";
import type { ProgressPayload } from "../types/media";
import { AUTO_PLAY_NEXT_IN_PLAYLIST_SETTING_LABEL } from "../mock/settings";
import { loadUiState } from "./useUiStateStore";

type EndFilePayload = {
    reason?: string;
};

type PendingResume = {
    url: string;
    position: number;
};

type PlayerApi = {
    state: {
        media: {
            url: string;
            title: string;
        };
        playback: {
            duration: number;
        };
    };
    setLoopFile: (enabled: boolean) => Promise<void>;
};

type PlaylistApi = {
    getPathForEnd: (currentPath: string) => string | null;
};

type HistoryApi = {
    markPlayed: (
        path: string,
        position: number,
        duration: number,
        title?: string,
    ) => void;
    recordProgress: (
        path: string,
        position: number,
        duration: number,
        isPlaying: boolean,
        title?: string,
    ) => void;
};

type NowPlayingApi = {
    updateNowPlayingMetadata: () => void;
    updateNowPlayingStatus: (position?: number) => void;
    maybeUpdateNowPlayingStatus: (position?: number) => void;
    captureNowPlayingArtwork: () => Promise<void>;
};

type TracksApi = {
    applyExternalTracksForUrl: (url: string) => Promise<void>;
};

type UseAppPlaybackEventsOptions = {
    player: PlayerApi;
    tracks: TracksApi;
    playlistState: PlaylistApi;
    history: HistoryApi;
    nowPlaying: NowPlayingApi;
    pendingResume: Ref<PendingResume | null>;
    isLoopOne: Ref<boolean>;
    isLoading: Ref<boolean>;
    loadingUrl: Ref<string>;
    playPath: (path: string) => Promise<void>;
};

export const useAppPlaybackEvents = ({
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
}: UseAppPlaybackEventsOptions) => {
    const shouldAutoPlayNextInPlaylist = async () => {
        const stored = await loadUiState<{
            settings?: {
                groups?: Array<{
                    title: string;
                    items: Array<{ label: string; value: string }>;
                }>;
            };
        }>();
        const value = stored?.settings?.groups
            ?.flatMap((group) => group.items)
            .find(
                (item) =>
                    item.label === AUTO_PLAY_NEXT_IN_PLAYLIST_SETTING_LABEL,
            )?.value;
        if (!value) return true;
        return value === "On";
    };

    const handleEndFile = async () => {
        if (!(await shouldAutoPlayNextInPlaylist())) return;
        const nextPath = playlistState.getPathForEnd(player.state.media.url);
        if (!nextPath) return;
        await playPath(nextPath);
    };

    const onFileLoaded = () => {
        const pending = pendingResume.value;
        const resumePosition =
            pending && pending.url === player.state.media.url
                ? pending.position
                : 0;
        if (pending && pending.url === player.state.media.url) {
            pendingResume.value = null;
        }

        history.markPlayed(
            player.state.media.url,
            resumePosition,
            player.state.playback.duration,
            player.state.media.title,
        );
        void player.setLoopFile(isLoopOne.value);
        isLoading.value = false;
        loadingUrl.value = "";
        nowPlaying.updateNowPlayingMetadata();
        nowPlaying.updateNowPlayingStatus(resumePosition);
        void nowPlaying.captureNowPlayingArtwork();
        void tracks.applyExternalTracksForUrl(player.state.media.url);
    };

    const onProgress = (payload: ProgressPayload) => {
        history.recordProgress(
            player.state.media.url,
            payload.time_pos,
            player.state.playback.duration,
            payload.is_playing,
            player.state.media.title,
        );
        nowPlaying.maybeUpdateNowPlayingStatus(payload.time_pos);
    };

    const onEndFile = ({ reason }: EndFilePayload) => {
        if (reason !== "eof") return;
        void handleEndFile();
    };

    return {
        onFileLoaded,
        onProgress,
        onEndFile,
    };
};
