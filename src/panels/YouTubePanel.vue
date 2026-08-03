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
}>();

const yt = useYouTubeModule();
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
                qualityMaxHeight: 1080,
                container: "mp4",
                audioOnly: false,
                audioFormat: "mp3",
                embedSubs: false,
                subLangs: "en.*,-live_chat",
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

const downloads = useYouTubeDownloads();
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

const statusLabel = (item: { status: string; error?: string | null }) => {
    if (item.error) return item.error;
    switch (item.status) {
        case "queued":
            return "Queued";
        case "downloading":
            return "Downloading";
        case "paused":
            return "Paused";
        case "done":
            return "In Library ✓";
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
            <form class="yt-search" @submit.prevent="yt.search()">
                <input
                    ref="searchInputRef"
                    v-model="yt.query.value"
                    class="yt-search__input"
                    type="text"
                    placeholder="Search YouTube..."
                    spellcheck="false"
                    autocomplete="off"
                />
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
            </div>

            <div
                ref="listRef"
                class="yt-list"
                @scroll.passive="onListScroll"
            >
                <div v-if="yt.error.value" class="yt-state yt-state--error">
                    {{ yt.error.value }}
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
                <div v-else class="yt-state yt-state--hint">
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
