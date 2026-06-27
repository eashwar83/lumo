<script setup lang="ts">
import type {
    OnlineSubtitleProviderId,
    OnlineSubtitleProviderTab,
    OnlineSubtitleSearchResult,
} from "../composables/useMediaTracks";

const props = defineProps<{
    open: boolean;
    providerTabs: OnlineSubtitleProviderTab[];
    activeProviderId: OnlineSubtitleProviderId;
    results: OnlineSubtitleSearchResult[];
    loading: boolean;
    applying: boolean;
    errorMessage: string;
}>();

const emit = defineEmits<{
    (e: "close"): void;
    (e: "providerChange", providerId: OnlineSubtitleProviderId): void;
    (e: "select", result: OnlineSubtitleSearchResult): void;
}>();

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
                    <div
                        class="online-subtitle-dialog__tabs"
                        role="tablist"
                        aria-label="Online subtitle source"
                    >
                        <button
                            v-for="provider in props.providerTabs"
                            :key="provider.id"
                            class="online-subtitle-dialog__tab"
                            :class="{
                                'online-subtitle-dialog__tab--active':
                                    provider.id === props.activeProviderId,
                            }"
                            type="button"
                            role="tab"
                            :aria-selected="provider.id === props.activeProviderId"
                            :disabled="props.applying"
                            @click="emit('providerChange', provider.id)"
                        >
                            <span>{{ provider.label }}</span>
                            <span
                                v-if="provider.loading"
                                class="online-subtitle-dialog__tab-spinner"
                                aria-hidden="true"
                            ></span>
                            <span
                                v-else-if="provider.searched"
                                class="online-subtitle-dialog__tab-count"
                            >
                                {{ provider.count }}
                            </span>
                        </button>
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
                    <span
                        class="online-subtitle-dialog__content-spinner"
                        aria-label="Searching"
                    ></span>
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
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 10px 50px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}

.online-subtitle-dialog__close {
    position: absolute;
    top: 50%;
    right: 16px;
    display: grid;
    width: 30px;
    height: 30px;
    place-items: center;
    transform: translateY(-50%);
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

.online-subtitle-dialog__tabs {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    min-width: 0;
    max-width: 100%;
    padding: 2px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 12px;
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.075), rgba(255, 255, 255, 0.035)),
        rgba(10, 15, 18, 0.28);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
    overflow-x: auto;
}

.online-subtitle-dialog__tab {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-height: 26px;
    border: 1px solid transparent;
    border-radius: 9px;
    background: transparent;
    color: rgba(255, 255, 255, 0.58);
    padding: 4px 10px;
    font-size: 12px;
    font-weight: 650;
    line-height: 1.2;
    white-space: nowrap;
    transition:
        background-color 0.18s ease,
        border-color 0.18s ease,
        box-shadow 0.18s ease,
        color 0.18s ease;
}

.online-subtitle-dialog__tab:hover:not(:disabled) {
    color: rgba(255, 255, 255, 0.82);
    background: rgba(255, 255, 255, 0.06);
}

.online-subtitle-dialog__tab--active {
    border-color: rgba(116, 166, 255, 0.66);
    background:
        linear-gradient(180deg, rgba(116, 166, 255, 0.19), rgba(116, 166, 255, 0.1)),
        rgba(255, 255, 255, 0.055);
    color: #f6f6f6;
    box-shadow:
        inset 0 1px 0 rgba(255, 255, 255, 0.12),
        0 0 0 1px rgba(116, 166, 255, 0.14);
}

.online-subtitle-dialog__tab:disabled {
    opacity: 0.56;
}

.online-subtitle-dialog__tab-count {
    min-width: 16px;
    padding: 1px 4px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.68);
    font-size: 10px;
    text-align: center;
}

.online-subtitle-dialog__tab--active .online-subtitle-dialog__tab-count {
    background: rgba(116, 166, 255, 0.18);
    color: rgba(255, 255, 255, 0.88);
}

.online-subtitle-dialog__tab-spinner {
    width: 9px;
    height: 9px;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: rgba(255, 255, 255, 0.78);
    border-radius: 999px;
    animation: online-subtitle-dialog-spin 0.7s linear infinite;
}

@keyframes online-subtitle-dialog-spin {
    to {
        transform: rotate(360deg);
    }
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

.online-subtitle-dialog__content-spinner {
    display: block;
    width: 18px;
    height: 18px;
    border: 2px solid rgba(255, 255, 255, 0.18);
    border-top-color: rgba(255, 255, 255, 0.78);
    border-radius: 999px;
    animation: online-subtitle-dialog-spin 0.7s linear infinite;
}

.online-subtitle-dialog__error {
    color: rgba(255, 127, 127, 0.96);
}

.online-subtitle-dialog__applying {
    border-top: 1px solid rgba(255, 255, 255, 0.1);
}
</style>
