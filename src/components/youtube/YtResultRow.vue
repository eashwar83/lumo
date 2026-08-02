<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { YoutubeItem } from "../../composables/useYouTubeModule";

const props = defineProps<{
    item: YoutubeItem;
    isFavorite?: boolean;
}>();

// Loaded directly by the webview like a browser would — instant and cached
// by WebView2 itself. On error, fall back to the always-present mqdefault
// frame, then to the placeholder icon.
const thumbFailed = ref(0);
watch(
    () => props.item.id,
    () => {
        thumbFailed.value = 0;
    },
);
const thumbSrc = computed(() => {
    if (thumbFailed.value === 0) return props.item.thumbnailUrl ?? "";
    const fallback = `https://i.ytimg.com/vi/${props.item.id}/mqdefault.jpg`;
    if (thumbFailed.value === 1 && props.item.thumbnailUrl !== fallback) {
        return fallback;
    }
    return "";
});

const emit = defineEmits<{
    (e: "play", item: YoutubeItem): void;
    (e: "open", item: YoutubeItem): void;
    (e: "toggle-heart", item: YoutubeItem): void;
    (e: "download", item: YoutubeItem): void;
}>();

const metaLine = () => {
    const parts: string[] = [];
    if (props.item.channel) parts.push(props.item.channel);
    if (props.item.viewCountText) parts.push(props.item.viewCountText);
    if (props.item.publishedText) parts.push(props.item.publishedText);
    if (props.item.videoCountText) parts.push(props.item.videoCountText);
    return parts.join(" · ");
};

const onActivate = () => {
    if (props.item.kind === "video") {
        emit("play", props.item);
    } else {
        emit("open", props.item);
    }
};
</script>

<template>
    <div
        class="yt-row"
        role="button"
        tabindex="0"
        @click="onActivate"
        @keydown.enter="onActivate"
        @keydown.space.prevent="onActivate"
    >
        <div
            class="yt-row__thumb"
            :class="{ 'yt-row__thumb--round': props.item.kind === 'channel' }"
        >
            <img
                v-if="thumbSrc"
                class="yt-row__thumb-img"
                :src="thumbSrc"
                alt=""
                loading="lazy"
                referrerpolicy="no-referrer"
                @error="thumbFailed += 1"
            />
            <div v-else class="yt-row__thumb-fallback" aria-hidden="true">
                <svg
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.5"
                >
                    <path
                        d="M2.5 17a24.12 24.12 0 0 1 0-10 2 2 0 0 1 1.4-1.4 49.56 49.56 0 0 1 16.2 0A2 2 0 0 1 21.5 7a24.12 24.12 0 0 1 0 10 2 2 0 0 1-1.4 1.4 49.55 49.55 0 0 1-16.2 0A2 2 0 0 1 2.5 17"
                    />
                    <path d="m10 15 5-3-5-3z" />
                </svg>
            </div>
            <span
                v-if="props.item.badge === 'LIVE'"
                class="yt-row__badge yt-row__badge--live"
                >LIVE</span
            >
            <span
                v-else-if="props.item.durationText"
                class="yt-row__badge"
                >{{ props.item.durationText }}</span
            >
            <span
                v-else-if="props.item.kind === 'playlist'"
                class="yt-row__badge"
                >Playlist</span
            >
        </div>

        <div class="yt-row__meta">
            <div class="yt-row__title" :title="props.item.title">
                {{ props.item.title }}
            </div>
            <div class="yt-row__sub">{{ metaLine() }}</div>
        </div>

        <div class="yt-row__actions" @click.stop>
            <button
                class="yt-row__action"
                type="button"
                title="Play"
                :disabled="props.item.kind !== 'video'"
                @click="emit('play', props.item)"
            >
                <svg viewBox="0 0 24 24" fill="currentColor">
                    <path d="M8 5v14l11-7z" />
                </svg>
            </button>
            <button
                class="yt-row__action"
                type="button"
                title="Add to queue (coming soon)"
                disabled
            >
                <svg
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                >
                    <path d="M3 6h12M3 12h12M3 18h8" />
                    <path d="M17 12v6M14 15h6" />
                </svg>
            </button>
            <button
                class="yt-row__action"
                type="button"
                title="Download…"
                :disabled="props.item.kind !== 'video'"
                @click="emit('download', props.item)"
            >
                <svg
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                >
                    <path d="M12 3v12" />
                    <path d="m7 11 5 5 5-5" />
                    <path d="M5 20h14" />
                </svg>
            </button>
            <button
                class="yt-row__action yt-row__action--heart"
                :class="{ 'yt-row__action--heart-on': props.isFavorite }"
                type="button"
                :title="
                    props.isFavorite
                        ? 'Remove from Favourites'
                        : 'Add to Favourites'
                "
                :aria-pressed="props.isFavorite"
                @click="emit('toggle-heart', props.item)"
            >
                <svg
                    viewBox="0 0 24 24"
                    :fill="props.isFavorite ? 'currentColor' : 'none'"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                >
                    <path
                        d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"
                    />
                </svg>
            </button>
        </div>
    </div>
</template>
