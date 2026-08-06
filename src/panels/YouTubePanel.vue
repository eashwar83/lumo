<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { HistoryEntry } from "../types/history";
import YtResultRow from "../components/youtube/YtResultRow.vue";
import YtDownloadDialog from "../components/youtube/YtDownloadDialog.vue";
import {
    useYouTubeDownloads,
    formatEta,
    formatSpeed,
    type DownloadOptions,
} from "../composables/useYouTubeDownloads";
import {
    describeFilters,
    useYouTubeSearchStore,
    type StoredSearch,
} from "../composables/useYouTubeSearchStore";
import { useYouTubeSettings } from "../composables/useYouTubeSettings";
import {
    useYouTubeModule,
    type YoutubeItem,
    type YoutubeTab,
} from "../composables/useYouTubeModule";

const props = defineProps<{
    isVisible: boolean;
    favoritePaths: Set<string>;
    history: HistoryEntry[];
}>();

const YT_URL_PATTERN = /(^|\/\/)(www\.|m\.)?(youtube\.com\/(watch|shorts|live)|youtu\.be\/)/i;

const ytHistory = computed(() =>
    props.history.filter((entry) => YT_URL_PATTERN.test(entry.path)),
);

const historyProgress = (entry: HistoryEntry) => {
    if (!entry.duration || entry.duration <= 0) return 0;
    return Math.min(100, Math.round((entry.lastPosition / entry.duration) * 100));
};

const historyThumb = (entry: HistoryEntry) => {
    const match = entry.path.match(/(?:v=|youtu\.be\/|shorts\/|live\/)([\w-]{11})/);
    return match ? `https://i.ytimg.com/vi/${match[1]}/mqdefault.jpg` : "";
};

const emit = defineEmits<{
    (e: "play-youtube", payload: { url: string; title?: string }): void;
    (e: "notify", message: string): void;
    (
        e: "toggle-youtube-favorite",
        payload: { url: string; title: string; thumbnailUrl?: string | null },
    ): void;
    (e: "open-youtube-settings"): void;
}>();

defineExpose({
    /** Lets Esc back out of a drill-in before closing the panel. */
    closeBrowseView: () => yt.closeBrowse(),
});

const searchStore = useYouTubeSearchStore();
const yt = useYouTubeModule((query, filters) =>
    searchStore.recordSearch(query, filters),
);

const isCurrentSearchSaved = computed(() =>
    searchStore.isSaved(yt.query.value, yt.filters),
);

const onToggleSaveSearch = () => {
    if (!yt.query.value.trim()) return;
    const nowSaved = searchStore.toggleSaved(yt.query.value, yt.filters);
    emit("notify", nowSaved ? "Search saved" : "Search removed");
};

/** Only worth showing once there is something to clear. */
const canClearSearch = computed(
    () =>
        yt.query.value.trim().length > 0 ||
        yt.hasSearched.value ||
        yt.items.value.length > 0,
);

const onClearSearch = () => {
    closeHistory();
    yt.clearSearch();
    searchInputRef.value?.focus();
};

const onRunStoredSearch = (entry: {
    query: string;
    filters: typeof yt.filters;
}) => {
    isHistoryOpen.value = false;
    void yt.applyStoredSearch(entry.query, entry.filters);
};

// --- search history dropdown -----------------------------------------
// Anchored to the input rather than to the results area: the old
// full-width list lived in the "nothing searched yet" branch, so the
// first search hid it for the rest of the session.
const isHistoryOpen = ref(false);
const highlightedIndex = ref(-1);

/** Saved searches first, then recent, narrowed by what is typed. */
const historyEntries = computed(() => {
    const needle = yt.query.value.trim().toLowerCase();
    const match = (entry: StoredSearch) =>
        !needle || entry.query.toLowerCase().includes(needle);
    return [
        ...searchStore.saved.value
            .filter(match)
            .map((entry) => ({ entry, saved: true })),
        ...searchStore.recent.value
            .filter(match)
            .map((entry) => ({ entry, saved: false })),
    ].slice(0, 12);
});

