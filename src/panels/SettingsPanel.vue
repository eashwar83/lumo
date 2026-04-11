<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useSettingsPanel } from "../composables/useSettingsPanel";
import { getPathDisplayName } from "../utils/getPathDisplayName";
import {
    ENABLE_COMPACT_MODE_SETTING_LABEL,
    WALLPAPER_MODE_SETTING_LABEL,
    type SettingItem,
    type SettingGroup,
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
    factoryReset,
    isFactoryResetInProgress,
    browseForPath,
    browseForCustomShaders,
    selectedShaderFiles,
    activeShaderFiles,
    unavailableShaderFiles,
    multiShaderEnabled,
    renderingMode,
    setShaderEnabled,
    setMultiShaderEnabled,
    setRenderingMode,
    removeShaderFromList,
    clearShaders,
    isFixedLogPathItem,
    isLoading,
} = useSettingsPanel();

const isLinuxPlatform =
    typeof navigator !== "undefined" && /\blinux\b/i.test(navigator.userAgent);
const isWindowsPlatform =
    typeof navigator !== "undefined" && /\bwindows\b/i.test(navigator.userAgent);

const shouldShowSettingItem = (item: SettingItem): boolean =>
    !(
        (isLinuxPlatform && item.label === ENABLE_COMPACT_MODE_SETTING_LABEL) ||
        (!isWindowsPlatform && item.label === WALLPAPER_MODE_SETTING_LABEL)
    );

const visibleItems = (group: SettingGroup): SettingItem[] =>
    group.items.filter(shouldShowSettingItem);

const shouldShowGroup = (group: SettingGroup): boolean =>
    visibleItems(group).length > 0;

const getShaderDisplayName = (path: string): string => {
    const name = getPathDisplayName(path, path);
    return name.replace(/\.glsl$/i, "");
};

const isShaderUnavailable = (path: string): boolean =>
    unavailableShaderFiles.value.includes(path);

const getActiveShaderOrder = (path: string): number | null => {
    const index = activeShaderFiles.value.indexOf(path);
    return index >= 0 ? index + 1 : null;
};

const toggleShaderEnabled = (path: string) => {
    const nextEnabled = !activeShaderFiles.value.includes(path);
    setShaderEnabled(path, nextEnabled);
};

const SHADER_COLLAPSED_VISIBLE_COUNT = 4;
const isShaderListExpanded = ref(false);
const shouldShowShaderListCollapseToggle = computed(
    () => selectedShaderFiles.value.length > SHADER_COLLAPSED_VISIBLE_COUNT,
);
const shouldShowMultiShaderToggle = computed(
    () => selectedShaderFiles.value.length > 1,
);
const visibleShaderFiles = computed(() => {
    if (
        shouldShowShaderListCollapseToggle.value &&
        !isShaderListExpanded.value
    ) {
        return selectedShaderFiles.value.slice(0, SHADER_COLLAPSED_VISIBLE_COUNT);
    }
    return selectedShaderFiles.value;
});

const isNormalRenderingMode = computed(() => renderingMode.value === "normal");
const isAnimeModeRenderingMode = computed(
    () => renderingMode.value === "animeMode",
);
const hasEnabledShaderInCurrentMode = computed(
    () => activeShaderFiles.value.length > 0,
);
const shaderModeHintText = computed(() =>
    isAnimeModeRenderingMode.value
        ? "Anime Mode: Auto-detect anime videos and apply shaders only for anime."
        : "General Mode: Selected shaders will be applied to all videos.",
);

