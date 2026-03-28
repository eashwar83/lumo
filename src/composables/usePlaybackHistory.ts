import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { HistoryEntry } from "../types/history";
import { normalizePlaybackKey } from "../utils/playbackSource";

const MAX_HISTORY = 100;
const SAVE_DEBOUNCE_MS = 800;
const MIN_PROGRESS_UPDATE_MS = 1000;
const RESUME_FROM_START_THRESHOLD = 0.99;

const normalizeTrackList = (value: unknown): string[] => {
    if (!Array.isArray(value)) return [];
    const deduped = new Set<string>();
    value.forEach((entry) => {
        if (typeof entry !== "string") return;
        const trimmed = entry.trim();
        if (!trimmed) return;
        deduped.add(trimmed);
    });
    return Array.from(deduped);
};

const normalizeHistory = (entries: HistoryEntry[]): HistoryEntry[] => {
    const map = new Map<string, HistoryEntry>();
    entries.forEach((entry) => {
        if (!entry?.path) return;
        const normalizedPath = normalizePlaybackKey(entry.path);
        const normalizedTitle =
            typeof entry.title === "string" ? entry.title.trim() : "";
        const normalized: HistoryEntry = {
            id:
                typeof entry.id === "string" && entry.id.trim()
                    ? entry.id.trim()
                    : undefined,
            path: normalizedPath,
            title: normalizedTitle,
            lastPosition: Number.isFinite(entry.lastPosition)
                ? entry.lastPosition
                : 0,
            duration:
                Number.isFinite(entry.duration) && entry.duration > 0
                    ? entry.duration
                    : 0,
            lastPlayedAt: Number.isFinite(entry.lastPlayedAt)
                ? entry.lastPlayedAt
                : 0,
            isPinned: Boolean(entry.isPinned),
            externalAudioTracks: normalizeTrackList(entry.externalAudioTracks),
            externalSubTracks: normalizeTrackList(entry.externalSubTracks),
        };
        map.set(normalizedPath, normalized);
    });
    return Array.from(map.values())
        .sort((a, b) => {
            if (a.isPinned !== b.isPinned) {
                return a.isPinned ? -1 : 1;
            }
            return b.lastPlayedAt - a.lastPlayedAt;
        })
        .slice(0, MAX_HISTORY);
};