const openHistory = () => {
    highlightedIndex.value = -1;
    isHistoryOpen.value = true;
};

const closeHistory = () => {
    isHistoryOpen.value = false;
    highlightedIndex.value = -1;
};

const onSubmitSearch = () => {
    closeHistory();
    void yt.search();
};

/** Arrow keys walk the list; Enter runs whichever row is highlighted. */
const onSearchKeydown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
        closeHistory();
        return;
    }
    if (!isHistoryOpen.value || !historyEntries.value.length) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        // -1 means no row is highlighted, so Enter searches what was typed.
        const count = historyEntries.value.length;
        const next =
            highlightedIndex.value + (event.key === "ArrowDown" ? 1 : -1);
        highlightedIndex.value =
            next < -1 ? count - 1 : next >= count ? -1 : next;
        return;
    }
    if (event.key === "Enter" && highlightedIndex.value >= 0) {
        event.preventDefault();
        onRunStoredSearch(historyEntries.value[highlightedIndex.value].entry);
    }
};

const removeHistoryEntry = (item: { entry: StoredSearch; saved: boolean }) => {
    if (item.saved) searchStore.removeSaved(item.entry);
    else searchStore.removeRecent(item.entry);
};
const searchInputRef = ref<HTMLInputElement | null>(null);
const listRef = ref<HTMLDivElement | null>(null);

const TABS: { id: YoutubeTab; label: string }[] = [
    { id: "search", label: "Search" },
    { id: "trending", label: "Trending" },
    { id: "history", label: "History" },
    { id: "downloads", label: "Downloads" },
];

let warmedUp = false;

watch(
    () => props.isVisible,
    (visible) => {
        if (!visible) return;
        if (!warmedUp) {
            // Pays the first-use costs (TLS handshake, antivirus scan of
            // yt-dlp) in the background before the first search needs them.
            warmedUp = true;
            invoke("youtube_warmup").catch(() => {});
        }
        if (!yt.hasSearched.value) {
            requestAnimationFrame(() => searchInputRef.value?.focus());
        }
    },
    { immediate: true },
);

const onPlay = (item: YoutubeItem) => {
    emit("play-youtube", { url: item.url, title: item.title });
};

const onOpen = (item: YoutubeItem) => {
    if (item.kind === "channel") {
        yt.openChannel(item.channelUrl || item.url);
    } else if (item.kind === "playlist") {
        yt.openPlaylist(item.url);
    }
};

const onOpenChannel = (item: YoutubeItem) => {
    if (item.channelUrl) yt.openChannel(item.channelUrl);
};

const playAll = (items: YoutubeItem[], shuffle = false) => {
    const videos = items.filter((item) => item.kind === "video");
    if (!videos.length) return;
    const first = shuffle
        ? videos[Math.floor(Math.random() * videos.length)]
        : videos[0];
    emit("play-youtube", { url: first.url, title: first.title });
};

const downloadAll = async (items: YoutubeItem[]) => {
    const videos = items.filter((item) => item.kind === "video").slice(0, 24);
    if (!videos.length) return;
    for (const video of videos) {
        await downloads.add(
            { url: video.url, title: video.title },
            {
                qualityMaxHeight: youtubeSettings.qualityMaxHeight,
                container: "mp4",
                audioOnly: false,
                audioFormat: "mp3",
                embedSubs: false,
                subLangs: "en",
                embedThumbnail: true,
                embedChapters: true,
                front: false,
            },
        );
    }
    emit("notify", `Queued ${videos.length} downloads`);
    yt.activeTab.value = "downloads";
};

const onBrowseScroll = (event: Event) => {
    const list = event.target as HTMLElement;
    if (list.scrollTop + list.clientHeight >= list.scrollHeight * 0.8) {
        yt.loadMoreBrowse();
    }
};

const TRENDING_CATEGORIES = [
    { id: "now" as const, label: "Now" },
    { id: "music" as const, label: "Music" },
    { id: "top100" as const, label: "Top 100" },
];

