import { invoke } from "@tauri-apps/api/core";

type AdjacentPlaybackSourceResult = {
    playbackKey: string;
};

export const resolveAdjacentPathInSameDirectory = async (
    currentPath: string,
    direction: 1 | -1,
): Promise<string | null> => {
    const result = await invoke<AdjacentPlaybackSourceResult | null>(
        "resolve_adjacent_playback_source",
        {
            payload: {
                playbackKey: currentPath,
                direction,
            },
        },
    ).catch(() => null);
    return result?.playbackKey ?? null;
};
