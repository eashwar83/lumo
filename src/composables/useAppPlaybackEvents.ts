import { invoke } from "@tauri-apps/api/core";
import type { Ref } from "vue";
import type { ProgressPayload } from "../types/media";
import { MEDIA_FILE_EXTENSIONS } from "../constants/media";
import { AUTO_PLAY_NEXT_IN_PLAYLIST_SETTING_LABEL } from "../mock/settings";
import type { NetworkBrowseEntry, NetworkBrowseResult } from "../types/network";
import { createWebdavPlaybackKey, parsePlaybackSource } from "../utils/playbackSource";
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
    const mediaExtensionSet = new Set(
        MEDIA_FILE_EXTENSIONS.map((item) => item.toLowerCase()),
    );

    const getLowerExt = (path: string): string => {
        const index = path.lastIndexOf(".");
        if (index < 0) return "";
        return path.slice(index + 1).toLowerCase();
    };

    const isMediaFilePath = (path: string): boolean =>
        mediaExtensionSet.has(getLowerExt(path));

    const normalizeWebdavPath = (path: string): string => {
        const trimmed = path.trim();
        if (!trimmed || trimmed === "/") return "/";
        const withLeading = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
        return withLeading.replace(/\/+$/, "");
    };

    const getWebdavParentPath = (path: string): string | null => {
        const normalized = normalizeWebdavPath(path);
        if (normalized === "/") return null;
        const index = normalized.lastIndexOf("/");
        if (index <= 0) return "/";
        return normalized.slice(0, index);
    };

    const resolveNextPathInLocalDirectory = async (
        currentPath: string,
    ): Promise<string | null> => {
        if (!isMediaFilePath(currentPath)) return null;
        const siblings = await invoke<string[]>("list_local_media_siblings", {
            path: currentPath,
        }).catch(() => []);
        if (!siblings.length) return null;
        const mediaSiblings = siblings.filter((item) => isMediaFilePath(item));
        const currentIndex = mediaSiblings.findIndex((item) => item === currentPath);
        if (currentIndex < 0 || currentIndex + 1 >= mediaSiblings.length) return null;
        return mediaSiblings[currentIndex + 1] ?? null;
    };

    const resolveNextPathInWebdavDirectory = async (
        connectionId: string,
        filePath: string,
    ): Promise<string | null> => {
        const parentPath = getWebdavParentPath(filePath);
        if (!parentPath) return null;
        const result = await invoke<NetworkBrowseResult>("browse_network_connection", {
            payload: {
                connectionId,
                mode: "browse",
                protocol: "webdav",
                path: parentPath,
            },
        }).catch(() => null);
        if (!result) return null;
        const mediaFiles = result.entries.filter(
            (entry: NetworkBrowseEntry) =>
                entry.entryType === "file" && isMediaFilePath(entry.path),
        );
        const normalizedCurrentPath = normalizeWebdavPath(filePath);
        const currentIndex = mediaFiles.findIndex(
            (entry) => normalizeWebdavPath(entry.path) === normalizedCurrentPath,
        );
        if (currentIndex < 0 || currentIndex + 1 >= mediaFiles.length) return null;
        const nextEntry = mediaFiles[currentIndex + 1];
        return createWebdavPlaybackKey(connectionId, nextEntry.path);
    };

    const resolveNextPathInSameDirectory = async (
        currentPath: string,
    ): Promise<string | null> => {
        const source = parsePlaybackSource(currentPath);
        if (source.type === "local") {
            return resolveNextPathInLocalDirectory(source.path);
        }
        if (source.type === "dlna") {
            return null;
        }
        return resolveNextPathInWebdavDirectory(source.connectionId, source.filePath);
    };

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
        const nextPath =
            playlistState.getPathForEnd(player.state.media.url) ??
            (await resolveNextPathInSameDirectory(player.state.media.url));
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
        isLoading.value = false;
        loadingUrl.value = "";
        void handleEndFile();
    };

    const onPlaybackRestart = () => {
        isLoading.value = false;
        loadingUrl.value = "";
    };

    return {
        onFileLoaded,
        onPlaybackRestart,
        onProgress,
        onEndFile,
    };
};