watch(
    () => yt.activeTab.value,
    (tab) => {
        if (tab === "trending" && !yt.trendingPage.value) {
            void yt.loadTrending();
        }
    },
);

const onListScroll = () => {
    const list = listRef.value;
    if (!list) return;
    if (list.scrollTop + list.clientHeight >= list.scrollHeight * 0.8) {
        void yt.loadMore();
    }
};

const onFilterChange = () => {
    void yt.applyFilters();
};

// Turns raw backend errors into an actionable banner.
const errorBanner = computed(() => {
    const message = yt.error.value;
    if (!message) return null;
    const lower = message.toLowerCase();
    if (
        lower.includes("sign in") ||
        lower.includes("age") ||
        lower.includes("private") ||
        lower.includes("not available in your country")
    ) {
        return {
            text: "Sign-in required — import browser cookies in Settings → YouTube to unlock age- or region-restricted videos.",
            action: "settings" as const,
        };
    }
    if (
        lower.includes("403") ||
        lower.includes("429") ||
        lower.includes("throttl") ||
        lower.includes("too many requests")
    ) {
        return {
            text: "YouTube is rate-limiting or blocking these requests. Updating yt-dlp usually fixes it (Settings → YouTube).",
            action: "settings" as const,
        };
    }
    if (
        lower.includes("dns") ||
        lower.includes("offline") ||
        lower.includes("network") ||
        lower.includes("connect") ||
        lower.includes("timed out")
    ) {
        return {
            text: "Can't reach YouTube — you appear to be offline. Cached results are still available.",
            action: "retry" as const,
        };
    }
    return { text: message, action: "retry" as const };
});

const downloads = useYouTubeDownloads();
const { settings: youtubeSettings } = useYouTubeSettings();
const downloadTarget = ref<YoutubeItem | null>(null);
const isDownloadDialogOpen = ref(false);

const onDownloadRequest = (item: YoutubeItem) => {
    downloadTarget.value = item;
    isDownloadDialogOpen.value = true;
};

const onDownloadConfirm = async (options: DownloadOptions) => {
    const target = downloadTarget.value;
    isDownloadDialogOpen.value = false;
    if (!target) return;
    try {
        await downloads.add({ url: target.url, title: target.title }, options);
        emit("notify", options.front ? "Downloading…" : "Added to queue");
        yt.activeTab.value = "downloads";
    } catch (error) {
        emit(
            "notify",
            String(error).replace(/^Error:\s*/, "").slice(0, 160),
        );
    }
};

const statusLabel = (item: {
    status: string;
    error?: string | null;
    subtitleNote?: string | null;
}) => {
    if (item.error) return item.error;
    switch (item.status) {
        case "queued":
            return "Queued";
        case "downloading":
            return "Downloading";
        case "paused":
            return "Paused";
        case "done":
            return item.subtitleNote
                ? `In Library ✓ · ${item.subtitleNote}`
                : "In Library ✓";
        case "cancelled":
            return "Cancelled";
        default:
            return item.status;
    }
};
</script>

