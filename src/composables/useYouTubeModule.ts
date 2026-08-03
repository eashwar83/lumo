import { computed, reactive, ref } from "vue";
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

export type SortOption = {
    label: string;
    token: string;
    selected: boolean;
    kind: "continuation" | "params";
};

export type BrowsePage = {
    title: string;
    subtitle: string;
    avatarUrl?: string | null;
    items: YoutubeItem[];
    nextCursor: string | null;
    sortOptions: SortOption[];
};

/** Client-side ordering for playlist contents (YouTube offers none). */
export type PlaylistSort = "order" | "views" | "longest" | "shortest";

/** Drill-in view opened from a result row (channel or playlist). */
export type BrowseView = {
    kind: "channel" | "playlist";
    target: string;
    tab: "videos" | "playlists";
} | null;

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

    // --- drill-in views (channel / playlist) and trending -------------------
    const browseView = ref<BrowseView>(null);
    const browsePage = ref<BrowsePage | null>(null);
    const isBrowseLoading = ref(false);
    const browseError = ref("");
    const trendingCategory = ref<"now" | "music" | "top100">("now");
    let browseToken = 0;

    const activeSort = ref<SortOption | null>(null);
    // Query typed into the in-view search box: scopes a channel to its own
    // Search tab; filters a playlist's loaded items by title.
    const browseQuery = ref("");
    const appliedChannelQuery = ref("");

    const loadBrowse = async (
        cursor: string | null = null,
        sort: SortOption | null = null,
    ) => {
        const view = browseView.value;
        if (!view) return;
        const token = ++browseToken;
        // A sort reload keeps the current page on screen (its header and
        // chips are the only copy we have — sorted responses omit them).
        const previous = browsePage.value;
        isBrowseLoading.value = !cursor;
        if (!cursor) {
            browseError.value = "";
            if (!sort) browsePage.value = null;
        }
        try {
            const page = await invoke<BrowsePage>(
                view.kind === "channel" ? "youtube_channel" : "youtube_playlist",
                {
                    payload:
                        view.kind === "channel"
                            ? {
                                  target: view.target,
                                  tab: view.tab,
                                  cursor,
                                  sortToken: sort?.token ?? null,
                                  sortKind: sort?.kind ?? null,
                                  query: appliedChannelQuery.value || null,
                              }
                            : { target: view.target, cursor },
                },
            );
            if (token !== browseToken) return;
            if (cursor && previous) {
                // Continuation responses carry no header or sort bar — keep
                // the first page's title/subtitle/avatar/options.
                browsePage.value = {
                    title: previous.title,
                    subtitle: previous.subtitle,
                    avatarUrl: previous.avatarUrl,
                    sortOptions: previous.sortOptions,
                    items: [...previous.items, ...page.items],
                    nextCursor: page.nextCursor,
                };
            } else if (sort && previous) {
                // A sort reload keeps the header and the (unchanged) chips,
                // marking the chosen one active.
                browsePage.value = {
                    ...page,
                    title: previous.title,
                    subtitle: previous.subtitle,
                    avatarUrl: previous.avatarUrl,
                    sortOptions: page.sortOptions.length
                        ? page.sortOptions
                        : previous.sortOptions.map((option) => ({
                              ...option,
                              selected: option.token === sort.token,
                          })),
                };
            } else {
                browsePage.value = page;
            }
        } catch (err) {
            if (token !== browseToken) return;
            browseError.value = String(err)
                .replace(/^Error:\s*/, "")
                .slice(0, 200);
        } finally {
            if (token === browseToken) isBrowseLoading.value = false;
        }
    };

    // Playlist-side ordering and duration filter (client-side by design).
    const playlistSort = ref<PlaylistSort>("order");
    const playlistDuration = ref<"" | "short" | "medium" | "long">("");

    const visibleBrowseItems = computed(() => {
        const page = browsePage.value;
        if (!page) return [];
        if (browseView.value?.kind !== "playlist") return page.items;
        let items = [...page.items];
        const needle = browseQuery.value.trim().toLowerCase();
        if (needle) {
            items = items.filter((item) =>
                item.title.toLowerCase().includes(needle),
            );
        }
        if (playlistDuration.value) {
            items = items.filter((item) => {
                const seconds = item.durationSeconds ?? 0;
                if (!seconds) return false;
                if (playlistDuration.value === "short") return seconds < 240;
                if (playlistDuration.value === "medium")
                    return seconds >= 240 && seconds <= 1200;
                return seconds > 1200;
            });
        }
        if (playlistSort.value === "views") {
            items.sort(
                (a, b) =>
                    parseViewCount(b.viewCountText) -
                    parseViewCount(a.viewCountText),
            );
        } else if (playlistSort.value === "longest") {
            items.sort(
                (a, b) => (b.durationSeconds ?? 0) - (a.durationSeconds ?? 0),
            );
        } else if (playlistSort.value === "shortest") {
            items.sort(
                (a, b) => (a.durationSeconds ?? 0) - (b.durationSeconds ?? 0),
            );
        }
        return items;
    });

    const openChannel = (target: string) => {
        browseView.value = { kind: "channel", target, tab: "videos" };
        activeSort.value = null;
        browseQuery.value = "";
        appliedChannelQuery.value = "";
        void loadBrowse(null);
    };

    const openPlaylist = (target: string) => {
        browseView.value = { kind: "playlist", target, tab: "videos" };
        activeSort.value = null;
        browseQuery.value = "";
        appliedChannelQuery.value = "";
        playlistSort.value = "order";
        playlistDuration.value = "";
        void loadBrowse(null);
    };

    const setChannelTab = (tab: "videos" | "playlists") => {
        if (!browseView.value || browseView.value.tab === tab) return;
        browseView.value = { ...browseView.value, tab };
        activeSort.value = null;
        browseQuery.value = "";
        appliedChannelQuery.value = "";
        void loadBrowse(null);
    };

    /** Runs the channel's own search (playlists filter live instead). */
    const submitBrowseQuery = () => {
        if (browseView.value?.kind !== "channel") return;
        appliedChannelQuery.value = browseQuery.value.trim();
        activeSort.value = null;
        void loadBrowse(null);
    };

    const clearBrowseQuery = () => {
        browseQuery.value = "";
        if (browseView.value?.kind === "channel" && appliedChannelQuery.value) {
            appliedChannelQuery.value = "";
            void loadBrowse(null);
        }
    };

    /** Applies one of YouTube's own sort options (channel surfaces). */
    const setBrowseSort = (option: SortOption) => {
        if (!browseView.value) return;
        activeSort.value = option;
        void loadBrowse(null, option);
    };

    const closeBrowse = (): boolean => {
        if (!browseView.value) return false;
        browseView.value = null;
        browsePage.value = null;
        return true;
    };

    const isBrowseLoadingMore = ref(false);

    const loadMoreBrowse = async () => {
        const cursor = browsePage.value?.nextCursor;
        if (!cursor || isBrowseLoading.value || isBrowseLoadingMore.value) return;
        isBrowseLoadingMore.value = true;
        try {
            await loadBrowse(cursor, activeSort.value);
        } finally {
            isBrowseLoadingMore.value = false;
        }
    };

    const trendingPage = ref<BrowsePage | null>(null);
    const isTrendingLoading = ref(false);
    const trendingError = ref("");

    const loadTrending = async () => {
        isTrendingLoading.value = true;
        trendingError.value = "";
        try {
            trendingPage.value = await invoke<BrowsePage>("youtube_trending", {
                category: trendingCategory.value,
            });
        } catch (err) {
            trendingError.value = String(err)
                .replace(/^Error:\s*/, "")
                .slice(0, 200);
            trendingPage.value = null;
        } finally {
            isTrendingLoading.value = false;
        }
    };

    const setTrendingCategory = (category: "now" | "music" | "top100") => {
        trendingCategory.value = category;
        void loadTrending();
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
        browseView,
        browsePage,
        isBrowseLoading,
        browseError,
        openChannel,
        openPlaylist,
        setChannelTab,
        closeBrowse,
        loadMoreBrowse,
        isBrowseLoadingMore,
        setBrowseSort,
        browseQuery,
        appliedChannelQuery,
        submitBrowseQuery,
        clearBrowseQuery,
        visibleBrowseItems,
        playlistSort,
        playlistDuration,
        trendingPage,
        trendingCategory,
        isTrendingLoading,
        trendingError,
        loadTrending,
        setTrendingCategory,
    };
};
