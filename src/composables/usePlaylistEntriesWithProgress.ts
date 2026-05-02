import { computed, type Ref } from "vue";
import type { HistoryEntry } from "../types/history";
import type { PlaylistEntry } from "../types/playlist";
import { normalizePlaybackKey } from "../utils/playbackSource";

export const usePlaylistEntriesWithProgress = (
    orderedPlaylist: Readonly<Ref<PlaylistEntry[]>>,
    historyEntries: Readonly<Ref<HistoryEntry[]>>,
) =>
    computed(() => {
        const ratioByPath = new Map<string, number>();
        historyEntries.value.forEach((entry) => {
            const duration = entry.duration;
            const position = entry.lastPosition;
            if (!Number.isFinite(duration) || duration <= 0) {
                ratioByPath.set(normalizePlaybackKey(entry.path), 0);
                return;
            }
            const ratio = Math.max(0, Math.min(1, position / duration));
            ratioByPath.set(normalizePlaybackKey(entry.path), ratio);
        });

        return orderedPlaylist.value.map((entry) => ({
            ...entry,
            playedRatio: ratioByPath.get(normalizePlaybackKey(entry.path)) ?? 0,
        }));
    });