<template>
    <section class="yt panel--youtube" aria-label="YouTube">
        <div class="yt-header">
            <div class="yt-tabs" role="tablist">
                <button
                    v-for="tab in TABS"
                    :key="tab.id"
                    class="yt-tab"
                    :class="{ 'yt-tab--active': yt.activeTab.value === tab.id }"
                    type="button"
                    role="tab"
                    :aria-selected="yt.activeTab.value === tab.id"
                    @click="yt.activeTab.value = tab.id"
                >
                    {{ tab.label }}
                    <span
                        v-if="tab.id === 'downloads' && downloads.activeCount.value"
                        class="yt-tab__badge"
                        >{{ downloads.activeCount.value }}</span
                    >
                </button>
            </div>
        </div>

        <!-- Drill-in: channel or playlist opened from a row -->
        <template v-if="yt.browseView.value">
            <div class="yt-browse-head">
                <button
                    class="yt-chip"
                    type="button"
                    @click="yt.closeBrowse()"
                >
                    ‹ Back to results
                </button>
                <div class="yt-browse-title">
                    <img
                        v-if="yt.browsePage.value?.avatarUrl"
                        class="yt-browse-avatar"
                        :src="yt.browsePage.value.avatarUrl"
                        alt=""
                        referrerpolicy="no-referrer"
                    />
                    <div>
                        <div class="yt-browse-name">
                            {{ yt.browsePage.value?.title || "Loading…" }}
                        </div>
                        <div class="yt-browse-sub">
                            {{ yt.browsePage.value?.subtitle || "" }}
                        </div>
                    </div>
                </div>

                <form
                    class="yt-browse-search"
                    @submit.prevent="yt.submitBrowseQuery()"
                >
                    <input
                        v-model="yt.browseQuery.value"
                        class="yt-browse-search__input"
                        type="text"
                        :placeholder="
                            yt.browseView.value.kind === 'channel'
                                ? 'Search this channel'
                                : 'Filter this playlist'
                        "
                        spellcheck="false"
                        autocomplete="off"
                    />
                    <button
                        v-if="yt.browseQuery.value"
                        class="yt-browse-search__clear"
                        type="button"
                        title="Clear"
                        @click="yt.clearBrowseQuery()"
                    >
                        ✕
                    </button>
                </form>
            </div>

            <div class="yt-chips">
                <template v-if="yt.appliedChannelQuery.value">
                    <span class="yt-chip yt-chip--on">
                        Results for “{{ yt.appliedChannelQuery.value }}”
                    </span>
                    <button
                        class="yt-chip"
                        type="button"
                        @click="yt.clearBrowseQuery()"
                    >
                        Clear search
                    </button>
                </template>
                <template v-else-if="yt.browseView.value.kind === 'channel'">
                    <button
                        class="yt-chip"
                        :class="{
                            'yt-chip--on': yt.browseView.value.tab === 'videos',
                        }"
                        type="button"
                        @click="yt.setChannelTab('videos')"
                    >
                        Videos
                    </button>
                    <button
                        class="yt-chip"
                        :class="{
                            'yt-chip--on':
                                yt.browseView.value.tab === 'playlists',
                        }"
                        type="button"
                        @click="yt.setChannelTab('playlists')"
                    >
                        Playlists
                    </button>
                    <span
                        v-if="yt.browsePage.value?.sortOptions.length"
                        class="yt-chips__divider"
                    ></span>
                    <button
                        v-for="option in yt.browsePage.value?.sortOptions || []"
                        :key="option.token"
                        class="yt-chip"
                        :class="{ 'yt-chip--on': option.selected }"
                        type="button"
                        @click="yt.setBrowseSort(option)"
                    >
                        {{ option.label }}
                    </button>
                </template>
                <template v-else>
                    <button
                        class="yt-chip"
                        type="button"
                        @click="playAll(yt.browsePage.value?.items || [])"
                    >
                        ▶ Play all
                    </button>
                    <button
                        class="yt-chip"
                        type="button"
                        @click="playAll(yt.browsePage.value?.items || [], true)"
                    >
                        ⤨ Shuffle
                    </button>
                    <button
                        class="yt-chip"
                        type="button"
                        @click="downloadAll(yt.browsePage.value?.items || [])"
                    >
                        ⤓ Download all
                    </button>
                    <span class="yt-chips__divider"></span>
                    <label class="yt-chip">
                        <span class="yt-chip__label">Sort</span>
                        <select
                            v-model="yt.playlistSort.value"
                            class="yt-chip__select"
                        >
                            <option value="order">Playlist order</option>
                            <option value="views">Most viewed</option>
                            <option value="longest">Longest</option>
                            <option value="shortest">Shortest</option>
                        </select>
                    </label>
                    <label class="yt-chip">
                        <span class="yt-chip__label">Duration</span>
                        <select
                            v-model="yt.playlistDuration.value"
                            class="yt-chip__select"
                        >
                            <option value="">Any</option>
                            <option value="short">Under 4 min</option>
                            <option value="medium">4–20 min</option>
                            <option value="long">Over 20 min</option>
                        </select>
                    </label>
                </template>
            </div>

            <div class="yt-list" @scroll.passive="onBrowseScroll">
                <div v-if="yt.browseError.value" class="yt-state yt-state--error">
                    {{ yt.browseError.value }}
                </div>
                <div v-else-if="yt.isBrowseLoading.value" class="yt-state">
                    Loading…
                </div>
                <template v-else-if="yt.visibleBrowseItems.value.length">
                    <YtResultRow
                        v-for="item in yt.visibleBrowseItems.value"
                        :key="`${item.kind}:${item.id}`"
                        :item="item"
                        :is-favorite="props.favoritePaths.has(item.url)"
                        @play="onPlay"
                        @open="onOpen"
                        @toggle-heart="
                            emit('toggle-youtube-favorite', {
                                url: $event.url,
                                title: $event.title,
                                thumbnailUrl: $event.thumbnailUrl,
                            })
                        "
                        @download="onDownloadRequest"
                        @open-channel="onOpenChannel"
                    />
                    <button
                        v-if="yt.browsePage.value?.nextCursor"
                        class="yt-loadmore"
                        type="button"
                        :disabled="yt.isBrowseLoadingMore.value"
                        @click="yt.loadMoreBrowse()"
                    >
                        {{
                            yt.isBrowseLoadingMore.value
                                ? "Loading…"
                                : "Load more"
                        }}
                    </button>
                </template>
                <div v-else class="yt-state">Nothing to show here</div>
            </div>
        </template>

        <template v-else-if="yt.activeTab.value === 'search'">
            <form class="yt-search" @submit.prevent="onSubmitSearch">
                <div class="yt-search__field">
                    <input
                        ref="searchInputRef"
                        v-model="yt.query.value"
                        class="yt-search__input"
                        type="text"
                        placeholder="Search YouTube..."
                        spellcheck="false"
                        autocomplete="off"
                        @focus="openHistory"
                        @input="openHistory"
                        @blur="closeHistory"
                        @keydown="onSearchKeydown"
                    />
                    <div
                        v-if="isHistoryOpen && historyEntries.length"
                        class="yt-history"
                    >
                        <div
                            v-for="(item, index) in historyEntries"
                            :key="`${item.saved ? 'saved' : 'recent'}-${item.entry.at}`"
                            class="yt-history__row"
                            :class="{
                                'yt-history__row--on':
                                    index === highlightedIndex,
                                'yt-history__row--divide':
                                    index > 0 &&
                                    historyEntries[index - 1].saved &&
                                    !item.saved,
                            }"
                            role="button"
                            tabindex="-1"
                            @mousedown.prevent="onRunStoredSearch(item.entry)"
                        >
                            <svg
                                v-if="item.saved"
                                class="yt-history__icon yt-history__icon--saved"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                                aria-hidden="true"
                            >
                                <path d="m12 3 2.9 5.9 6.5.9-4.7 4.6 1.1 6.5L12 17.8 6.2 20.9l1.1-6.5L2.6 9.8l6.5-.9z" />
                            </svg>
                            <svg
                                v-else
                                class="yt-history__icon"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                aria-hidden="true"
                            >
                                <circle cx="12" cy="12" r="9" />
                                <path d="M12 7v6l4 2" />
                            </svg>
                            <span class="yt-history__query">{{
                                item.entry.query
                            }}</span>
                            <span class="yt-history__filters">{{
                                describeFilters(item.entry.filters)
                            }}</span>
                            <button
                                class="yt-history__remove"
                                type="button"
                                :title="
                                    item.saved
                                        ? 'Remove saved search'
                                        : 'Remove from history'
                                "
                                @mousedown.stop.prevent="
                                    removeHistoryEntry(item)
                                "
                            >
                                ✕
                            </button>
                        </div>
                        <button
                            v-if="searchStore.recent.value.length"
                            class="yt-history__clear"
                            type="button"
                            @mousedown.prevent="searchStore.clearRecent()"
                        >
                            Clear search history
                        </button>
                    </div>
                    <button
                        class="yt-search__save"
                        :class="{ 'yt-search__save--on': isCurrentSearchSaved }"
                        type="button"
                        :title="
                            isCurrentSearchSaved
                                ? 'Remove saved search'
                                : 'Save this search (with filters)'
                        "
                        :disabled="!yt.query.value.trim()"
                        @click="onToggleSaveSearch"
                    >
                        <svg
                            viewBox="0 0 24 24"
                            :fill="isCurrentSearchSaved ? 'currentColor' : 'none'"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linejoin="round"
                        >
                            <path d="m12 3 2.9 5.9 6.5.9-4.7 4.6 1.1 6.5L12 17.8 6.2 20.9l1.1-6.5L2.6 9.8l6.5-.9z" />
                        </svg>
                    </button>
                </div>
                <button
                    class="yt-search__btn"
                    type="submit"
                    :disabled="yt.isLoading.value"
                >
                    Search
                </button>
            </form>

            <div class="yt-chips">
                <label class="yt-chip">
                    <span class="yt-chip__label">Sort</span>
                    <select
                        v-model="yt.filters.sort"
                        class="yt-chip__select"
                        @change="onFilterChange"
                    >
                        <option value="relevance">Relevance</option>
                        <option value="date">Upload date</option>
                        <option value="views">View count</option>
                        <option value="rating">Rating</option>
                    </select>
                </label>
                <label class="yt-chip">
                    <span class="yt-chip__label">Duration</span>
                    <select
                        v-model="yt.filters.duration"
                        class="yt-chip__select"
                        @change="onFilterChange"
                    >
                        <option value="">Any</option>
                        <option value="short">Under 4 min</option>
                        <option value="medium">4–20 min</option>
                        <option value="long">Over 20 min</option>
                    </select>
                </label>
                <label class="yt-chip">
                    <span class="yt-chip__label">Uploaded</span>
                    <select
                        v-model="yt.filters.uploadDate"
                        class="yt-chip__select"
                        @change="onFilterChange"
                    >
                        <option value="">Any time</option>
                        <option value="hour">Last hour</option>
                        <option value="today">Today</option>
                        <option value="week">This week</option>
                        <option value="month">This month</option>
                        <option value="year">This year</option>
                    </select>
                </label>
                <label class="yt-chip">
                    <span class="yt-chip__label">Type</span>
                    <select
                        v-model="yt.filters.kind"
                        class="yt-chip__select"
                        @change="onFilterChange"
                    >
                        <option value="">All</option>
                        <option value="video">Videos</option>
                        <option value="channel">Channels</option>
                        <option value="playlist">Playlists</option>
                        <option value="movie">Films</option>
                    </select>
                </label>
                <button
                    class="yt-chip yt-chip--toggle"
                    :class="{ 'yt-chip--on': yt.filters.hd }"
                    type="button"
                    :aria-pressed="yt.filters.hd"
                    @click="
                        yt.filters.hd = !yt.filters.hd;
                        onFilterChange();
                    "
                >
                    HD
                </button>
                <!-- Sits at the end of the chip row so it lines up under
                     the Search button without making the panel taller. -->
                <button
                    v-if="canClearSearch"
                    class="yt-search__clear"
                    type="button"
                    title="Clear the query, results and filters"
                    @click="onClearSearch"
                >
                    Clear
                </button>
            </div>

            <div
                ref="listRef"
                class="yt-list"
                @scroll.passive="onListScroll"
            >
                <div v-if="errorBanner" class="yt-banner">
                    <span class="yt-banner__text">{{ errorBanner.text }}</span>
                    <button
                        v-if="errorBanner.action === 'settings'"
                        class="yt-chip"
                        type="button"
                        @click="emit('open-youtube-settings')"
                    >
                        Open settings
                    </button>
                    <button
                        v-else
                        class="yt-chip"
                        type="button"
                        @click="yt.search()"
                    >
                        Try again
                    </button>
                </div>
                <div
                    v-else-if="yt.isLoading.value"
                    class="yt-state"
                >
                    Searching…
                </div>
                <template v-else-if="yt.items.value.length">
                    <YtResultRow
                        v-for="item in yt.items.value"
                        :key="`${item.kind}:${item.id}`"
                        :item="item"
                        :is-favorite="props.favoritePaths.has(item.url)"
                        @play="onPlay"
                        @open="onOpen"
                        @toggle-heart="
                            emit('toggle-youtube-favorite', {
                                url: $event.url,
                                title: $event.title,
                                thumbnailUrl: $event.thumbnailUrl,
                            })
                        "
                        @download="onDownloadRequest"
                        @open-channel="onOpenChannel"
                    />
                    <div
                        v-if="yt.isLoadingMore.value"
                        class="yt-state yt-state--more"
                    >
                        Loading more…
                    </div>
                    <div
                        v-else-if="!yt.nextCursor.value"
                        class="yt-state yt-state--more"
                    >
                        End of results
                    </div>
                </template>
                <div
                    v-else-if="yt.hasSearched.value"
                    class="yt-state"
                >
                    No results for “{{ yt.submittedQuery.value }}”
                </div>
                <template v-else>
                    <div class="yt-state yt-state--hint">
                        <svg
                            class="yt-state__icon"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.5"
                            aria-hidden="true"
                        >
                            <path
                                d="M2.5 17a24.12 24.12 0 0 1 0-10 2 2 0 0 1 1.4-1.4 49.56 49.56 0 0 1 16.2 0A2 2 0 0 1 21.5 7a24.12 24.12 0 0 1 0 10 2 2 0 0 1-1.4 1.4 49.55 49.55 0 0 1-16.2 0A2 2 0 0 1 2.5 17"
                            />
                            <path d="m10 15 5-3-5-3z" />
                        </svg>
                        Search YouTube — results play right in Lumo
                    </div>
                </template>
            </div>
        </template>

        <div
            v-else-if="yt.activeTab.value === 'history'"
            class="yt-list yt-list--history"
        >
            <div v-if="!ytHistory.length" class="yt-state">
                Videos you watch appear here — history stays on this device
            </div>
            <div
                v-for="entry in ytHistory"
                :key="entry.path"
                class="yt-row"
                role="button"
                tabindex="0"
                @click="
                    emit('play-youtube', {
                        url: entry.path,
                        title: entry.title?.trim() || undefined,
                    })
                "
                @keydown.enter="
                    emit('play-youtube', {
                        url: entry.path,
                        title: entry.title?.trim() || undefined,
                    })
                "
            >
                <div class="yt-row__thumb">
                    <img
                        v-if="historyThumb(entry)"
                        class="yt-row__thumb-img"
                        :src="historyThumb(entry)"
                        alt=""
                        loading="lazy"
                        referrerpolicy="no-referrer"
                    />
                    <span
                        v-if="historyProgress(entry) > 0"
                        class="yt-row__resume"
                        :style="{ width: historyProgress(entry) + '%' }"
                    ></span>
                </div>
                <div class="yt-row__meta">
                    <div class="yt-row__title">
                        {{ entry.title?.trim() || entry.path }}
                    </div>
                    <div class="yt-row__sub">
                        {{
                            historyProgress(entry) > 0
                                ? `${historyProgress(entry)}% watched`
                                : "Started"
                        }}
                    </div>
                </div>
            </div>
        </div>

        <template v-else-if="yt.activeTab.value === 'trending'">
            <div class="yt-chips">
                <button
                    v-for="category in TRENDING_CATEGORIES"
                    :key="category.id"
                    class="yt-chip"
                    :class="{
                        'yt-chip--on':
                            yt.trendingCategory.value === category.id,
                    }"
                    type="button"
                    @click="yt.setTrendingCategory(category.id)"
                >
                    {{ category.label }}
                </button>
            </div>
            <div class="yt-list">
                <div
                    v-if="yt.trendingError.value"
                    class="yt-state yt-state--error"
                >
                    {{ yt.trendingError.value }}
                </div>
                <div v-else-if="yt.isTrendingLoading.value" class="yt-state">
                    Loading charts…
                </div>
                <template v-else-if="yt.trendingPage.value?.items.length">
                    <YtResultRow
                        v-for="item in yt.trendingPage.value.items"
                        :key="`${item.kind}:${item.id}`"
                        :item="item"
                        :is-favorite="props.favoritePaths.has(item.url)"
                        @play="onPlay"
                        @open="onOpen"
                        @toggle-heart="
                            emit('toggle-youtube-favorite', {
                                url: $event.url,
                                title: $event.title,
                                thumbnailUrl: $event.thumbnailUrl,
                            })
                        "
                        @download="onDownloadRequest"
                        @open-channel="onOpenChannel"
                    />
                </template>
                <div v-else class="yt-state">No chart data right now</div>
            </div>
        </template>

        <template v-else-if="yt.activeTab.value === 'downloads'">
            <div class="yt-dl-toolbar">
                <span class="yt-dl-toolbar__count">
                    {{ downloads.items.value.length }} item{{
                        downloads.items.value.length === 1 ? "" : "s"
                    }}
                    · {{ downloads.activeCount.value }} active
                </span>
                <div class="yt-dl-toolbar__actions">
                    <button
                        class="yt-chip"
                        type="button"
                        @click="downloads.openFolder()"
                    >
                        Open folder
                    </button>
                    <button
                        class="yt-chip"
                        type="button"
                        @click="downloads.clearDone()"
                    >
                        Clear finished
                    </button>
                </div>
            </div>
            <div class="yt-list">
                <div v-if="!downloads.items.value.length" class="yt-state">
                    Downloads you start appear here
                </div>
                <div
                    v-for="item in downloads.items.value"
                    :key="item.id"
                    class="yt-row yt-row--download"
                    :class="{ 'yt-row--failed': item.status === 'failed' }"
                >
                    <div class="yt-row__meta">
                        <div class="yt-row__title">{{ item.title }}</div>
                        <div class="yt-dl-progress">
                            <span
                                class="yt-dl-progress__fill"
                                :style="{
                                    width: item.progressPercent + '%',
                                }"
                            ></span>
                        </div>
                        <div class="yt-row__sub">
                            {{ statusLabel(item) }}
                            <span v-if="item.status === 'downloading'">
                                · {{ Math.round(item.progressPercent) }}%
                                <span v-if="formatSpeed(item.speedBps)">
                                    · {{ formatSpeed(item.speedBps) }}</span
                                >
                                <span v-if="formatEta(item.etaSeconds)">
                                    · {{ formatEta(item.etaSeconds) }}</span
                                >
                            </span>
                        </div>
                    </div>
                    <div class="yt-row__actions">
                        <button
                            v-if="
                                item.status === 'downloading' ||
                                item.status === 'queued'
                            "
                            class="yt-chip"
                            type="button"
                            @click="downloads.pause(item.id)"
                        >
                            Pause
                        </button>
                        <button
                            v-else-if="
                                item.status === 'paused' ||
                                item.status === 'failed'
                            "
                            class="yt-chip"
                            type="button"
                            @click="downloads.resume(item.id)"
                        >
                            Resume
                        </button>
                        <button
                            v-if="item.status === 'done'"
                            class="yt-chip"
                            type="button"
                            @click="
                                emit('play-youtube', {
                                    url: item.filePath || item.url,
                                    title: item.title,
                                })
                            "
                        >
                            Play
                        </button>
                        <button
                            class="yt-chip"
                            type="button"
                            @click="downloads.remove(item.id)"
                        >
                            ✕
                        </button>
                    </div>
                </div>
            </div>
        </template>


        <YtDownloadDialog
            :open="isDownloadDialogOpen"
            :item="downloadTarget"
            :queue-ahead="downloads.activeCount.value"
            @close="isDownloadDialogOpen = false"
            @confirm="onDownloadConfirm"
        />
    </section>
</template>

<style src="../styles/youtube-panel.css"></style>
