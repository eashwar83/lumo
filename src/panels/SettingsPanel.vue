<script setup lang="ts">
import { useSettingsPanel } from "../composables/useSettingsPanel";
import {
    ENABLE_COMPACT_MODE_SETTING_LABEL,
    type SettingItem,
} from "../mock/settings";

const {
    settingGroups,
    runtimeVersions,
    shouldShowSetDefaultMediaButton,
    isSetDefaultButtonDisabled,
    isSetDefaultButtonLoading,
    setDefaultButtonText,
    isSetDefaultSuccess,
    shouldShowUpdateButton,
    isUpdateButtonDisabled,
    updateButtonText,
    isUpdateRetry,
    shouldShowUpdateStatus,
    updateStatusText,
    shouldShowUpdateHint,
    updateHintText,
    openProjectGithub,
    isApplyingMediaAssociation,
    setMediaAssociationToSoia,
    installUpdate,
    resetAllSettings,
    browseForPath,
    isFixedLogPathItem,
    isLoading,
} = useSettingsPanel();

const isLinuxPlatform =
    typeof navigator !== "undefined" && /\blinux\b/i.test(navigator.userAgent);

const shouldShowSettingItem = (item: SettingItem): boolean =>
    !(isLinuxPlatform && item.label === ENABLE_COMPACT_MODE_SETTING_LABEL);
</script>

