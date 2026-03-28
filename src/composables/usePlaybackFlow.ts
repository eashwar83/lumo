import { computed, onMounted, onUnmounted, ref, type Ref } from "vue";
import type { HistoryEntry } from "../types/history";
import type { NetworkPlayRequest } from "../types/network";
import { parsePlaybackSource } from "../utils/playbackSource";
import type { PlayerApi } from "./usePlaybackController";
import { loadUiState } from "./useUiStateStore";
import {
    ALLOW_URL_INPUT_DURING_PLAYBACK_SETTING_LABEL,
    AUTO_PLAY_ON_OPEN_SETTING_LABEL,
    DEFAULT_SPEED_SETTING_LABEL,
    ENABLE_COMPACT_MODE_SETTING_LABEL,
    SETTINGS_UPDATED_EVENT,
    SKIP_INTRO_SECONDS_SETTING_LABEL,
} from "../mock/settings";

type TracksApi = {
    resetTracks: () => void;
};

type HistoryApi = {
    hasEntry: (path: string) => boolean;
    getResumePosition: (path: string) => number;
    recordStop: (
        path: string,
        position: number,
        duration: number,
        title?: string,
    ) => void;
};

type PlaylistApi = {
    createPlaylistWithPaths: (
        paths: string[],
        options?: {
            openInDrawer?: boolean;
            setAsPlayback?: boolean;
        },
    ) => string | null;
};

type NowPlayingApi = {
    clearArtwork: () => void;
    clearNowPlaying: () => void;
};

type UsePlaybackFlowOptions = {
    isMacOS: boolean;
    player: PlayerApi;
    tracks: TracksApi;
    history: HistoryApi;
    playlistState: PlaylistApi;
    nowPlaying: NowPlayingApi;
    hideAllMenus: () => void;
    isInfoOpen: Ref<boolean>;
};

type StoredSettingGroup = {
    title: string;
    items: Array<{ label: string; value: string }>;
};

type PlaybackPreferences = {
    skipIntroSeconds: number;
    defaultSpeed: number;
    autoPlay: boolean;
    allowUrlInputDuringPlayback: boolean;
    compactModeEnabled: boolean;
};

const DEFAULT_PLAYBACK_PREFERENCES: PlaybackPreferences = {
    skipIntroSeconds: 0,
    defaultSpeed: 1.0,
    autoPlay: true,
    allowUrlInputDuringPlayback: true,
    compactModeEnabled: false,
};

const parsePlaybackPreferences = (
    groups?: StoredSettingGroup[],
): PlaybackPreferences => {
    const items = groups?.flatMap((group) => group.items) ?? [];
    const getValue = (label: string) =>
        items.find((item) => item.label === label)?.value;

    const skipIntroParsed = Number.parseFloat(
        getValue(SKIP_INTRO_SECONDS_SETTING_LABEL) ?? "",
    );
    const skipIntroSeconds =
        Number.isFinite(skipIntroParsed) && skipIntroParsed > 0
            ? Math.max(0, skipIntroParsed)
            : 0;

    const defaultSpeedRaw = getValue(DEFAULT_SPEED_SETTING_LABEL) ?? "1.0x";
    const defaultSpeed = Number.parseFloat(defaultSpeedRaw.replace(/x$/i, "").trim());
    const autoPlayValue = getValue(AUTO_PLAY_ON_OPEN_SETTING_LABEL) ?? "On";
    const allowUrlInputDuringPlaybackValue =
        getValue(ALLOW_URL_INPUT_DURING_PLAYBACK_SETTING_LABEL) ?? "On";
    const compactModeValue = getValue(ENABLE_COMPACT_MODE_SETTING_LABEL) ?? "On";

    return {
        skipIntroSeconds,
        defaultSpeed:
            Number.isFinite(defaultSpeed) && defaultSpeed > 0 ? defaultSpeed : 1.0,
        autoPlay: autoPlayValue === "On",
        allowUrlInputDuringPlayback: allowUrlInputDuringPlaybackValue === "On",
        compactModeEnabled: compactModeValue === "On",
    };
};