watch(
    selectedShaderFiles,
    (next) => {
        if (!next.length) {
            isShaderListExpanded.value = false;
        }
    },
    { deep: true },
);
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
                v-show="shouldShowGroup(group)"
            >
                <div class="panel__subtitle panel__subtitle--large">
                    {{ group.title }}
                </div>
                <div class="panel__table panel__table--card">
                    <div
                        v-for="item in visibleItems(group)"
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
                <template v-if="group.title === 'Playback'">
                    <div class="panel__subtitle panel__subtitle--large">
                        Rendering
                    </div>
                    <div class="panel__table panel__table--card">
                        <div class="panel__row panel__row--card panel__row--shader-header">
                            <div class="panel__card-text">
                                <div class="panel__card-title">Custom Shader</div>
                                <div class="panel__card-subtitle">
                                    Select one or more <code>.glsl</code> shader files.
                                </div>
                            </div>
                            <div class="panel__control panel__control--card panel__control--stack">
                                <div class="panel__actions panel__actions--inline panel__actions--shader">
                                    <button
                                        class="panel__action panel__action--ghost panel__action--compact"
                                        type="button"
                                        @click="browseForCustomShaders"
                                    >
                                        Add Shaders
                                    </button>
                                    <button
                                        class="panel__action panel__action--ghost panel__action--compact"
                                        type="button"
                                        :disabled="!selectedShaderFiles.length"
                                        @click="clearShaders"
                                    >
                                        Clear
                                    </button>
                                </div>
                            </div>
                        </div>
                        <div class="panel__row panel__row--card panel__row--stacked">
                            <div class="panel__shader-list-wrap">
                                <div class="panel__shader-mode-hint">
                                    {{ shaderModeHintText }}
                                </div>
                                <div
                                    v-if="!selectedShaderFiles.length"
                                    class="panel__shader-empty"
                                >
                                    No shader files selected.
                                </div>
                                <div v-else class="panel__shader-list">
                                    <div
                                        v-for="shaderPath in visibleShaderFiles"
                                        :key="shaderPath"
                                        class="panel__shader-item"
                                        :class="{
                                            'panel__shader-item--active':
                                                getActiveShaderOrder(shaderPath) !== null,
                                            'panel__shader-item--unavailable':
                                                isShaderUnavailable(shaderPath),
                                        }"
                                        role="checkbox"
                                        :aria-checked="
                                            getActiveShaderOrder(shaderPath) !== null
                                        "
                                        :aria-disabled="
                                            isShaderUnavailable(shaderPath)
                                        "
                                        :tabindex="
                                            isShaderUnavailable(shaderPath) ? -1 : 0
                                        "
                                        :title="
                                            isShaderUnavailable(shaderPath)
                                                ? `File not found: ${shaderPath}`
                                                : shaderPath
                                        "
                                        @click="toggleShaderEnabled(shaderPath)"
                                        @keydown.enter.prevent="
                                            toggleShaderEnabled(shaderPath)
                                        "
                                        @keydown.space.prevent="
                                            toggleShaderEnabled(shaderPath)
                                        "
                                    >
                                        <span
                                            v-if="multiShaderEnabled"
                                            class="panel__shader-select"
                                            :class="{
                                                'panel__shader-select--active':
                                                    getActiveShaderOrder(shaderPath) !== null,
                                                'panel__shader-select--unavailable':
                                                    isShaderUnavailable(shaderPath),
                                            }"
                                            :title="
                                                getActiveShaderOrder(shaderPath) !== null
                                                    ? `Shader order ${getActiveShaderOrder(shaderPath)}`
                                                    : 'Select shader'
                                            "
                                        >
                                            {{ getActiveShaderOrder(shaderPath) ?? "" }}
                                        </span>
                                        <span class="panel__shader-name">
                                            {{ getShaderDisplayName(shaderPath) }}
                                        </span>
                                        <span
                                            v-if="isShaderUnavailable(shaderPath)"
                                            class="panel__shader-missing"
                                        >
                                            Missing
                                        </span>
                                        <button
                                            class="panel__shader-remove"
                                            type="button"
                                            aria-label="Remove shader"
                                            @click.stop.prevent="
                                                removeShaderFromList(shaderPath)
                                            "
                                        >
                                            <svg
                                                class="panel__shader-remove-icon"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2"
                                                stroke-linecap="round"
                                            >
                                                <path d="M6 6l12 12M18 6l-12 12" />
                                            </svg>
                                        </button>
                                    </div>
                                </div>
                                <div class="panel__shader-list-footer">
                                    <div class="panel__shader-list-footer-left">
                                        <label
                                            v-if="shouldShowMultiShaderToggle"
                                            class="panel__shader-multi-toggle"
                                        >
                                            <span class="panel__shader-multi-label">
                                                Use Multiple Shader
                                            </span>
                                            <span class="panel__toggle panel__toggle--shader-multi">
                                                <input
                                                    class="panel__toggle-input"
                                                    type="checkbox"
                                                    :checked="multiShaderEnabled"
                                                    @change="
                                                        setMultiShaderEnabled(
                                                            ($event.target as HTMLInputElement).checked,
                                                        )
                                                    "
                                                />
                                                <span class="panel__toggle-track">
                                                    <span class="panel__toggle-thumb"></span>
                                                </span>
                                            </span>
                                        </label>
                                    </div>

                                    <div class="panel__shader-list-footer-center">
                                        <button
                                            class="panel__shader-mode-switch"
                                            type="button"
                                            @click="
                                                setRenderingMode(
                                                    isNormalRenderingMode ? 'animeMode' : 'normal',
                                                )
                                            "
                                        >
                                            <span
                                                class="panel__shader-mode-switch-item"
                                                :class="{
                                                    'panel__shader-mode-switch-item--current':
                                                        isNormalRenderingMode,
                                                    'panel__shader-mode-switch-item--enabled':
                                                        isNormalRenderingMode &&
                                                        hasEnabledShaderInCurrentMode,
                                                }"
                                            >
                                                General Mode
                                            </span>
                                            <span
                                                class="panel__shader-mode-switch-item"
                                                :class="{
                                                    'panel__shader-mode-switch-item--current':
                                                        isAnimeModeRenderingMode,
                                                    'panel__shader-mode-switch-item--enabled':
                                                        isAnimeModeRenderingMode &&
                                                        hasEnabledShaderInCurrentMode,
                                                }"
                                            >
                                                Anime Mode
                                            </span>
                                        </button>
                                    </div>

                                    <div class="panel__shader-list-footer-right">
                                        <div
                                            v-if="shouldShowShaderListCollapseToggle"
                                            class="panel__shader-list-actions"
                                        >
                                            <button
                                                class="panel__action panel__action--ghost panel__action--compact panel__shader-toggle"
                                                type="button"
                                                @click="
                                                    isShaderListExpanded = !isShaderListExpanded
                                                "
                                            >
                                                <span>
                                                    {{
                                                        isShaderListExpanded
                                                            ? `Collapse (${selectedShaderFiles.length})`
                                                            : `Show all (${selectedShaderFiles.length})`
                                                    }}
                                                </span>
                                                <svg
                                                    class="panel__shader-toggle-icon"
                                                    viewBox="0 0 20 20"
                                                    fill="none"
                                                    stroke="currentColor"
                                                    stroke-width="2"
                                                    stroke-linecap="round"
                                                    stroke-linejoin="round"
                                                    aria-hidden="true"
                                                >
                                                    <path
                                                        v-if="isShaderListExpanded"
                                                        d="M5 12l5-5 5 5"
                                                    />
                                                    <path
                                                        v-else
                                                        d="M5 8l5 5 5-5"
                                                    />
                                                </svg>
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </template>
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
                    <div class="panel__row panel__row--card">
                        <div class="panel__card-text">
                            <div class="panel__card-title">Clear All Local Data</div>
                        </div>
                        <div class="panel__control panel__control--card">
                            <button
                                class="panel__reset panel__reset--danger"
                                type="button"
                                :disabled="isFactoryResetInProgress"
                                @click="factoryReset"
                            >
                                {{
                                    isFactoryResetInProgress
                                        ? "Resetting..."
                                        : "Factory Reset"
                                }}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
</template>

<style scoped src="../styles/panels.css"></style>
