import type { PlayerApi } from "./usePlaybackController";
import { resolveAdjacentPathInSameDirectory } from "./usePlaybackAdjacency";
import {
    resolvePlaybackNavigationPath,
    type PlaybackDirection,
} from "../utils/playbackNavigation";

type PlaylistApi = {
    getAdjacentPath: (
        currentPath: string,
        direction: PlaybackDirection,
    ) => string | null;
    getPathForEnd: (currentPath: string) => string | null;
};

type UsePlaybackNavigationOptions = {
    player: PlayerApi;
    playlistState: PlaylistApi;
    playPath: (path: string) => Promise<void>;
};

export const usePlaybackNavigation = ({
    player,
    playlistState,
    playPath,
}: UsePlaybackNavigationOptions) => {
    const resolveTrackPath = (direction: PlaybackDirection) =>
        resolvePlaybackNavigationPath({
            currentPath: player.state.media.url,
            direction,
            resolvePlaylistPath: playlistState.getAdjacentPath,
            resolveDirectoryPath: resolveAdjacentPathInSameDirectory,
        });

    const playTrack = async (direction: PlaybackDirection) => {
        const nextPath = await resolveTrackPath(direction);
        if (!nextPath) return;
        await playPath(nextPath);
    };

    const playNextAfterEnd = async () => {
        const currentPath = player.state.media.url;
        const nextPath = await resolvePlaybackNavigationPath({
            currentPath,
            direction: 1,
            resolvePlaylistPath: () => playlistState.getPathForEnd(currentPath),
            resolveDirectoryPath: resolveAdjacentPathInSameDirectory,
        });
        if (!nextPath) return;
        await playPath(nextPath);
    };

    return {
        resolveTrackPath,
        playPreviousTrack: () => playTrack(-1),
        playNextTrack: () => playTrack(1),
        playNextAfterEnd,
    };
};
