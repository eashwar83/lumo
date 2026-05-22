import {
    createDebouncedUiStateSaver,
    loadUiState,
} from "./useUiStateStore";

type PlaybackVolumeApi = {
    state: {
        playback: {
            volume: number;
        };
    };
    setVolume: (volume: number) => Promise<void>;
    toggleMuted: () => Promise<void>;
};

type PersistedPlaybackVolumeState = {
    volume?: number;
};

const clampVolume = (value: number) =>
    Math.max(0, Math.min(100, Math.round(value)));

const normalizePersistedVolume = (value: unknown) => {
    if (typeof value !== "number" || !Number.isFinite(value)) return null;
    return clampVolume(value);
};

export const usePlaybackVolumePersistence = (player: PlaybackVolumeApi) => {
    const stateSaver = createDebouncedUiStateSaver(350);

    const buildPersistedState = () => ({
        playback: {
            volume: clampVolume(player.state.playback.volume),
        } satisfies PersistedPlaybackVolumeState,
    });

    const persistVolumeDebounced = () => {
        stateSaver.saveDebounced(buildPersistedState());
    };

    const setVolume = async (volume: number) => {
        await player.setVolume(volume);
        persistVolumeDebounced();
    };

    const toggleMuted = async () => {
        await player.toggleMuted();
        persistVolumeDebounced();
    };

    void (async () => {
        const stored = await loadUiState<{
            playback?: PersistedPlaybackVolumeState;
        }>();
        const persistedVolume = normalizePersistedVolume(stored?.playback?.volume);
        if (persistedVolume === null) return;
        await player.setVolume(persistedVolume);
    })();

    return {
        setVolume,
        toggleMuted,
    };
};
