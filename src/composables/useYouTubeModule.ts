import { reactive, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export type YoutubeItem = {
    kind: "video" | "channel" | "playlist";
    id: string;
    url: string;
    title: string;
    channel?: string | null;
    channelUrl?: string | null;
    durationSeconds?: number | null;
    durationText?: string | null;
    viewCountText?: string | null;
    publishedText?: string | null;
    thumbnailUrl?: string | null;
    videoCountText?: string | null;
    badge?: string | null;
};

type YoutubeSearchPage = {
    items: YoutubeItem[];
    nextCursor: string | null;
    source: string;
};

export type YoutubeTab = "search" | "trending" | "history" | "downloads";

export type YoutubeFilters = {
    sort: "relevance" | "date" | "views" | "rating";
    uploadDate: "" | "hour" | "today" | "week" | "month" | "year";
    duration: "" | "short" | "medium" | "long";
    kind: "" | "video" | "channel" | "playlist" | "movie";
    hd: boolean;
};

const defaultFilters = (): YoutubeFilters => ({
    sort: "relevance",
    uploadDate: "",
    duration: "",
    kind: "",
    hd: false,
});

// YouTube's own date/view sort params return semi-random order these days
// (verified against live Innertube), so we re-sort the fetched window
// ourselves from the metadata text it serves.

const parseViewCount = (text?: string | null): number => {
    if (!text) return -1;
    const match = text
        .replace(/,/g, "")
        .match(/([\d.]+)\s*([KMB])?/i);
    if (!match) return -1;
    const base = Number.parseFloat(match[1]);
    if (!Number.isFinite(base)) return -1;
    const unit = (match[2] ?? "").toUpperCase();
    const factor =
        unit === "B" ? 1e9 : unit === "M" ? 1e6 : unit === "K" ? 1e3 : 1;
    return base * factor;
};

const AGE_UNIT_HOURS: Record<string, number> = {
    second: 1 / 3600,
    minute: 1 / 60,
    hour: 1,
    day: 24,
    week: 24 * 7,
    month: 24 * 30,
    year: 24 * 365,
};

const parseAgeHours = (text?: string | null): number => {
    if (!text) return Number.POSITIVE_INFINITY;
    const match = text.match(
        /(\d+)\s+(second|minute|hour|day|week|month|year)/i,
    );
    if (!match) return Number.POSITIVE_INFINITY;
    return Number.parseInt(match[1], 10) * AGE_UNIT_HOURS[match[2].toLowerCase()];
};

const sortItemsForDisplay = (
    list: YoutubeItem[],
    sort: YoutubeFilters["sort"],
): YoutubeItem[] => {
    if (sort === "views") {
        return [...list].sort(
            (a, b) =>
                parseViewCount(b.viewCountText) - parseViewCount(a.viewCountText),
        );
    }
    if (sort === "date") {
        return [...list].sort(
            (a, b) =>
                parseAgeHours(a.publishedText) - parseAgeHours(b.publishedText),
        );
    }
    return list;
};

export const useYouTubeModule = () => {
    const activeTab = ref<YoutubeTab>("search");
    const query = ref("");
    const submittedQuery = ref("");
    const filters = reactive(defaultFilters());
    const items = ref<YoutubeItem[]>([]);
    const nextCursor = ref<string | null>(null);
    const isLoading = ref(false);
    const isLoadingMore = ref(false);
    const error = ref("");
    const hasSearched = ref(false);

    let requestToken = 0;

    const toBackendFilters = () => ({
        sort: filters.sort,
        uploadDate: filters.uploadDate || null,
        duration: filters.duration || null,
        kind: filters.kind || null,
        hd: filters.hd,
    });

    const runSearch = async (cursor: string | null) => {
        const token = ++requestToken;
        const appending = cursor !== null;
        if (appending) {
            isLoadingMore.value = true;
        } else {
            isLoading.value = true;
            error.value = "";
        }
        try {
            const page = await invoke<YoutubeSearchPage>("youtube_search", {
                payload: {
                    query: submittedQuery.value,
                    filters: toBackendFilters(),
                    cursor,
                },
            });
            if (token !== requestToken) return;
            const merged = appending
                ? [...items.value, ...page.items]
                : page.items;
            items.value = sortItemsForDisplay(merged, filters.sort);
            nextCursor.value = page.nextCursor;
            if (!appending) {
                // Speculatively resolve the top results in the background so
                // clicking one is (near-)instant instead of paying the full
                // yt-dlp extraction on click.
                const topVideos = page.items
                    .filter((item) => item.kind === "video")
                    .slice(0, 4)
                    .map((item) => item.url);
                if (topVideos.length) {
                    invoke("youtube_preresolve", { urls: topVideos }).catch(
                        () => {},
                    );
                }
            }
        } catch (err) {
            if (token !== requestToken) return;
            error.value = String(err).replace(/^Error:\s*/, "").slice(0, 200);
            if (!appending) {
                items.value = [];
                nextCursor.value = null;
            }
        } finally {
            if (token === requestToken) {
                isLoading.value = false;
                isLoadingMore.value = false;
            }
        }
    };

    const search = async () => {
        const trimmed = query.value.trim();
        if (!trimmed) return;
        submittedQuery.value = trimmed;
        hasSearched.value = true;
        await runSearch(null);
    };

    const loadMore = async () => {
        if (!nextCursor.value || isLoading.value || isLoadingMore.value) return;
        await runSearch(nextCursor.value);
    };

    /** Re-run the current search after a filter chip changes. */
    const applyFilters = async () => {
        if (!hasSearched.value) return;
        await runSearch(null);
    };

    return {
        activeTab,
        query,
        submittedQuery,
        filters,
        items,
        nextCursor,
        isLoading,
        isLoadingMore,
        error,
        hasSearched,
        search,
        loadMore,
        applyFilters,
    };
};
