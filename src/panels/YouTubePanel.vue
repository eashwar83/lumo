<script setup lang="ts">
import { ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import YtResultRow from "../components/youtube/YtResultRow.vue";
import {
    useYouTubeModule,
    type YoutubeItem,
    type YoutubeTab,
} from "../composables/useYouTubeModule";

const props = defineProps<{
    isVisible: boolean;
}>();

const emit = defineEmits<{
    (e: "play-youtube", payload: { url: string; title?: string }): void;
    (e: "notify", message: string): void;
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
    const label = item.kind === "channel" ? "Channel" : "Playlist";
    emit("notify", `${label} view arrives in a later milestone`);
};

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
                </button>
            </div>
        </div>

        <template v-if="yt.activeTab.value === 'search'">
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
                        @play="onPlay"
                        @open="onOpen"
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

        <div v-else class="yt-state yt-state--placeholder">
            {{
                yt.activeTab.value === "trending"
                    ? "Trending"
                    : yt.activeTab.value === "history"
                      ? "Watch history"
                      : "Downloads"
            }}
            arrives in a later milestone
        </div>
    </section>
</template>

<style src="../styles/youtube-panel.css"></style>