<template>
    <div class="panel panel--settings">
        <div class="panel__header">
            <div class="panel__title">Settings</div>
            <div class="panel__header-actions">
                <div v-if="shouldShowUpdateStatus" class="panel__update-status" aria-live="polite">
                    <span class="panel__spinner" aria-hidden="true"></span>
                    <div class="panel__update-status-text">{{ updateStatusText }}</div>
                </div>
                <div v-if="shouldShowUpdateButton" class="panel__update-action-wrap">
                    <span
                        v-if="shouldShowUpdateHint"
                        class="panel__update-hint"
                    >
                        {{ updateHintText }}
                    </span>
                    <button
                        class="panel__action panel__action--glow panel__header-action panel__header-action--compact"
                        type="button"
                        :disabled="isUpdateButtonDisabled"
                        @click="installUpdate"
                    >
                        <span
                            v-if="isUpdateRetry"
                            class="panel__status-icon panel__status-icon--failed"
                            aria-hidden="true"
                        >
                            <svg class="panel__status-icon-svg" viewBox="0 0 20 20">
                                <path d="M5.5 5.5l9 9M14.5 5.5l-9 9" />
                            </svg>
                        </span>
                        <span>{{ updateButtonText }}</span>
                    </button>
                </div>
                <button
                    v-if="shouldShowSetDefaultMediaButton"
                    class="panel__action panel__action--glow panel__header-action"
                    type="button"
                    :disabled="isSetDefaultButtonDisabled || isApplyingMediaAssociation"
                    @click="setMediaAssociationToSoia"
                >
                    <span
                        v-if="isSetDefaultSuccess"
                        class="panel__status-icon panel__status-icon--success"
                        aria-hidden="true"
                    >
                        <svg class="panel__status-icon-svg" viewBox="0 0 20 20">
                            <path d="M4.5 10.5l3.4 3.4 7.6-7.8" />
                        </svg>
                    </span>
                    <span>{{ setDefaultButtonText }}</span>
                    <span v-if="isSetDefaultButtonLoading" class="panel__loading-dots" aria-hidden="true">
                        <span class="panel__loading-dot"></span>
                        <span class="panel__loading-dot"></span>
                        <span class="panel__loading-dot"></span>
                    </span>
                </button>
                <button class="panel__reset" type="button" @click="resetAllSettings">
                    Reset
                </button>
            </div>
        </div>
        <div v-if="isLoading" class="panel__skeleton">
            <div class="panel__skeleton-row"></div>
            <div class="panel__skeleton-row"></div>
            <div class="panel__skeleton-row"></div>
        </div>
        <div v-if="!settingGroups.length" class="panel__empty">
            <div class="panel__empty-title">No settings yet</div>
            <div class="panel__empty-body">
                Add configuration options to start customizing playback.
            </div>
        </div>
        <div v-else class="panel__stack">
            <div
                v-for="group in settingGroups"
                :key="group.title"
                class="panel__section"
            >
                <div class="panel__subtitle panel__subtitle--large">
                    {{ group.title }}
                </div>
                <div class="panel__table panel__table--card">
                    <div
                        v-for="item in group.items.filter(shouldShowSettingItem)"
                        :key="item.label"
                        class="panel__row panel__row--card"
                    >
                        <div class="panel__card-text">
                            <div class="panel__card-title">
                                {{ item.displayLabel ?? item.label }}
                            </div>
                        </div>
                        <div class="panel__control panel__control--card">
                            <template v-if="item.type === 'path'">
                                <div class="panel__path-control">
                                    <template v-if="isFixedLogPathItem(item)">
                                        <span class="panel__value-text panel__path-text panel__path-text--log">
                                            {{ item.value || item.placeholder || "Unavailable" }}
                                        </span>
                                    </template>
                                    <template v-else>
                                        <input
                                            v-model="item.value"
                                            class="panel__input panel__input--path"
                                            type="text"
                                            :placeholder="item.placeholder"
                                        />
                                    </template>
                                    <button
                                        class="panel__action panel__action--ghost panel__action--icon panel__path-action"
                                        type="button"
                                        :title="isFixedLogPathItem(item) ? 'Open Folder' : item.browseTitle ?? 'Browse'"
                                        :aria-label="isFixedLogPathItem(item) ? 'Open Folder' : item.browseTitle ?? 'Browse'"
                                        @click="browseForPath(item)"
                                    >
                                        <svg
                                            class="panel__action-icon panel__path-action-icon"
                                            xmlns="http://www.w3.org/2000/svg"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="2"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                        >
                                            <path
                                                d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
                                            ></path>
                                        </svg>
                                    </button>
                                </div>
                            </template>
                            <template v-else-if="item.type === 'slider'">
                                <div class="panel__slider">
                                    <input
                                        v-model="item.value"
                                        class="panel__slider-input"
                                        type="range"
                                        :min="item.min"
                                        :max="item.max"
                                        :step="item.step"
                                        :style="{
                                            '--slider-value': `${
                                                ((Number(item.value) - item.min) /
                                                    (item.max - item.min)) *
                                                100
                                            }%`,
                                        }"
                                    />
                                    <div class="panel__slider-value">
                                        {{ item.value }}{{ item.unit }}
                                    </div>
                                </div>
                            </template>
                            <template v-else-if="item.type === 'toggle'">
                                <label class="panel__toggle">
                                    <input
                                        class="panel__toggle-input"
                                        type="checkbox"
                                        :checked="item.value === item.onValue"
                                        @change="
                                            item.value = ($event.target as HTMLInputElement).checked
                                                ? item.onValue
                                                : item.offValue
                                        "
                                    />
                                    <span class="panel__toggle-track">
                                        <span class="panel__toggle-thumb"></span>
                                    </span>
                                </label>
                            </template>
                            <template v-else>
                                <div class="panel__select-wrap">
                                    <select
                                        v-model="item.value"
                                        class="panel__select panel__select--card"
                                    >
                                        <option
                                            v-for="option in item.options"
                                            :key="option"
                                            :value="option"
                                        >
                                            {{ option }}
                                        </option>
                                    </select>
                                </div>
                            </template>
                        </div>
                    </div>
                </div>
            </div>
            <div class="panel__section">
                <div class="panel__subtitle panel__subtitle--large">
                    About
                </div>
                <div class="panel__table panel__table--card">
                    <div class="panel__row panel__row--card" data-window-no-drag>
                        <div class="panel__card-text">
                            <div class="panel__card-title">GitHub</div>
                        </div>
                        <div class="panel__control panel__control--card">
                            <div class="panel__github-actions">
                                <a
                                    class="panel__link-button"
                                    href="https://github.com/FengZeng/soia"
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    data-window-no-drag
                                    @click.prevent="openProjectGithub"
                                >
                                    https://github.com/FengZeng/soia
                                </a>
                            </div>
                        </div>
                    </div>
                    <div class="panel__row panel__row--card">
                        <div class="panel__card-text">
                            <div class="panel__card-title">Runtime</div>
                        </div>
                        <div class="panel__control panel__control--card">
                            <span class="panel__value-text">
                                Soia {{ runtimeVersions?.soiaVersion ?? "Unavailable" }}
                                · mpv {{ runtimeVersions?.mpvVersion ?? "Unavailable" }}
                                · FFmpeg {{ runtimeVersions?.ffmpegVersion ?? "Unavailable" }}
                            </span>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped src="../styles/panels.css"></style>