export const usePlaybackHistory = () => {
    const history = ref<HistoryEntry[]>([]);
    const isReady = ref(false);
    let saveTimer: number | null = null;
    let lastProgressSaveAt = 0;

    const clearSaveTimer = () => {
        if (saveTimer) {
            window.clearTimeout(saveTimer);
            saveTimer = null;
        }
    };

    const saveHistory = async () => {
        await invoke("save_play_history", { entries: history.value });
    };

    const saveHistoryEntry = async (entry: HistoryEntry) => {
        await invoke("save_play_history_entry", { entry });
    };

    const scheduleSave = () => {
        clearSaveTimer();
        saveTimer = window.setTimeout(() => {
            void saveHistory();
            saveTimer = null;
        }, SAVE_DEBOUNCE_MS);
    };

    const loadHistory = async () => {
        try {
            const entries = await invoke<HistoryEntry[]>("load_play_history");
            history.value = normalizeHistory(entries ?? []);
        } catch {
            history.value = [];
        } finally {
            isReady.value = true;
        }
    };

    const clearHistory = () => {
        history.value = [];
        scheduleSave();
    };

    const removeEntry = (path: string) => {
        history.value = history.value.filter((entry) => entry.path !== path);
        scheduleSave();
    };

    const upsertEntry = (entry: HistoryEntry) => {
        const next = normalizeHistory([
            entry,
            ...history.value.filter((item) => item.path !== entry.path),
        ]);
        history.value = next;
    };

    const createEntry = (
        path: string,
        position: number,
        duration = 0,
        isPinned = false,
        externalAudioTracks: string[] = [],
        externalSubTracks: string[] = [],
        title = "",
        id?: string,
    ): HistoryEntry => ({
        id,
        path,
        title,
        lastPosition: Math.max(0, position),
        duration: Number.isFinite(duration) && duration > 0 ? duration : 0,
        lastPlayedAt: Date.now(),
        isPinned,
        externalAudioTracks,
        externalSubTracks,
    });

    const upsertAndPersistEntry = async (entry: HistoryEntry) => {
        upsertEntry(entry);
        await saveHistoryEntry(entry);
    };

    const getResumePosition = (path: string): number => {
        const normalizedPath = normalizePlaybackKey(path);
        const entry = history.value.find((item) => item.path === normalizedPath);
        if (!entry) return 0;
        if (
            entry.duration > 0 &&
            entry.lastPosition / entry.duration > RESUME_FROM_START_THRESHOLD
        ) {
            return 0;
        }
        return entry.lastPosition;
    };

    const hasEntry = (path: string): boolean => {
        const normalizedPath = normalizePlaybackKey(path);
        return history.value.some((item) => item.path === normalizedPath);
    };

    const markPlayed = (
        path: string,
        position: number,
        duration = 0,
        title = "",
    ) => {
        if (!path) return;
        const normalizedPath = normalizePlaybackKey(path);
        const existing = history.value.find(
            (item) => item.path === normalizedPath,
        );
        const isPinned = existing?.isPinned ?? false;
        const resolvedTitle = title.trim() || existing?.title || "";
        void upsertAndPersistEntry(
            createEntry(
                normalizedPath,
                position,
                duration,
                isPinned,
                normalizeTrackList(existing?.externalAudioTracks),
                normalizeTrackList(existing?.externalSubTracks),
                resolvedTitle,
                existing?.id,
            ),
        );
    };

    const recordProgress = (
        path: string,
        position: number,
        duration: number,
        isPlaying: boolean,
        title = "",
    ) => {
        if (!path || !isPlaying) return;
        if (!Number.isFinite(position)) return;
        const now = Date.now();
        if (isPlaying && now - lastProgressSaveAt < MIN_PROGRESS_UPDATE_MS) {
            return;
        }
        lastProgressSaveAt = now;
        const normalizedPath = normalizePlaybackKey(path);
        const existing = history.value.find(
            (item) => item.path === normalizedPath,
        );
        const isPinned = existing?.isPinned ?? false;
        const resolvedTitle = title.trim() || existing?.title || "";
        const entry: HistoryEntry = createEntry(
            normalizedPath,
            position,
            duration,
            isPinned,
            normalizeTrackList(existing?.externalAudioTracks),
            normalizeTrackList(existing?.externalSubTracks),
            resolvedTitle,
            existing?.id,
        );
        entry.lastPlayedAt = now;
        upsertEntry(entry);
        void saveHistoryEntry(entry);
    };

    const recordStop = (
        path: string,
        position: number,
        duration = 0,
        title = "",
    ) => {
        if (!path || position === 0) return;
        const normalizedPath = normalizePlaybackKey(path);
        const existing = history.value.find(
            (item) => item.path === normalizedPath,
        );
        const isPinned = existing?.isPinned ?? false;
        const resolvedTitle = title.trim() || existing?.title || "";
        void upsertAndPersistEntry(
            createEntry(
                normalizedPath,
                position,
                duration,
                isPinned,
                normalizeTrackList(existing?.externalAudioTracks),
                normalizeTrackList(existing?.externalSubTracks),
                resolvedTitle,
                existing?.id,
            ),
        );
    };

    const togglePinned = (path: string) => {
        const normalizedPath = normalizePlaybackKey(path);
        const target = history.value.find((item) => item.path === normalizedPath);
        if (!target) return;
        const nextEntry: HistoryEntry = {
            ...target,
            isPinned: !target.isPinned,
        };
        upsertEntry(nextEntry);
        void saveHistoryEntry(nextEntry);
    };

    const getExternalTracks = (path: string) => {
        const normalizedPath = normalizePlaybackKey(path);
        const entry = history.value.find((item) => item.path === normalizedPath);
        return {
            audio: normalizeTrackList(entry?.externalAudioTracks),
            sub: normalizeTrackList(entry?.externalSubTracks),
        };
    };

    const recordExternalTrack = async (
        path: string,
        kind: "audio" | "sub",
        trackPath: string,
    ) => {
        const normalizedPath = normalizePlaybackKey(path);
        const trimmed = trackPath.trim();
        if (!normalizedPath || !trimmed) return;
        const existing = history.value.find(
            (item) => item.path === normalizedPath,
        );
        const audio = normalizeTrackList(existing?.externalAudioTracks);
        const sub = normalizeTrackList(existing?.externalSubTracks);
        const list = kind === "audio" ? audio : sub;
        if (!list.includes(trimmed)) {
            list.push(trimmed);
        }
        const nextEntry: HistoryEntry = existing
            ? {
                  ...existing,
                  externalAudioTracks: audio,
                  externalSubTracks: sub,
              }
            : {
                  path: normalizedPath,
                  lastPosition: 0,
                  duration: 0,
                  lastPlayedAt: 0,
                  isPinned: false,
                  externalAudioTracks: audio,
                  externalSubTracks: sub,
              };
        upsertEntry(nextEntry);
        await saveHistoryEntry(nextEntry);
    };

    onMounted(() => {
        void loadHistory();
    });

    return {
        history,
        isReady,
        loadHistory,
        getResumePosition,
        hasEntry,
        markPlayed,
        recordProgress,
        recordStop,
        clearHistory,
        removeEntry,
        togglePinned,
        getExternalTracks,
        recordExternalTrack,
    };
};
