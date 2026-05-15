import { computed, onMounted, onUnmounted, ref, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { HistoryEntry } from "../types/history";
import type { NetworkPlayRequest } from "../types/network";
import { parsePlaybackSource } from "../utils/playbackSource";
import type { PlayerApi } from "./usePlaybackController";
import { loadUiState } from "./useUiStateStore";
import {
    ALLOW_URL_INPUT_DURING_PLAYBACK_SETTING_LABEL,
    DEFAULT_SPEED_SETTING_LABEL,
    DISABLE_SUBTITLES_SETTING_LABEL,
    ENABLE_COMPACT_MODE_SETTING_LABEL,
    PLAYBACK_TITLE_SETTING_LABEL,
    SETTINGS_UPDATED_EVENT,
    SKIP_INTRO_SECONDS_SETTING_LABEL,
    WALLPAPER_MODE_SETTING_LABEL,
    type PlaybackTitleMode,
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
    updateTitle: (path: string, title: string) => void;
};

type PlaylistApi = {
    createPlaylistWithPaths: (
        paths: string[],
        options?: {
            name?: string;
            openInDrawer?: boolean;
            setAsPlayback?: boolean;
        },
    ) => string | null;
    createPlaylistWithEntries: (
        entries: Array<{ path: string; title?: string }>,
        options?: {
            name?: string;
            openInDrawer?: boolean;
            setAsPlayback?: boolean;
        },
    ) => string | null;
};

type ParsedPlaylistEntry = {
    path: string;
    title?: string | null;
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
    loadingState?: {
        isLoading: Ref<boolean>;
        loadingUrl: Ref<string>;
    };
    onPlaybackIntent?: () => void | Promise<void>;
};

type StoredSettingGroup = {
    title: string;
    items: Array<{ label: string; value: string }>;
};

type PlaybackPreferences = {
    skipIntroSeconds: number;
    defaultSpeed: number;
    autoPlay: boolean;
    playbackTitleMode: PlaybackTitleMode;
    compactModeEnabled: boolean;
    wallpaperModeEnabled: boolean;
    subtitlesDisabled: boolean;
};

type SmbPlaybackSourceResolution = {
    connectionId: string;
    filePath: string;
    playbackKey: string;
};

const resolveSmbPlaybackSourceFromUrl = (url: string) =>
    invoke<SmbPlaybackSourceResolution | null>(
        "resolve_smb_playback_source_from_url",
        { url },
    );

const DEFAULT_PLAYBACK_PREFERENCES: PlaybackPreferences = {
    skipIntroSeconds: 0,
    defaultSpeed: 1.0,
    autoPlay: true,
    playbackTitleMode: "Show",
    compactModeEnabled: false,
    wallpaperModeEnabled: false,
    subtitlesDisabled: false,
};

const normalizePlaybackTitleMode = (
    value?: string | null,
): PlaybackTitleMode => {
    const normalized = value?.trim().toLowerCase();
    if (normalized === "editable" || normalized === "on") {
        return "Editable";
    }
    if (normalized === "hidden") {
        return "Hidden";
    }
    return "Show";
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
    const playbackTitleModeValue = normalizePlaybackTitleMode(
        getValue(PLAYBACK_TITLE_SETTING_LABEL) ??
            getValue(ALLOW_URL_INPUT_DURING_PLAYBACK_SETTING_LABEL),
    );
    const compactModeValue = getValue(ENABLE_COMPACT_MODE_SETTING_LABEL) ?? "On";
    const wallpaperModeValue = getValue(WALLPAPER_MODE_SETTING_LABEL) ?? "Disable";
    const subtitlesDisabledValue =
        getValue(DISABLE_SUBTITLES_SETTING_LABEL) ?? "Off";

    return {
        skipIntroSeconds,
        defaultSpeed:
            Number.isFinite(defaultSpeed) && defaultSpeed > 0 ? defaultSpeed : 1.0,
        autoPlay: true,
        playbackTitleMode: playbackTitleModeValue,
        compactModeEnabled: compactModeValue === "On",
        wallpaperModeEnabled: wallpaperModeValue === "Enable",
        subtitlesDisabled: subtitlesDisabledValue === "On",
    };
};

const isPlaylistSource = (value: string) => {
    const trimmed = value.trim();
    if (!trimmed) return false;
    try {
        const parsed = new URL(trimmed);
        const pathname = parsed.pathname.toLowerCase();
        return pathname.endsWith(".m3u") || pathname.endsWith(".m3u8");
    } catch {
        const lower = trimmed.toLowerCase();
        return lower.endsWith(".m3u") || lower.endsWith(".m3u8");
    }
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
    loadingState,
    onPlaybackIntent,
}: UsePlaybackFlowOptions) => {
    const isLoading = loadingState?.isLoading ?? ref(false);
    const loadingUrl = loadingState?.loadingUrl ?? ref("");
    const pendingResume = ref<{ url: string; position: number } | null>(null);
    const hideHistory = ref(false);
    const playbackPreferences = ref<PlaybackPreferences>({
        ...DEFAULT_PLAYBACK_PREFERENCES,
        compactModeEnabled: true,
        wallpaperModeEnabled: false,
    });
    const preferredTitleByUrl = new Map<string, string>();
    const preferredTitleByResourceKey = new Map<string, string>();
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

    const triggerPlaybackIntent = async () => {
        await onPlaybackIntent?.();
    };

    const resourceKeyFromUrl = (value: string) => {
        const raw = value.trim();
        if (!raw) return "";
        try {
            const parsed = new URL(raw);
            const pathname = decodeURIComponent(parsed.pathname || "").trim();
            if (!pathname) return parsed.origin.toLowerCase();
            return `${parsed.origin}${pathname}`.toLowerCase();
        } catch {
            return raw.toLowerCase();
        }
    };

    const rememberPreferredTitle = (url: string, preferredTitle?: string) => {
        const normalizedPreferredTitle = preferredTitle?.trim() || "";
        if (!normalizedPreferredTitle) return "";
        const fileNameFromUrl = (() => {
            try {
                const parsed = new URL(url);
                const pathname = decodeURIComponent(parsed.pathname || "");
                const segments = pathname.split("/").filter(Boolean);
                return segments.length ? segments[segments.length - 1] : "";
            } catch {
                const segments = url.split("/").filter(Boolean);
                return segments.length ? segments[segments.length - 1] : "";
            }
        })();
        const extensionMatch = fileNameFromUrl.match(/(\.[a-z0-9]{1,8})$/i);
        const normalizedWithExtension =
            !/\.[a-z0-9]{1,8}$/i.test(normalizedPreferredTitle) && extensionMatch
                ? `${normalizedPreferredTitle}${extensionMatch[1]}`
                : normalizedPreferredTitle;
        preferredTitleByUrl.set(url, normalizedWithExtension);
        const key = resourceKeyFromUrl(url);
        if (key) preferredTitleByResourceKey.set(key, normalizedWithExtension);
        return normalizedWithExtension;
    };

    const applyResolvedMediaTitle = (url: string, title?: string | null) => {
        const normalizedTitle = title?.trim() || "";
        if (!normalizedTitle) return;
        if (player.state.media.url !== url) return;
        player.state.media.title = rememberPreferredTitle(url, normalizedTitle);
        history.updateTitle(url, player.state.media.title);
    };

    const playLocalPath = async (path: string, preferredTitle?: string) => {
        if (!path) return;
        await triggerPlaybackIntent();
        hideHistory.value = true;
        nowPlaying.clearArtwork();
        tracks.resetTracks();
        player.state.media.url = path;
        player.state.media.title = rememberPreferredTitle(path, preferredTitle);
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        player.state.playback.hwdecCurrent = "";
        loadingUrl.value = path;
        isLoading.value = true;
        await ensurePlaybackPreferencesLoaded();
        const preferences = playbackPreferences.value;
        await player.setPlaybackSpeed(preferences.defaultSpeed);
        const resumePosition = getStartPosition(path, preferences.skipIntroSeconds);
        pendingResume.value = { url: path, position: resumePosition };
        const result = await player.loadFile(resumePosition, preferences.autoPlay);
        applyResolvedMediaTitle(path, result.title);
    };

    const playWebdav = async (
        connectionId: string,
        filePath: string,
        playbackKey: string,
        preferredTitle?: string,
    ) => {
        await triggerPlaybackIntent();
        hideHistory.value = true;
        nowPlaying.clearArtwork();
        tracks.resetTracks();
        player.state.media.url = playbackKey;
        player.state.media.title = rememberPreferredTitle(
            playbackKey,
            preferredTitle,
        );
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        player.state.playback.hwdecCurrent = "";
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
        await player.loadNetworkFile(
            "webdav",
            connectionId,
            filePath,
            resumePosition,
            preferences.autoPlay,
        );
    };

    const playDlna = async (
        resourceUrl: string,
        playbackKey: string,
        preferredTitle?: string,
    ) => {
        await triggerPlaybackIntent();
        hideHistory.value = true;
        nowPlaying.clearArtwork();
        tracks.resetTracks();
        player.state.media.url = playbackKey;
        player.state.media.title = rememberPreferredTitle(
            playbackKey,
            preferredTitle,
        );
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        player.state.playback.hwdecCurrent = "";
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
        const result = await player.loadFileAtUrl(
            resourceUrl,
            resumePosition,
            preferences.autoPlay,
        );
        applyResolvedMediaTitle(playbackKey, result.title);
    };

    const playSmb = async (url: string, preferredTitle?: string) => {
        await triggerPlaybackIntent();
        hideHistory.value = true;
        nowPlaying.clearArtwork();
        tracks.resetTracks();
        player.state.media.url = url;
        player.state.media.title = rememberPreferredTitle(url, preferredTitle);
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        player.state.playback.hwdecCurrent = "";
        loadingUrl.value = url;
        isLoading.value = true;
        await ensurePlaybackPreferencesLoaded();
        const preferences = playbackPreferences.value;
        await player.setPlaybackSpeed(preferences.defaultSpeed);
        const resumePosition = getStartPosition(
            url,
            preferences.skipIntroSeconds,
        );
        pendingResume.value = {
            url,
            position: resumePosition,
        };
        const result = await player.loadFileAtUrl(
            url,
            resumePosition,
            preferences.autoPlay,
        );
        applyResolvedMediaTitle(url, result.title);
    };

    const playSmbNetwork = async (
        connectionId: string,
        filePath: string,
        playbackKey: string,
        preferredTitle?: string,
    ) => {
        await triggerPlaybackIntent();
        hideHistory.value = true;
        nowPlaying.clearArtwork();
        tracks.resetTracks();
        player.state.media.url = playbackKey;
        player.state.media.title = rememberPreferredTitle(
            playbackKey,
            preferredTitle,
        );
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        player.state.playback.hwdecCurrent = "";
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
        await player.loadNetworkFile(
            "smb",
            connectionId,
            filePath,
            resumePosition,
            preferences.autoPlay,
        );
    };

    const playPath = async (path: string, preferredTitle?: string) => {
        if (!path) return;
        const source = parsePlaybackSource(path);
        if (source.type === "webdav") {
            await playWebdav(
                source.connectionId,
                source.filePath,
                source.key,
                preferredTitle,
            );
            return;
        }
        if (source.type === "dlna") {
            await playDlna(source.resourceUrl, source.key, preferredTitle);
            return;
        }
        if (source.type === "smb") {
            if (source.connectionId && source.filePath) {
                await playSmbNetwork(
                    source.connectionId,
                    source.filePath,
                    source.key,
                    preferredTitle,
                );
                return;
            }
            if (source.url) {
                const networkSource = await resolveSmbPlaybackSourceFromUrl(source.url);
                if (networkSource) {
                    await playSmbNetwork(
                        networkSource.connectionId,
                        networkSource.filePath,
                        networkSource.playbackKey,
                        preferredTitle,
                    );
                    return;
                }
                await playSmb(source.url, preferredTitle);
                return;
            }
            throw new Error("Invalid SMB playback source");
        }
        await playLocalPath(source.path, preferredTitle);
    };

    const openWithSelected = async (selected: string[]) => {
        if (!selected.length) {
            hideHistory.value = false;
            isLoading.value = false;
            return;
        }
        if (selected.length === 1) {
            const selectedPath = selected[0];
            if (isPlaylistSource(selectedPath)) {
                try {
                    const entries: ParsedPlaylistEntry[] =
                        await player.parsePlaylistSource(selectedPath);
                    const paths = entries
                        .map((entry) => entry.path?.trim() ?? "")
                        .filter(Boolean);
                    if (!paths.length) {
                        hideHistory.value = false;
                        isLoading.value = false;
                        return;
                    }
                    if (paths.length > 1) {
                        const fileName = selectedPath.split(/[/\\]+/).pop() ?? "";
                        const playlistName = fileName.replace(/\.(m3u8?|M3U8?)$/, "");
                        playlistState.createPlaylistWithEntries(
                            entries.map((entry) => ({
                                path: entry.path?.trim() ?? "",
                                title: entry.title?.trim() || undefined,
                            })),
                            {
                            name: playlistName || undefined,
                            setAsPlayback: true,
                            },
                        );
                    }
                    const firstEntry = entries.find(
                        (entry) => entry.path?.trim() === paths[0],
                    );
                    await playPath(paths[0], firstEntry?.title?.trim() || undefined);
                    return;
                } catch {
                    // Fall back to default open behavior when parsing fails.
                }
            }
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
        if (isPlaylistSource(player.state.media.url)) {
            try {
                const source = player.state.media.url;
                const entries: ParsedPlaylistEntry[] =
                    await player.parsePlaylistSource(source);
                const paths = entries
                    .map((entry) => entry.path?.trim() ?? "")
                    .filter(Boolean);
                if (!paths.length) {
                    hideHistory.value = false;
                    isLoading.value = false;
                    return;
                }
                if (paths.length > 1) {
                    let playlistName = "IPTV Playlist";
                    try {
                        const parsed = new URL(source);
                        const fileName = parsed.pathname.split("/").pop() ?? "";
                        const normalized = fileName.replace(/\.(m3u8?|M3U8?)$/, "");
                        if (normalized.trim()) {
                            playlistName = normalized;
                        }
                    } catch {
                        const fileName = source.split(/[/\\]+/).pop() ?? "";
                        const normalized = fileName.replace(/\.(m3u8?|M3U8?)$/, "");
                        if (normalized.trim()) {
                            playlistName = normalized;
                        }
                    }
                    playlistState.createPlaylistWithEntries(
                        entries.map((entry) => ({
                            path: entry.path?.trim() ?? "",
                            title: entry.title?.trim() || undefined,
                        })),
                        {
                            name: playlistName,
                            setAsPlayback: true,
                        },
                    );
                }
                const firstEntry = entries.find(
                    (entry) => entry.path?.trim() === paths[0],
                );
                await playPath(paths[0], firstEntry?.title?.trim() || undefined);
                return;
            } catch {
                // Fall through to default load when playlist parsing fails.
            }
        }
        triggerPlaybackIntent();
        hideHistory.value = true;
        tracks.resetTracks();
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        player.state.playback.hwdecCurrent = "";
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
        const result = await player.loadFile(resumePosition, preferences.autoPlay);
        applyResolvedMediaTitle(player.state.media.url, result.title);
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
        const preferredTitle = entry.title?.trim() || "";
        const source = parsePlaybackSource(entry.path);
        if (source.type === "webdav") {
            await playWebdav(
                source.connectionId,
                source.filePath,
                source.key,
                preferredTitle,
            );
            return;
        }
        if (source.type === "dlna") {
            await playDlna(
                source.resourceUrl,
                source.key,
                preferredTitle,
            );
            return;
        }
        if (source.type === "smb") {
            if (source.connectionId && source.filePath) {
                await playSmbNetwork(
                    source.connectionId,
                    source.filePath,
                    source.key,
                    preferredTitle,
                );
                return;
            }
            if (source.url) {
                const networkSource = await resolveSmbPlaybackSourceFromUrl(source.url);
                if (networkSource) {
                    await playSmbNetwork(
                        networkSource.connectionId,
                        networkSource.filePath,
                        networkSource.playbackKey,
                        preferredTitle,
                    );
                    return;
                }
                await playSmb(source.url, preferredTitle);
                return;
            }
            throw new Error("Invalid SMB playback source");
        }
        hideHistory.value = true;
        await playPath(source.path, preferredTitle);
    };

    const onPlayNetwork = async (payload: NetworkPlayRequest) => {
        const displayName = payload.displayName?.trim() || "";
        const protocol = payload.protocol.trim().toLowerCase();
        if (protocol === "http-dlna" || protocol === "dlna") {
            await playDlna(
                payload.filePath,
                payload.playbackKey,
                displayName,
            );
            return;
        }
        if (protocol === "webdav") {
            await playWebdav(
                payload.connectionId,
                payload.filePath,
                payload.playbackKey,
                displayName,
            );
            return;
        }
        if (protocol === "smb" || protocol === "samba") {
            await playSmbNetwork(
                payload.connectionId,
                payload.filePath,
                payload.playbackKey,
                displayName,
            );
            return;
        }
        throw new Error(`Unsupported network protocol for playback: ${payload.protocol}`);
    };

    const onUpdateUrl = (value: string) => {
        player.state.media.url = value;
        player.state.media.title = "";
        isLoading.value = false;
        loadingUrl.value = "";
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        player.state.playback.hwdecCurrent = "";
        nowPlaying.clearArtwork();
    };

    const resolveMediaTitle = (incomingTitle: string, currentUrl: string) => {
        const preferred = preferredTitleByUrl.get(currentUrl)?.trim() || "";
        if (preferred) return preferred;
        const preferredByKey =
            preferredTitleByResourceKey
                .get(resourceKeyFromUrl(currentUrl))
                ?.trim() || "";
        if (preferredByKey) return preferredByKey;
        return incomingTitle.trim();
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
        player.state.media.title = "";
        player.state.playback.isPlaying = false;
        player.state.playback.isBuffering = false;
        player.state.playback.downloadSpeedBps = 0;
        player.state.playback.currentTime = 0;
        player.state.playback.duration = 0;
        player.state.playback.bufferedTime = 0;
        player.state.playback.videoBitrate = 0;
        player.state.playback.hwdecCurrent = "";
        tracks.resetTracks();
        hideAllMenus();
        isInfoOpen.value = false;
    };

    const isLoadingForCurrentUrl = computed(
        () => isLoading.value && loadingUrl.value === player.state.media.url,
    );
    const playbackTitleMode = computed(
        () => playbackPreferences.value.playbackTitleMode,
    );
    const compactModeEnabled = computed(
        () => playbackPreferences.value.compactModeEnabled,
    );
    const wallpaperModeEnabled = computed(
        () => playbackPreferences.value.wallpaperModeEnabled,
    );
    const subtitlesDisabled = computed(
        () => playbackPreferences.value.subtitlesDisabled,
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
        resolveMediaTitle,
        onStopPlayback,
        requestOpenFilePicker,
        openSelectedPaths: openWithSelected,
        playbackTitleMode,
        compactModeEnabled,
        wallpaperModeEnabled,
        subtitlesDisabled,
    };
};