export const usePlaybackFlow = ({
    isMacOS,
    player,
    tracks,
    history,
    playlistState,
    nowPlaying,
    hideAllMenus,
    isInfoOpen,
}: UsePlaybackFlowOptions) => {
    const isLoading = ref(false);
    const loadingUrl = ref("");
    const pendingResume = ref<{ url: string; position: number } | null>(null);
    const hideHistory = ref(false);
    const playbackPreferences = ref<PlaybackPreferences>({
        ...DEFAULT_PLAYBACK_PREFERENCES,
        compactModeEnabled: true,
    });
    let loadPlaybackPreferencesPromise: Promise<void> | null = null;

    const updatePlaybackPreferences = (groups?: StoredSettingGroup[]) => {
        playbackPreferences.value = parsePlaybackPreferences(groups);
    };

    const loadPlaybackPreferences = async () => {
        const stored = await loadUiState<{
            settings?: {
                groups?: StoredSettingGroup[];
            };
        }>();
        updatePlaybackPreferences(stored?.settings?.groups);
    };

    const ensurePlaybackPreferencesLoaded = async () => {
        if (!loadPlaybackPreferencesPromise) {
            loadPlaybackPreferencesPromise = loadPlaybackPreferences().finally(() => {
                loadPlaybackPreferencesPromise = null;
            });
        }
        await loadPlaybackPreferencesPromise;
    };

    const onSettingsUpdated = (event: Event) => {
        const customEvent = event as CustomEvent<{ groups?: StoredSettingGroup[] }>;
        updatePlaybackPreferences(customEvent.detail?.groups);
    };

    const getStartPosition = (path: string, skipIntroSeconds: number) => {
        const hasHistoryEntry = history.hasEntry(path);
        const resumePosition = history.getResumePosition(path);
        if (hasHistoryEntry) return resumePosition;
        return skipIntroSeconds > 0 ? skipIntroSeconds : resumePosition;
    };

    const playPath = async (path: string) => {
        if (!path) return;
        hideHistory.value = true;
        nowPlaying.clearArtwork();
        tracks.resetTracks();
        player.state.media.url = path;
        player.state.media.title = "";
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        loadingUrl.value = path;
        isLoading.value = true;
        await ensurePlaybackPreferencesLoaded();
        const preferences = playbackPreferences.value;
        await player.setPlaybackSpeed(preferences.defaultSpeed);
        const resumePosition = getStartPosition(path, preferences.skipIntroSeconds);
        pendingResume.value = { url: path, position: resumePosition };
        await player.loadFile(resumePosition, preferences.autoPlay);
    };

    const playWebdav = async (
        connectionId: string,
        filePath: string,
        playbackKey: string,
    ) => {
        hideHistory.value = true;
        nowPlaying.clearArtwork();
        tracks.resetTracks();
        player.state.media.url = playbackKey;
        player.state.media.title = "";
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        loadingUrl.value = playbackKey;
        isLoading.value = true;
        await ensurePlaybackPreferencesLoaded();
        const preferences = playbackPreferences.value;
        await player.setPlaybackSpeed(preferences.defaultSpeed);
        const resumePosition = getStartPosition(
            playbackKey,
            preferences.skipIntroSeconds,
        );
        pendingResume.value = {
            url: playbackKey,
            position: resumePosition,
        };
        await player.loadWebdavFile(
            connectionId,
            filePath,
            resumePosition,
            preferences.autoPlay,
        );
    };

    const openWithSelected = async (selected: string[]) => {
        if (!selected.length) {
            hideHistory.value = false;
            isLoading.value = false;
            return;
        }
        if (selected.length > 1) {
            playlistState.createPlaylistWithPaths(selected, {
                setAsPlayback: true,
            });
        }
        await playPath(selected[0]);
    };

    const openWithFilePicker = async () => {
        hideHistory.value = true;
        const selected = await player.pickFiles();
        await openWithSelected(selected);
    };

    const openWithAutoPicker = async () => {
        hideHistory.value = true;
        const selected = await player.pickMediaPathsAuto();
        await openWithSelected(selected);
    };

    const requestOpenFilePicker = () => {
        if (isMacOS) {
            void openWithAutoPicker();
            return;
        }
        void openWithFilePicker();
    };

    const onLoadFile = async () => {
        if (!player.state.media.url) return;
        hideHistory.value = true;
        tracks.resetTracks();
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        loadingUrl.value = player.state.media.url;
        isLoading.value = true;
        await ensurePlaybackPreferencesLoaded();
        const preferences = playbackPreferences.value;
        await player.setPlaybackSpeed(preferences.defaultSpeed);
        const resumePosition = getStartPosition(
            player.state.media.url,
            preferences.skipIntroSeconds,
        );
        pendingResume.value = {
            url: player.state.media.url,
            position: resumePosition,
        };
        await player.loadFile(resumePosition, preferences.autoPlay);
        if (!player.state.media.isFileLoaded) {
            hideHistory.value = false;
        }
    };

    onMounted(() => {
        void ensurePlaybackPreferencesLoaded();
        if (typeof window === "undefined") return;
        window.addEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    onUnmounted(() => {
        if (typeof window === "undefined") return;
        window.removeEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    const onPlayHistory = async (entry: HistoryEntry) => {
        const source = parsePlaybackSource(entry.path);
        if (source.type === "webdav") {
            await playWebdav(source.connectionId, source.filePath, source.key);
            return;
        }
        hideHistory.value = true;
        await playPath(source.path);
    };

    const onPlayNetwork = async (payload: NetworkPlayRequest) => {
        await playWebdav(
            payload.connectionId,
            payload.filePath,
            payload.playbackKey,
        );
    };

    const onUpdateUrl = (value: string) => {
        player.state.media.url = value;
        player.state.media.title = "";
        isLoading.value = false;
        loadingUrl.value = "";
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        nowPlaying.clearArtwork();
    };

    const onStopPlayback = async () => {
        isLoading.value = false;
        loadingUrl.value = "";
        nowPlaying.clearNowPlaying();
        history.recordStop(
            player.state.media.url,
            player.state.playback.currentTime,
            player.state.playback.duration,
            player.state.media.title,
        );
        await player.stopPlayback();
        hideHistory.value = false;
        player.state.media.isFileLoaded = false;
        player.state.media.lastLoadedUrl = "";
        player.state.media.title = "";
        player.state.playback.isPlaying = false;
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        player.state.playback.currentTime = 0;
        player.state.playback.duration = 0;
        player.state.playback.bufferedTime = 0;
        player.state.playback.videoBitrate = 0;
        tracks.resetTracks();
        hideAllMenus();
        isInfoOpen.value = false;
    };

    const isLoadingForCurrentUrl = computed(
        () => isLoading.value && loadingUrl.value === player.state.media.url,
    );
    const allowUrlInputDuringPlayback = computed(
        () => playbackPreferences.value.allowUrlInputDuringPlayback,
    );
    const compactModeEnabled = computed(
        () => playbackPreferences.value.compactModeEnabled,
    );

    return {
        isLoading,
        loadingUrl,
        pendingResume,
        hideHistory,
        isLoadingForCurrentUrl,
        playPath,
        onLoadFile,
        onPlayHistory,
        onPlayNetwork,
        onUpdateUrl,
        onStopPlayback,
        requestOpenFilePicker,
        openSelectedPaths: openWithSelected,
        allowUrlInputDuringPlayback,
        compactModeEnabled,
    };
};
