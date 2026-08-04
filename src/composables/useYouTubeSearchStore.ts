import { ref } from "vue";
import type { YoutubeFilters } from "./useYouTubeModule";

// Recent and saved YouTube searches, kept on this device only. Stored in
// web storage like the module's other lightweight preferences.

const RECENT_KEY = "lumo.youtube.recentSearches";
const SAVED_KEY = "lumo.youtube.savedSearches";
const MAX_RECENT = 50;

export type StoredSearch = {
    query: string;
    filters: YoutubeFilters;
    at: number;
};

const read = (key: string): StoredSearch[] => {
    try {
        const raw = localStorage.getItem(key);
        if (!raw) return [];
        const parsed = JSON.parse(raw);
        return Array.isArray(parsed) ? (parsed as StoredSearch[]) : [];
    } catch {
        return [];
    }
};

const write = (key: string, entries: StoredSearch[]) => {
    try {
        localStorage.setItem(key, JSON.stringify(entries));
    } catch {
        // Storage unavailable (private mode / quota) — session-only.
    }
};

const recent = ref<StoredSearch[]>(read(RECENT_KEY));
const saved = ref<StoredSearch[]>(read(SAVED_KEY));

/** Same query AND same filter set counts as the same search. */
const isSameSearch = (a: StoredSearch, query: string, filters: YoutubeFilters) =>
    a.query === query &&
    a.filters.sort === filters.sort &&
    a.filters.uploadDate === filters.uploadDate &&
    a.filters.duration === filters.duration &&
    a.filters.kind === filters.kind &&
    a.filters.hd === filters.hd;

export const describeFilters = (filters: YoutubeFilters): string => {
    const parts: string[] = [];
    if (filters.sort && filters.sort !== "relevance") {
        parts.push(
            filters.sort === "date"
                ? "Upload date"
                : filters.sort === "views"
                  ? "View count"
                  : "Rating",
        );
    }
    if (filters.duration) {
        parts.push(
            filters.duration === "short"
                ? "Under 4 min"
                : filters.duration === "medium"
                  ? "4–20 min"
                  : "Over 20 min",
        );
    }
    if (filters.uploadDate) {
        parts.push(
            filters.uploadDate === "hour"
                ? "Last hour"
                : filters.uploadDate === "today"
                  ? "Today"
                  : filters.uploadDate === "week"
                    ? "This week"
                    : filters.uploadDate === "month"
                      ? "This month"
                      : "This year",
        );
    }
    if (filters.kind) {
        parts.push(
            filters.kind === "video"
                ? "Videos"
                : filters.kind === "channel"
                  ? "Channels"
                  : filters.kind === "playlist"
                    ? "Playlists"
                    : "Films",
        );
    }
    if (filters.hd) parts.push("HD");
    return parts.join(" · ");
};

export const useYouTubeSearchStore = () => {
    const recordSearch = (query: string, filters: YoutubeFilters) => {
        const trimmed = query.trim();
        if (!trimmed) return;
        const entry: StoredSearch = {
            query: trimmed,
            filters: { ...filters },
            at: Date.now(),
        };
        recent.value = [
            entry,
            ...recent.value.filter(
                (item) => !isSameSearch(item, trimmed, filters),
            ),
        ].slice(0, MAX_RECENT);
        write(RECENT_KEY, recent.value);
    };

    const removeRecent = (entry: StoredSearch) => {
        recent.value = recent.value.filter((item) => item.at !== entry.at);
        write(RECENT_KEY, recent.value);
    };

    const clearRecent = () => {
        recent.value = [];
        write(RECENT_KEY, recent.value);
    };

    const isSaved = (query: string, filters: YoutubeFilters) =>
        saved.value.some((item) => isSameSearch(item, query.trim(), filters));

    const toggleSaved = (query: string, filters: YoutubeFilters): boolean => {
        const trimmed = query.trim();
        if (!trimmed) return false;
        if (isSaved(trimmed, filters)) {
            saved.value = saved.value.filter(
                (item) => !isSameSearch(item, trimmed, filters),
            );
            write(SAVED_KEY, saved.value);
            return false;
        }
        saved.value = [
            { query: trimmed, filters: { ...filters }, at: Date.now() },
            ...saved.value,
        ];
        write(SAVED_KEY, saved.value);
        return true;
    };

    const removeSaved = (entry: StoredSearch) => {
        saved.value = saved.value.filter((item) => item.at !== entry.at);
        write(SAVED_KEY, saved.value);
    };

    return {
        recent,
        saved,
        recordSearch,
        removeRecent,
        clearRecent,
        isSaved,
        toggleSaved,
        removeSaved,
    };
};
