<script setup lang="ts">
import { ref, watch } from "vue";
import type { PlaylistEntry } from "../types/playlist";
import { getPathDisplayName } from "../utils/getPathDisplayName";
import { readImageDataUrl } from "../utils/readImageDataUrl";

const props = defineProps<{
    favorites: PlaylistEntry[];
}>();

const emit = defineEmits<{
    (e: "play", entry: PlaylistEntry): void;
    (e: "remove", entry: PlaylistEntry): void;
    (e: "clear"): void;
}>();

// path -> resolved <img> src (remote URL as-is, or data URL for local files)
const thumbs = ref<Record<string, string>>({});

const isRemote = (url?: string): boolean => !!url && /^https?:\/\//i.test(url);

const resolveThumb = async (entry: PlaylistEntry) => {
    const icon = entry.iconUrl?.trim();
    if (!icon || thumbs.value[entry.path]) return;
    if (isRemote(icon)) {
        thumbs.value = { ...thumbs.value, [entry.path]: icon };
        return;
    }
    const dataUrl = await readImageDataUrl(icon);
    if (dataUrl) {
        thumbs.value = { ...thumbs.value, [entry.path]: dataUrl };
    }
};

watch(
    () => props.favorites,
    (entries) => {
        entries.forEach((entry) => void resolveThumb(entry));
    },
    { immediate: true, deep: true },
);

const displayName = (entry: PlaylistEntry): string =>
    entry.title?.trim() || getPathDisplayName(entry.path);

const thumbFor = (entry: PlaylistEntry): string | null =>
    thumbs.value[entry.path] ?? null;
</script>

<template>
    <div class="favorites panel panel--favorites">
        <div class="panel__header">
            <div class="panel__title">
                Favourites
                <span v-if="props.favorites.length" class="favorites__count">
                    {{ props.favorites.length }}
                </span>
            </div>
            <button
                v-if="props.favorites.length"
                class="panel__reset"
                type="button"
                @click.stop="emit('clear')"
            >
                Clear
            </button>
        </div>

        <div class="favorites__content">
            <div v-if="!props.favorites.length" class="panel__empty">
                <svg
                    class="favorites__empty-icon"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.6"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    aria-hidden="true"
                >
                    <path
                        d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"
                    />
                </svg>
                <div class="panel__empty-title">No favourites yet</div>
                <div class="panel__empty-body">
                    Tap the heart while watching a video to add it here.
                </div>
            </div>

            <div v-else class="favorites__grid">
                <div
                    v-for="entry in props.favorites"
                    :key="entry.path"
                    class="favorites__card"
                    role="button"
                    tabindex="0"
                    :title="displayName(entry)"
                    @click="emit('play', entry)"
                    @keydown.enter="emit('play', entry)"
                    @keydown.space.prevent="emit('play', entry)"
                >
                    <div class="favorites__thumb">
                        <img
                            v-if="thumbFor(entry)"
                            class="favorites__thumb-img"
                            :src="thumbFor(entry) ?? ''"
                            :alt="displayName(entry)"
                            loading="lazy"
                            draggable="false"
                        />
                        <div v-else class="favorites__thumb-placeholder">
                            <svg
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1.5"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <path d="M8 5v14l11-7z" fill="currentColor" stroke="none" />
                            </svg>
                        </div>
                        <div class="favorites__play-overlay">
                            <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                                <path d="M8 5v14l11-7z" />
                            </svg>
                        </div>
                        <button
                            class="favorites__remove"
                            type="button"
                            aria-label="Remove from favourites"
                            title="Remove from favourites"
                            @click.stop="emit('remove', entry)"
                            @keydown.enter.stop
                            @keydown.space.prevent.stop
                        >
                            <svg
                                viewBox="0 0 24 24"
                                fill="currentColor"
                                aria-hidden="true"
                            >
                                <path
                                    d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"
                                />
                            </svg>
                        </button>
                    </div>
                    <div class="favorites__name">{{ displayName(entry) }}</div>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped src="../styles/panels.css"></style>

<style scoped>
.favorites {
    display: flex;
    flex-direction: column;
    gap: 12px;
    pointer-events: auto;
    cursor: default;
    user-select: none;
    -webkit-user-select: none;
    overflow: hidden;
}

.favorites__count {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    margin-left: 8px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.1);
    font-size: 11px;
    font-weight: 600;
    vertical-align: middle;
}

.favorites__content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
}

.favorites__empty-icon {
    width: 40px;
    height: 40px;
    margin: 0 auto 10px;
    color: #e0568a;
    opacity: 0.85;
}

.favorites__grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 14px;
    padding: 6px 6px 12px;
    scrollbar-width: thin;
}

.favorites__card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border-radius: 12px;
    cursor: pointer;
    transition: transform 0.12s ease;
}

.favorites__card:hover {
    transform: translateY(-2px);
}

.favorites__card:focus-visible {
    outline: 2px solid rgba(57, 108, 216, 0.6);
    outline-offset: 3px;
    border-radius: 12px;
}

.favorites__thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: 10px;
    overflow: hidden;
    background: rgba(0, 0, 0, 0.14);
    border: 1px solid rgba(0, 0, 0, 0.1);
}

.favorites__thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
}

.favorites__thumb-placeholder {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: rgba(0, 0, 0, 0.3);
}

.favorites__thumb-placeholder svg {
    width: 34px;
    height: 34px;
}

.favorites__play-overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.34);
    opacity: 0;
    transition: opacity 0.15s ease;
    color: #fff;
}

.favorites__play-overlay svg {
    width: 40px;
    height: 40px;
    filter: drop-shadow(0 2px 6px rgba(0, 0, 0, 0.5));
}

.favorites__card:hover .favorites__play-overlay,
.favorites__card:focus-visible .favorites__play-overlay {
    opacity: 1;
}

.favorites__remove {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 30px;
    height: 30px;
    border: none;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.5);
    color: #ff5b8a;
    display: grid;
    place-items: center;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s ease, background-color 0.15s ease,
        transform 0.12s ease;
}

.favorites__remove svg {
    width: 17px;
    height: 17px;
}

.favorites__card:hover .favorites__remove,
.favorites__remove:focus-visible {
    opacity: 1;
}

.favorites__remove:hover {
    background: rgba(0, 0, 0, 0.72);
    transform: scale(1.05);
}

.favorites__name {
    font-size: 12.5px;
    line-height: 1.35;
    color: var(--text-color);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    padding: 0 2px;
}

:root[data-theme="light"] .favorites__count {
    background: rgba(0, 0, 0, 0.08);
}

@media (prefers-color-scheme: dark) {
    :root:not([data-theme]) .favorites__count {
        background: rgba(255, 255, 255, 0.14);
    }
    :root:not([data-theme]) .favorites__thumb {
        background: rgba(255, 255, 255, 0.06);
        border-color: rgba(255, 255, 255, 0.1);
    }
    :root:not([data-theme]) .favorites__thumb-placeholder {
        color: rgba(255, 255, 255, 0.32);
    }
}

:root:is([data-theme="dark"], [data-theme="graphite"]) .favorites__count {
    background: rgba(255, 255, 255, 0.14);
}

:root:is([data-theme="dark"], [data-theme="graphite"]) .favorites__thumb {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.1);
}

:root:is([data-theme="dark"], [data-theme="graphite"])
    .favorites__thumb-placeholder {
    color: rgba(255, 255, 255, 0.32);
}
</style>
