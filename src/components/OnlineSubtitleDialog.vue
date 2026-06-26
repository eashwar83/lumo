<script setup lang="ts">
import { computed } from "vue";
import type { OnlineSubtitleSearchResult } from "../composables/useMediaTracks";

const props = defineProps<{
    open: boolean;
    results: OnlineSubtitleSearchResult[];
    loading: boolean;
    applying: boolean;
    errorMessage: string;
}>();

const emit = defineEmits<{
    (e: "close"): void;
    (e: "select", result: OnlineSubtitleSearchResult): void;
}>();

const resultTitle = computed(() => {
    if (props.loading) return "Searching subtitles...";
    const count = props.results.length;
    return `${count} Subtitle${count === 1 ? "" : "s"} Found.`;
});

const formatDownloads = (downloads?: number | null) => {
    if (downloads === null || downloads === undefined) return "";
    return `${downloads.toLocaleString()} downloads`;
};
</script>

<template>
    <Teleport to="body">
        <div v-if="props.open" class="online-subtitle-dialog" @keydown.esc="emit('close')">
            <div class="online-subtitle-dialog__backdrop" @click="emit('close')"></div>
            <div class="online-subtitle-dialog__panel ui-surface" role="dialog" aria-modal="true">
                <div class="online-subtitle-dialog__header">
                    <div class="online-subtitle-dialog__heading">
                        <div class="online-subtitle-dialog__title">{{ resultTitle }}</div>
                        <div
                            v-if="!props.loading && props.results.length"
                            class="online-subtitle-dialog__subtitle"
                        >
                            Select a subtitle to download and load.
                        </div>
                    </div>
                    <button
                        class="online-subtitle-dialog__close"
                        type="button"
                        :disabled="props.applying"
                        aria-label="Close"
                        @click="emit('close')"
                    >
                        <svg viewBox="0 0 24 24" aria-hidden="true">
                            <path d="M6 6l12 12M18 6 6 18" />
                        </svg>
                    </button>
                </div>

                <div v-if="props.loading" class="online-subtitle-dialog__state">
                    Searching OpenSubtitles...
                </div>
                <div v-else-if="props.errorMessage" class="online-subtitle-dialog__error">
                    {{ props.errorMessage }}
                </div>
                <div v-else-if="!props.results.length" class="online-subtitle-dialog__state">
                    No subtitles found.
                </div>

                <div v-else class="online-subtitle-dialog__list">
                    <button
                        v-for="result in props.results"
                        :key="result.id"
                        class="online-subtitle-dialog__item"
                        type="button"
                        :disabled="props.applying"
                        @click="emit('select', result)"
                    >
                        <span class="online-subtitle-dialog__item-main">
                            <span class="online-subtitle-dialog__item-title">
                                {{ result.title }}
                            </span>
                            <span class="online-subtitle-dialog__item-file">
                                {{ result.fileName }}
                            </span>
                        </span>
                        <span class="online-subtitle-dialog__item-meta">
                            <span>{{ result.language }}</span>
                            <span v-if="formatDownloads(result.downloads)">
                                {{ formatDownloads(result.downloads) }}
                            </span>
                        </span>
                    </button>
                </div>

                <div v-if="props.applying" class="online-subtitle-dialog__applying">
                    Loading subtitle...
                </div>
            </div>
        </div>
    </Teleport>
</template>

<style scoped>
.online-subtitle-dialog {
    position: fixed;
    inset: 0;
    z-index: 230;
}

.online-subtitle-dialog__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.62);
    backdrop-filter: blur(2px);
}

.online-subtitle-dialog__panel {
    position: absolute;
    top: 50%;
    left: 50%;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    width: min(640px, calc(100% - 36px));
    max-height: min(680px, calc(100% - 36px));
    overflow: hidden;
    transform: translate(-50%, -50%);
    border: 1px solid var(--glass-border);
    border-radius: 16px;
    color: #f6f6f6;
}

.online-subtitle-dialog__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 16px 16px 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.online-subtitle-dialog__heading {
    min-width: 0;
}

.online-subtitle-dialog__title {
    font-size: 16px;
    font-weight: 650;
}

.online-subtitle-dialog__subtitle {
    margin-top: 5px;
    color: rgba(255, 255, 255, 0.62);
    font-size: 12px;
}

.online-subtitle-dialog__close {
    display: grid;
    width: 30px;
    height: 30px;
    place-items: center;
    border: none;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.82);
}

.online-subtitle-dialog__close svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
}

.online-subtitle-dialog__close:disabled {
    opacity: 0.48;
}

.online-subtitle-dialog__list {
    display: grid;
    min-height: 0;
    gap: 8px;
    overflow: auto;
    padding: 12px;
    overscroll-behavior: contain;
}

.online-subtitle-dialog__item {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 14px;
    align-items: center;
    width: 100%;
    min-height: 68px;
    padding: 11px 12px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.055);
    color: inherit;
    text-align: left;
}

.online-subtitle-dialog__item:hover:not(:disabled) {
    border-color: rgba(116, 166, 255, 0.44);
    background: rgba(116, 166, 255, 0.14);
}

.online-subtitle-dialog__item:disabled {
    opacity: 0.62;
}

.online-subtitle-dialog__item-main {
    min-width: 0;
}

.online-subtitle-dialog__item-title,
.online-subtitle-dialog__item-file {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.online-subtitle-dialog__item-title {
    font-size: 13px;
    font-weight: 620;
}

.online-subtitle-dialog__item-file {
    margin-top: 5px;
    color: rgba(255, 255, 255, 0.56);
    font-size: 12px;
}

.online-subtitle-dialog__item-meta {
    display: grid;
    gap: 4px;
    justify-items: end;
    color: rgba(255, 255, 255, 0.62);
    font-size: 11px;
    white-space: nowrap;
}

.online-subtitle-dialog__state,
.online-subtitle-dialog__error,
.online-subtitle-dialog__applying {
    padding: 20px 16px;
    color: rgba(255, 255, 255, 0.72);
    font-size: 13px;
}

.online-subtitle-dialog__error {
    color: rgba(255, 127, 127, 0.96);
}

.online-subtitle-dialog__applying {
    border-top: 1px solid rgba(255, 255, 255, 0.1);
}
</style>
