<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useSettingsPanel } from "../composables/useSettingsPanel";
import { getPathDisplayName } from "../utils/getPathDisplayName";
import ShortcutSettings from "../components/ShortcutSettings.vue";
import { THEME_SETTING_GROUP_TITLE } from "../constants/theme";
import {
    AUTOLOAD_FOLDER_SETTING_LABEL,
    DISABLE_SUBTITLES_SETTING_LABEL,
    ENABLE_COMPACT_MODE_SETTING_LABEL,
    IMAGE_DISPLAY_DURATION_SETTING_LABEL,
    KEYBOARD_SHORTCUTS_GROUP_TITLE,
    ONLINE_SUBTITLES_SETTING_GROUP_TITLE,
    OPENSUBTITLES_API_KEY_SETTING_LABEL,
    OPENSUBTITLES_ENABLED_SETTING_LABEL,
    OPENSUBTITLES_LANGUAGES_SETTING_LABEL,
    PLAYBACK_TITLE_SETTING_LABEL,
    SCREENSHOT_DIR_SETTING_LABEL,
    SUBSOURCE_API_KEY_SETTING_LABEL,
    SUBSOURCE_ENABLED_SETTING_LABEL,
    SUBSOURCE_LANGUAGES_SETTING_LABEL,
    WALLPAPER_MODE_SETTING_LABEL,
    type SettingGroup,
    type SettingItem,
} from "../mock/settings";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ (e: "close"): void }>();

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
    clearDownloadedSubtitles,
    isClearingOnlineSubtitleCache,
    onlineSubtitleCacheStatus,
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

// --- Category model ---------------------------------------------------------
// The stored setting groups don't map one-to-one onto the categories we show in
// the rail: "General"/"Playback"/"Subtitles" all draw from the single stored
// "Playback" group, and "Advanced" merges three groups. We resolve items by
// label/title here rather than restructuring the data model.

type CategoryId =
    | "general"
    | "playback"
    | "video"
    | "subtitles"
    | "shortcuts"
    | "network"
    | "advanced"
    | "about";

const CATEGORIES: { id: CategoryId; label: string; paths: string[] }[] = [
    {
        id: "general",
        label: "General",
        paths: [
            "M4 21v-6",
            "M4 11V3",
            "M12 21v-9",
            "M12 5V3",
            "M20 21v-4",
            "M20 9V3",
            "M2 15h4",
            "M10 5h4",
            "M18 13h4",
        ],
    },
    { id: "playback", label: "Playback", paths: ["M7 5v14l11-7z"] },
    {
        id: "video",
        label: "Video",
        paths: ["M3 5h18v12H3z", "M10 9l5 3-5 3z"],
    },
    {
        id: "subtitles",
        label: "Subtitles",
        paths: ["M4 5h16v12H4z", "M7 13h4", "M13 13h4"],
    },
    {
        id: "shortcuts",
        label: "Shortcuts",
        paths: [
            "M3 6h18v12H3z",
            "M7 10h.01",
            "M12 10h.01",
            "M17 10h.01",
            "M8 14h8",
        ],
    },
    {
        id: "network",
        label: "Network",
        paths: [
            "M12 3a9 9 0 100 18 9 9 0 000-18",
            "M3 12h18",
            "M12 3c3 3.5 3 14.5 0 18",
            "M12 3c-3 3.5-3 14.5 0 18",
        ],
    },
    {
        id: "advanced",
        label: "Advanced",
        paths: ["M21 3a6 6 0 01-8 8l-7 7-2-2 7-7a6 6 0 018-8l-3 3 2 2z"],
    },
    {
        id: "about",
        label: "About",
        paths: ["M12 3a9 9 0 100 18 9 9 0 000-18", "M12 11v5", "M12 8h.01"],
    },
];

const activeCategory = ref<CategoryId>("general");

const PLAYBACK_GROUP_TITLE = "Playback";
const NETWORK_GROUP_TITLE = "Network";
const TOOLS_GROUP_TITLE = "Tools";
const ADVANCED_GROUP_TITLE = "Advanced";
const EXPERIMENTS_GROUP_TITLE = "Experiments";

// Items pulled OUT of the stored "Playback" group into other categories.
const GENERAL_FROM_PLAYBACK = new Set<string>([
    PLAYBACK_TITLE_SETTING_LABEL,
    AUTOLOAD_FOLDER_SETTING_LABEL,
    SCREENSHOT_DIR_SETTING_LABEL,
    IMAGE_DISPLAY_DURATION_SETTING_LABEL,
]);
const SUBTITLES_FROM_PLAYBACK = new Set<string>([
    DISABLE_SUBTITLES_SETTING_LABEL,
]);

const groupByTitle = (title: string): SettingGroup | undefined =>
    settingGroups.value.find((group) => group.title === title);

const visibleOfTitle = (title: string): SettingItem[] => {
    const group = groupByTitle(title);
    return group ? visibleItems(group) : [];
};

// --- Online subtitles (provider tabs) ---------------------------------------

const onlineSubtitleTabs = [
    {
        id: "opensubtitles",
        label: "OpenSubtitles",
        itemLabels: [
            OPENSUBTITLES_ENABLED_SETTING_LABEL,
            OPENSUBTITLES_API_KEY_SETTING_LABEL,
            OPENSUBTITLES_LANGUAGES_SETTING_LABEL,
        ],
    },
    {
        id: "subsource",
        label: "SubSource",
        itemLabels: [
            SUBSOURCE_ENABLED_SETTING_LABEL,
            SUBSOURCE_API_KEY_SETTING_LABEL,
            SUBSOURCE_LANGUAGES_SETTING_LABEL,
        ],
    },
] as const;
const activeOnlineSubtitleTab = ref<(typeof onlineSubtitleTabs)[number]["id"]>(
    "opensubtitles",
);

const onlineSubtitleItems = computed<SettingItem[]>(() => {
    const group = groupByTitle(ONLINE_SUBTITLES_SETTING_GROUP_TITLE);
    if (!group) return [];
    const activeTab =
        onlineSubtitleTabs.find((tab) => tab.id === activeOnlineSubtitleTab.value) ??
        onlineSubtitleTabs[0];
    const labels = new Set<string>(activeTab.itemLabels);
    return visibleItems(group).filter((item) => labels.has(item.label));
});

// --- Blocks rendered for the active category --------------------------------

type Block =
    | { kind: "items"; key: string; title?: string; items: SettingItem[] }
    | { kind: "onlineSubs"; key: string; items: SettingItem[] }
    | { kind: "shader"; key: string }
    | { kind: "shortcuts"; key: string; group: SettingGroup }
    | { kind: "about"; key: string };

const activeBlocks = computed<Block[]>(() => {
    switch (activeCategory.value) {
        case "general": {
            const items = [
                ...visibleOfTitle(THEME_SETTING_GROUP_TITLE),
                ...visibleOfTitle(PLAYBACK_GROUP_TITLE).filter((item) =>
                    GENERAL_FROM_PLAYBACK.has(item.label),
                ),
            ];
            return [{ kind: "items", key: "general", items }];
        }
        case "playback": {
            const items = visibleOfTitle(PLAYBACK_GROUP_TITLE).filter(
                (item) =>
                    !GENERAL_FROM_PLAYBACK.has(item.label) &&
                    !SUBTITLES_FROM_PLAYBACK.has(item.label),
            );
            return [{ kind: "items", key: "playback", items }];
        }
        case "video":
            return [{ kind: "shader", key: "shader" }];
        case "subtitles": {
            const blocks: Block[] = [];
            const disableSub = visibleOfTitle(PLAYBACK_GROUP_TITLE).filter(
                (item) => SUBTITLES_FROM_PLAYBACK.has(item.label),
            );
            if (disableSub.length) {
                blocks.push({ kind: "items", key: "sub-basic", items: disableSub });
            }
            blocks.push({
                kind: "onlineSubs",
                key: "online-subs",
                items: onlineSubtitleItems.value,
            });
            return blocks;
        }
        case "shortcuts": {
            const group = groupByTitle(KEYBOARD_SHORTCUTS_GROUP_TITLE);
            return group ? [{ kind: "shortcuts", key: "shortcuts", group }] : [];
        }
        case "network":
            return [
                {
                    kind: "items",
                    key: "network",
                    items: visibleOfTitle(NETWORK_GROUP_TITLE),
                },
            ];
        case "advanced": {
            const blocks: Block[] = [];
            const seek = visibleOfTitle(ADVANCED_GROUP_TITLE);
            if (seek.length) {
                blocks.push({
                    kind: "items",
                    key: "adv-seek",
                    title: "Seek Thumbnails",
                    items: seek,
                });
            }
            const tools = visibleOfTitle(TOOLS_GROUP_TITLE);
            if (tools.length) {
                blocks.push({
                    kind: "items",
                    key: "adv-tools",
                    title: "Downloads, Proxy & Logs",
                    items: tools,
                });
            }
            const experiments = visibleOfTitle(EXPERIMENTS_GROUP_TITLE);
            if (experiments.length) {
                blocks.push({
                    kind: "items",
                    key: "adv-exp",
                    title: "Experiments",
                    items: experiments,
                });
            }
            return blocks;
        }
        case "about":
            return [{ kind: "about", key: "about" }];
        default:
            return [];
    }
});

// --- Shader helpers ----------------------------------------------------------

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

// --- Custom select (dropdown) -----------------------------------------------

const openSelectKey = ref<string | null>(null);
const activeSelectOptionIndex = ref<number>(0);
const selectTriggerRefs = new Map<string, HTMLElement>();
const selectMenuStyle = ref<Record<string, string>>({});

const selectKeyOf = (item: SettingItem): string => item.label;

const isSelectItem = (
    item: SettingItem,
): item is Extract<SettingItem, { type: "select" }> => item.type === "select";

const getSelectOptionIndex = (item: SettingItem): number => {
    if (!isSelectItem(item)) return 0;
    const index = item.options.indexOf(item.value);
    return index >= 0 ? index : 0;
};

const clampOptionIndex = (item: SettingItem, index: number): number => {
    if (!isSelectItem(item) || !item.options.length) return 0;
    if (index < 0) return item.options.length - 1;
    if (index >= item.options.length) return 0;
    return index;
};

const openSelect = (key: string, item: SettingItem) => {
    openSelectKey.value = key;
    activeSelectOptionIndex.value = getSelectOptionIndex(item);
    nextTick(() => {
        updateOpenSelectMenuPosition();
    });
};

const closeSelect = () => {
    openSelectKey.value = null;
};

const toggleSelect = (key: string, item: SettingItem) => {
    if (openSelectKey.value === key) {
        closeSelect();
        return;
    }
    openSelect(key, item);
};

const isSelectOpen = (key: string): boolean => openSelectKey.value === key;

const setActiveSelectOption = (item: SettingItem, step: number) => {
    activeSelectOptionIndex.value = clampOptionIndex(
        item,
        activeSelectOptionIndex.value + step,
    );
};

const chooseSelectOption = (item: SettingItem, option: string) => {
    if (!isSelectItem(item)) return;
    item.value = option;
    closeSelect();
};

const setSelectTriggerRef = (key: string, element: HTMLElement | null) => {
    if (!element) {
        selectTriggerRefs.delete(key);
        return;
    }
    selectTriggerRefs.set(key, element);
};

const getOpenSelectMenuStyle = (): Record<string, string> => selectMenuStyle.value;

const updateOpenSelectMenuPosition = () => {
    if (!openSelectKey.value) return;
    const trigger = selectTriggerRefs.get(openSelectKey.value);
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const viewportHeight = window.innerHeight;
    const spaceAbove = rect.top;
    const spaceBelow = viewportHeight - rect.bottom;
    const estimatedMenuHeight = 240;
    const shouldOpenTop =
        spaceBelow < estimatedMenuHeight && spaceAbove > spaceBelow;
    const gap = 6;
    const maxHeight = Math.max(
        120,
        Math.min(320, shouldOpenTop ? spaceAbove - 10 : spaceBelow - 10),
    );
    const triggerStyles = getComputedStyle(trigger);
    const menuThemeVars: Record<string, string> = {
        "--panel-select-card-border": triggerStyles
            .getPropertyValue("--panel-select-card-border")
            .trim(),
        "--panel-select-card-text": triggerStyles
            .getPropertyValue("--panel-select-card-text")
            .trim(),
        "--panel-select-card-hover-bg": triggerStyles
            .getPropertyValue("--panel-select-card-hover-bg")
            .trim(),
        "--panel-select-card-focus-bg": triggerStyles
            .getPropertyValue("--panel-select-card-focus-bg")
            .trim(),
        "--panel-select-card-focus-border": triggerStyles
            .getPropertyValue("--panel-select-card-focus-border")
            .trim(),
        "--panel-select-menu-bg": triggerStyles
            .getPropertyValue("--panel-select-menu-bg")
            .trim(),
        "--panel-select-menu-border": triggerStyles
            .getPropertyValue("--panel-select-menu-border")
            .trim(),
        "--panel-select-menu-hover-bg": triggerStyles
            .getPropertyValue("--panel-select-menu-hover-bg")
            .trim(),
        "--panel-select-menu-selected-bg": triggerStyles
            .getPropertyValue("--panel-select-menu-selected-bg")
            .trim(),
        "--panel-select-menu-selected-border": triggerStyles
            .getPropertyValue("--panel-select-menu-selected-border")
            .trim(),
    };

    selectMenuStyle.value = shouldOpenTop
        ? {
              ...menuThemeVars,
              left: `${rect.left}px`,
              width: `${rect.width}px`,
              bottom: `${viewportHeight - rect.top + gap}px`,
              maxHeight: `${maxHeight}px`,
          }
        : {
              ...menuThemeVars,
              left: `${rect.left}px`,
              width: `${rect.width}px`,
              top: `${rect.bottom + gap}px`,
              maxHeight: `${maxHeight}px`,
          };
};

const onSelectTriggerKeydown = (
    event: KeyboardEvent,
    key: string,
    item: SettingItem,
) => {
    if (!isSelectItem(item) || !item.options.length) return;

    if (event.key === "ArrowDown") {
        event.preventDefault();
        if (!isSelectOpen(key)) {
            openSelect(key, item);
            return;
        }
        setActiveSelectOption(item, 1);
        return;
    }

    if (event.key === "ArrowUp") {
        event.preventDefault();
        if (!isSelectOpen(key)) {
            openSelect(key, item);
            return;
        }
        setActiveSelectOption(item, -1);
        return;
    }

    if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        if (!isSelectOpen(key)) {
            openSelect(key, item);
            return;
        }
        const nextOption = item.options[activeSelectOptionIndex.value];
        if (nextOption !== undefined) {
            chooseSelectOption(item, nextOption);
        }
        return;
    }

    if (event.key === "Escape") {
        if (isSelectOpen(key)) {
            event.preventDefault();
            event.stopPropagation();
            closeSelect();
        }
    }
};

const onDocumentPointerDown = (event: PointerEvent) => {
    if (!openSelectKey.value) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest(".panel__custom-select, .panel__custom-select-menu")) {
        return;
    }
    closeSelect();
};

const shouldShowShaderListCollapseToggle = computed(
    () => selectedShaderFiles.value.length > SHADER_COLLAPSED_VISIBLE_COUNT,
);
const shouldShowMultiShaderToggle = computed(
    () => selectedShaderFiles.value.length > 1,
);
const visibleShaderFiles = computed(() => {
    if (shouldShowShaderListCollapseToggle.value && !isShaderListExpanded.value) {
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
        if (!next.length) isShaderListExpanded.value = false;
    },
    { deep: true },
);

// --- Modal shell ------------------------------------------------------------

const cardRef = ref<HTMLElement | null>(null);

const onBackdropClick = () => emit("close");

// Esc closes an open dropdown first, otherwise the modal. `.stop` keeps it from
// reaching the global shortcut handler (which would exit fullscreen).
const onCardEscape = () => {
    if (openSelectKey.value) {
        closeSelect();
        return;
    }
    emit("close");
};

watch(
    () => props.visible,
    (visible) => {
        if (visible) {
            nextTick(() => cardRef.value?.focus());
        } else {
            closeSelect();
        }
    },
);

onMounted(() => {
    document.addEventListener("pointerdown", onDocumentPointerDown);
    document.addEventListener("scroll", updateOpenSelectMenuPosition, true);
    window.addEventListener("resize", updateOpenSelectMenuPosition);
});

onBeforeUnmount(() => {
    document.removeEventListener("pointerdown", onDocumentPointerDown);
    document.removeEventListener("scroll", updateOpenSelectMenuPosition, true);
    window.removeEventListener("resize", updateOpenSelectMenuPosition);
});
</script>

<template>
    <transition name="settings-modal-fade">
        <div
            v-if="props.visible"
            class="settings-modal"
            data-window-no-drag
            @click.self="onBackdropClick"
        >
            <div
                ref="cardRef"
                class="settings-modal__card"
                tabindex="-1"
                role="dialog"
                aria-label="Settings"
                @keydown.esc.stop.prevent="onCardEscape"
            >
                <header class="settings-modal__titlebar">
                    <div class="settings-modal__title">Settings</div>
                    <button
                        class="settings-modal__close"
                        type="button"
                        aria-label="Close settings"
                        @click="emit('close')"
                    >
                        ✕
                    </button>
                </header>

                <div class="settings-modal__body">
                    <nav class="settings-rail" aria-label="Settings categories">
                        <button
                            v-for="cat in CATEGORIES"
                            :key="cat.id"
                            class="settings-rail__item"
                            :class="{
                                'settings-rail__item--active':
                                    activeCategory === cat.id,
                            }"
                            type="button"
                            @click="activeCategory = cat.id"
                        >
                            <svg
                                class="settings-rail__icon"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <path
                                    v-for="(d, i) in cat.paths"
                                    :key="i"
                                    :d="d"
                                />
                            </svg>
                            <span class="settings-rail__label">{{
                                cat.label
                            }}</span>
                        </button>
                    </nav>

                    <div class="settings-modal__content">
                        <div v-if="isLoading" class="panel__skeleton">
                            <div class="panel__skeleton-row"></div>
                            <div class="panel__skeleton-row"></div>
                            <div class="panel__skeleton-row"></div>
                        </div>

                        <template v-else>
                            <div
                                v-for="block in activeBlocks"
                                :key="block.key"
                                class="settings-section"
                            >
                                <!-- Item / online-subtitles blocks -->
                                <template
                                    v-if="
                                        block.kind === 'items' ||
                                        block.kind === 'onlineSubs'
                                    "
                                >
                                    <div
                                        v-if="block.kind === 'items' && block.title"
                                        class="settings-section__title"
                                    >
                                        {{ block.title }}
                                    </div>

                                    <div
                                        v-if="block.kind === 'onlineSubs'"
                                        class="panel__section-toolbar"
                                    >
                                        <div
                                            class="panel__tabs"
                                            role="tablist"
                                            aria-label="Online subtitle providers"
                                        >
                                            <button
                                                v-for="tab in onlineSubtitleTabs"
                                                :key="tab.id"
                                                class="panel__tab"
                                                :class="{
                                                    'panel__tab--active':
                                                        activeOnlineSubtitleTab ===
                                                        tab.id,
                                                }"
                                                type="button"
                                                role="tab"
                                                :aria-selected="
                                                    activeOnlineSubtitleTab ===
                                                    tab.id
                                                "
                                                @click="
                                                    activeOnlineSubtitleTab =
                                                        tab.id
                                                "
                                            >
                                                {{ tab.label }}
                                            </button>
                                        </div>
                                        <div class="panel__toolbar-actions">
                                            <div
                                                v-if="onlineSubtitleCacheStatus"
                                                class="panel__cache-status"
                                                role="status"
                                            >
                                                {{ onlineSubtitleCacheStatus }}
                                            </div>
                                            <button
                                                class="panel__action panel__action--ghost panel__action--compact"
                                                type="button"
                                                :disabled="
                                                    isClearingOnlineSubtitleCache
                                                "
                                                @click="clearDownloadedSubtitles"
                                            >
                                                {{
                                                    isClearingOnlineSubtitleCache
                                                        ? "Clearing..."
                                                        : "Clear Cache"
                                                }}
                                            </button>
                                        </div>
                                    </div>

                                    <div class="panel__table panel__table--card">
                                        <div
                                            v-for="item in block.items"
                                            :key="item.label"
                                            class="panel__row panel__row--card"
                                        >
                                            <div class="panel__card-text">
                                                <div class="panel__card-title">
                                                    {{
                                                        item.displayLabel ??
                                                        item.label
                                                    }}
                                                </div>
                                            </div>
                                            <div
                                                class="panel__control panel__control--card"
                                            >
                                                <template
                                                    v-if="item.type === 'path'"
                                                >
                                                    <div
                                                        class="panel__path-field"
                                                    >
                                                        <div
                                                            class="panel__path-control"
                                                        >
                                                            <template
                                                                v-if="
                                                                    isFixedLogPathItem(
                                                                        item,
                                                                    )
                                                                "
                                                            >
                                                                <span
                                                                    class="panel__value-text panel__path-text panel__path-text--log"
                                                                >
                                                                    {{
                                                                        item.value ||
                                                                        item.placeholder ||
                                                                        "Unavailable"
                                                                    }}
                                                                </span>
                                                            </template>
                                                            <template v-else>
                                                                <input
                                                                    v-model="
                                                                        item.value
                                                                    "
                                                                    class="panel__input panel__input--path"
                                                                    :class="{
                                                                        'panel__input--invalid':
                                                                            item.validationMessage,
                                                                    }"
                                                                    type="text"
                                                                    :placeholder="
                                                                        item.placeholder
                                                                    "
                                                                    :aria-invalid="
                                                                        Boolean(
                                                                            item.validationMessage,
                                                                        )
                                                                    "
                                                                />
                                                            </template>
                                                            <button
                                                                class="panel__action panel__action--ghost panel__action--icon panel__path-action"
                                                                type="button"
                                                                :title="
                                                                    isFixedLogPathItem(
                                                                        item,
                                                                    )
                                                                        ? 'Open Folder'
                                                                        : item.browseTitle ??
                                                                          'Browse'
                                                                "
                                                                :aria-label="
                                                                    isFixedLogPathItem(
                                                                        item,
                                                                    )
                                                                        ? 'Open Folder'
                                                                        : item.browseTitle ??
                                                                          'Browse'
                                                                "
                                                                @click="
                                                                    browseForPath(
                                                                        item,
                                                                    )
                                                                "
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
                                                        <p
                                                            v-if="
                                                                item.validationMessage
                                                            "
                                                            class="panel__validation"
                                                            role="status"
                                                        >
                                                            <svg
                                                                class="panel__validation-icon"
                                                                xmlns="http://www.w3.org/2000/svg"
                                                                viewBox="0 0 24 24"
                                                                fill="none"
                                                                stroke="currentColor"
                                                                stroke-width="2"
                                                                stroke-linecap="round"
                                                                stroke-linejoin="round"
                                                                aria-hidden="true"
                                                            >
                                                                <path
                                                                    d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z"
                                                                />
                                                                <path
                                                                    d="M12 9v4"
                                                                />
                                                                <path
                                                                    d="M12 17h.01"
                                                                />
                                                            </svg>
                                                            <span>{{
                                                                item.validationMessage
                                                            }}</span>
                                                        </p>
                                                    </div>
                                                </template>
                                                <template
                                                    v-else-if="
                                                        item.type === 'text'
                                                    "
                                                >
                                                    <div
                                                        class="panel__path-field"
                                                    >
                                                        <input
                                                            v-model="item.value"
                                                            class="panel__input panel__input--path"
                                                            :class="{
                                                                'panel__input--invalid':
                                                                    item.validationMessage,
                                                            }"
                                                            type="text"
                                                            :placeholder="
                                                                item.placeholder
                                                            "
                                                            :aria-invalid="
                                                                Boolean(
                                                                    item.validationMessage,
                                                                )
                                                            "
                                                        />
                                                        <p
                                                            v-if="
                                                                item.validationMessage
                                                            "
                                                            class="panel__validation"
                                                            role="status"
                                                        >
                                                            <svg
                                                                class="panel__validation-icon"
                                                                xmlns="http://www.w3.org/2000/svg"
                                                                viewBox="0 0 24 24"
                                                                fill="none"
                                                                stroke="currentColor"
                                                                stroke-width="2"
                                                                stroke-linecap="round"
                                                                stroke-linejoin="round"
                                                                aria-hidden="true"
                                                            >
                                                                <path
                                                                    d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z"
                                                                />
                                                                <path
                                                                    d="M12 9v4"
                                                                />
                                                                <path
                                                                    d="M12 17h.01"
                                                                />
                                                            </svg>
                                                            <span>{{
                                                                item.validationMessage
                                                            }}</span>
                                                        </p>
                                                    </div>
                                                </template>
                                                <template
                                                    v-else-if="
                                                        item.type === 'slider'
                                                    "
                                                >
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
                                                                    ((Number(
                                                                        item.value,
                                                                    ) -
                                                                        item.min) /
                                                                        (item.max -
                                                                            item.min)) *
                                                                    100
                                                                }%`,
                                                            }"
                                                        />
                                                        <div
                                                            class="panel__slider-value"
                                                        >
                                                            {{ item.value
                                                            }}{{ item.unit }}
                                                        </div>
                                                    </div>
                                                </template>
                                                <template
                                                    v-else-if="
                                                        item.type === 'toggle'
                                                    "
                                                >
                                                    <label class="panel__toggle">
                                                        <input
                                                            class="panel__toggle-input"
                                                            type="checkbox"
                                                            :checked="
                                                                item.value ===
                                                                item.onValue
                                                            "
                                                            @change="
                                                                item.value = (
                                                                    $event.target as HTMLInputElement
                                                                ).checked
                                                                    ? item.onValue
                                                                    : item.offValue
                                                            "
                                                        />
                                                        <span
                                                            class="panel__toggle-track"
                                                        >
                                                            <span
                                                                class="panel__toggle-thumb"
                                                            ></span>
                                                        </span>
                                                    </label>
                                                </template>
                                                <template v-else>
                                                    <div
                                                        class="panel__custom-select"
                                                        :class="{
                                                            'panel__custom-select--open':
                                                                isSelectOpen(
                                                                    selectKeyOf(
                                                                        item,
                                                                    ),
                                                                ),
                                                        }"
                                                    >
                                                        <button
                                                            type="button"
                                                            class="panel__custom-select-trigger"
                                                            :ref="
                                                                (el) =>
                                                                    setSelectTriggerRef(
                                                                        selectKeyOf(
                                                                            item,
                                                                        ),
                                                                        el as HTMLElement | null,
                                                                    )
                                                            "
                                                            :aria-expanded="
                                                                isSelectOpen(
                                                                    selectKeyOf(
                                                                        item,
                                                                    ),
                                                                )
                                                            "
                                                            aria-haspopup="listbox"
                                                            @click="
                                                                toggleSelect(
                                                                    selectKeyOf(
                                                                        item,
                                                                    ),
                                                                    item,
                                                                )
                                                            "
                                                            @keydown="
                                                                onSelectTriggerKeydown(
                                                                    $event,
                                                                    selectKeyOf(
                                                                        item,
                                                                    ),
                                                                    item,
                                                                )
                                                            "
                                                        >
                                                            <span
                                                                class="panel__custom-select-value"
                                                            >
                                                                {{ item.value }}
                                                            </span>
                                                            <span
                                                                class="panel__custom-select-arrow"
                                                                aria-hidden="true"
                                                            >
                                                                <svg
                                                                    viewBox="0 0 12 12"
                                                                >
                                                                    <path
                                                                        d="M2.25 4.5L6 8.25L9.75 4.5"
                                                                    />
                                                                </svg>
                                                            </span>
                                                        </button>
                                                        <Teleport to="body">
                                                            <div
                                                                v-if="
                                                                    isSelectOpen(
                                                                        selectKeyOf(
                                                                            item,
                                                                        ),
                                                                    )
                                                                "
                                                                class="panel__custom-select-menu"
                                                                :style="
                                                                    getOpenSelectMenuStyle()
                                                                "
                                                                role="listbox"
                                                                :aria-label="
                                                                    item.displayLabel ??
                                                                    item.label
                                                                "
                                                            >
                                                                <button
                                                                    v-for="(
                                                                        option,
                                                                        optionIndex
                                                                    ) in item.options"
                                                                    :key="option"
                                                                    type="button"
                                                                    class="panel__custom-select-option"
                                                                    :class="{
                                                                        'panel__custom-select-option--selected':
                                                                            option ===
                                                                            item.value,
                                                                        'panel__custom-select-option--active':
                                                                            optionIndex ===
                                                                            activeSelectOptionIndex,
                                                                    }"
                                                                    role="option"
                                                                    :aria-selected="
                                                                        option ===
                                                                        item.value
                                                                    "
                                                                    @mouseenter="
                                                                        activeSelectOptionIndex =
                                                                            optionIndex
                                                                    "
                                                                    @click="
                                                                        chooseSelectOption(
                                                                            item,
                                                                            option,
                                                                        )
                                                                    "
                                                                >
                                                                    {{ option }}
                                                                </button>
                                                            </div>
                                                        </Teleport>
                                                    </div>
                                                </template>
                                            </div>
                                        </div>
                                    </div>
                                </template>

                                <!-- Video / custom shader block -->
                                <template v-else-if="block.kind === 'shader'">
                                    <div class="panel__table panel__table--card">
                                        <div
                                            class="panel__row panel__row--card panel__row--shader-header"
                                        >
                                            <div class="panel__card-text">
                                                <div class="panel__card-title">
                                                    Custom Shader
                                                </div>
                                                <div class="panel__card-subtitle">
                                                    Select one or more
                                                    <code>.glsl</code> shader
                                                    files.
                                                </div>
                                            </div>
                                            <div
                                                class="panel__control panel__control--card panel__control--stack"
                                            >
                                                <div
                                                    class="panel__actions panel__actions--inline panel__actions--shader"
                                                >
                                                    <button
                                                        class="panel__action panel__action--ghost panel__action--compact"
                                                        type="button"
                                                        @click="
                                                            browseForCustomShaders
                                                        "
                                                    >
                                                        Add Shaders
                                                    </button>
                                                    <button
                                                        class="panel__action panel__action--ghost panel__action--compact"
                                                        type="button"
                                                        :disabled="
                                                            !selectedShaderFiles.length
                                                        "
                                                        @click="clearShaders"
                                                    >
                                                        Clear
                                                    </button>
                                                </div>
                                            </div>
                                        </div>
                                        <div
                                            class="panel__row panel__row--card panel__row--stacked"
                                        >
                                            <div class="panel__shader-list-wrap">
                                                <div
                                                    class="panel__shader-mode-hint"
                                                >
                                                    {{ shaderModeHintText }}
                                                </div>
                                                <div
                                                    v-if="
                                                        !selectedShaderFiles.length
                                                    "
                                                    class="panel__shader-empty"
                                                >
                                                    No shader files selected.
                                                </div>
                                                <div
                                                    v-else
                                                    class="panel__shader-list"
                                                >
                                                    <div
                                                        v-for="shaderPath in visibleShaderFiles"
                                                        :key="shaderPath"
                                                        class="panel__shader-item"
                                                        :class="{
                                                            'panel__shader-item--active':
                                                                getActiveShaderOrder(
                                                                    shaderPath,
                                                                ) !== null,
                                                            'panel__shader-item--unavailable':
                                                                isShaderUnavailable(
                                                                    shaderPath,
                                                                ),
                                                        }"
                                                        role="checkbox"
                                                        :aria-checked="
                                                            getActiveShaderOrder(
                                                                shaderPath,
                                                            ) !== null
                                                        "
                                                        :aria-disabled="
                                                            isShaderUnavailable(
                                                                shaderPath,
                                                            )
                                                        "
                                                        :tabindex="
                                                            isShaderUnavailable(
                                                                shaderPath,
                                                            )
                                                                ? -1
                                                                : 0
                                                        "
                                                        :title="
                                                            isShaderUnavailable(
                                                                shaderPath,
                                                            )
                                                                ? `File not found: ${shaderPath}`
                                                                : shaderPath
                                                        "
                                                        @click="
                                                            toggleShaderEnabled(
                                                                shaderPath,
                                                            )
                                                        "
                                                        @keydown.enter.prevent="
                                                            toggleShaderEnabled(
                                                                shaderPath,
                                                            )
                                                        "
                                                        @keydown.space.prevent="
                                                            toggleShaderEnabled(
                                                                shaderPath,
                                                            )
                                                        "
                                                    >
                                                        <span
                                                            v-if="
                                                                multiShaderEnabled
                                                            "
                                                            class="panel__shader-select"
                                                            :class="{
                                                                'panel__shader-select--active':
                                                                    getActiveShaderOrder(
                                                                        shaderPath,
                                                                    ) !== null,
                                                                'panel__shader-select--unavailable':
                                                                    isShaderUnavailable(
                                                                        shaderPath,
                                                                    ),
                                                            }"
                                                            :title="
                                                                getActiveShaderOrder(
                                                                    shaderPath,
                                                                ) !== null
                                                                    ? `Shader order ${getActiveShaderOrder(
                                                                          shaderPath,
                                                                      )}`
                                                                    : 'Select shader'
                                                            "
                                                        >
                                                            {{
                                                                getActiveShaderOrder(
                                                                    shaderPath,
                                                                ) ?? ""
                                                            }}
                                                        </span>
                                                        <span
                                                            class="panel__shader-name"
                                                        >
                                                            {{
                                                                getShaderDisplayName(
                                                                    shaderPath,
                                                                )
                                                            }}
                                                        </span>
                                                        <span
                                                            v-if="
                                                                isShaderUnavailable(
                                                                    shaderPath,
                                                                )
                                                            "
                                                            class="panel__shader-missing"
                                                        >
                                                            Missing
                                                        </span>
                                                        <button
                                                            class="panel__shader-remove"
                                                            type="button"
                                                            aria-label="Remove shader"
                                                            @click.stop.prevent="
                                                                removeShaderFromList(
                                                                    shaderPath,
                                                                )
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
                                                                <path
                                                                    d="M6 6l12 12M18 6l-12 12"
                                                                />
                                                            </svg>
                                                        </button>
                                                    </div>
                                                </div>
                                                <div
                                                    class="panel__shader-list-footer"
                                                >
                                                    <div
                                                        class="panel__shader-list-footer-left"
                                                    >
                                                        <label
                                                            v-if="
                                                                shouldShowMultiShaderToggle
                                                            "
                                                            class="panel__shader-multi-toggle"
                                                        >
                                                            <span
                                                                class="panel__shader-multi-label"
                                                            >
                                                                Use Multiple
                                                                Shader
                                                            </span>
                                                            <span
                                                                class="panel__toggle panel__toggle--shader-multi"
                                                            >
                                                                <input
                                                                    class="panel__toggle-input"
                                                                    type="checkbox"
                                                                    :checked="
                                                                        multiShaderEnabled
                                                                    "
                                                                    @change="
                                                                        setMultiShaderEnabled(
                                                                            (
                                                                                $event.target as HTMLInputElement
                                                                            )
                                                                                .checked,
                                                                        )
                                                                    "
                                                                />
                                                                <span
                                                                    class="panel__toggle-track"
                                                                >
                                                                    <span
                                                                        class="panel__toggle-thumb"
                                                                    ></span>
                                                                </span>
                                                            </span>
                                                        </label>
                                                    </div>

                                                    <div
                                                        class="panel__shader-list-footer-center"
                                                    >
                                                        <button
                                                            class="panel__shader-mode-switch"
                                                            type="button"
                                                            @click="
                                                                setRenderingMode(
                                                                    isNormalRenderingMode
                                                                        ? 'animeMode'
                                                                        : 'normal',
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

                                                    <div
                                                        class="panel__shader-list-footer-right"
                                                    >
                                                        <div
                                                            v-if="
                                                                shouldShowShaderListCollapseToggle
                                                            "
                                                            class="panel__shader-list-actions"
                                                        >
                                                            <button
                                                                class="panel__action panel__action--ghost panel__action--compact panel__shader-toggle"
                                                                type="button"
                                                                @click="
                                                                    isShaderListExpanded =
                                                                        !isShaderListExpanded
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
                                                                        v-if="
                                                                            isShaderListExpanded
                                                                        "
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

                                <!-- Shortcuts editor -->
                                <template v-else-if="block.kind === 'shortcuts'">
                                    <ShortcutSettings :group="block.group" />
                                </template>

                                <!-- About -->
                                <template v-else-if="block.kind === 'about'">
                                    <div class="panel__table panel__table--card">
                                        <div
                                            class="panel__row panel__row--card"
                                            data-window-no-drag
                                        >
                                            <div class="panel__card-text">
                                                <div class="panel__card-title">
                                                    GitHub
                                                </div>
                                            </div>
                                            <div
                                                class="panel__control panel__control--card"
                                            >
                                                <div class="panel__social-actions">
                                                    <a
                                                        class="panel__link-button"
                                                        href="https://github.com/eashwar83/lumo"
                                                        target="_blank"
                                                        rel="noopener noreferrer"
                                                        data-window-no-drag
                                                        @click.prevent="
                                                            openProjectGithub
                                                        "
                                                    >
                                                        https://github.com/eashwar83/lumo
                                                    </a>
                                                </div>
                                            </div>
                                        </div>
                                        <div class="panel__row panel__row--card">
                                            <div class="panel__card-text">
                                                <div class="panel__card-title">
                                                    Runtime
                                                </div>
                                            </div>
                                            <div
                                                class="panel__control panel__control--card"
                                            >
                                                <span class="panel__value-text">
                                                    Lumo
                                                    {{
                                                        runtimeVersions?.soiaVersion ??
                                                        "Unavailable"
                                                    }}
                                                    · mpv
                                                    {{
                                                        runtimeVersions?.mpvVersion ??
                                                        "Unavailable"
                                                    }}
                                                    · FFmpeg
                                                    {{
                                                        runtimeVersions?.ffmpegVersion ??
                                                        "Unavailable"
                                                    }}
                                                </span>
                                            </div>
                                        </div>
                                        <div
                                            v-if="
                                                shouldShowUpdateButton ||
                                                shouldShowUpdateStatus
                                            "
                                            class="panel__row panel__row--card"
                                        >
                                            <div class="panel__card-text">
                                                <div class="panel__card-title">
                                                    Updates
                                                </div>
                                            </div>
                                            <div
                                                class="panel__control panel__control--card"
                                            >
                                                <div
                                                    v-if="shouldShowUpdateStatus"
                                                    class="panel__update-status"
                                                    aria-live="polite"
                                                >
                                                    <span
                                                        class="panel__spinner"
                                                        aria-hidden="true"
                                                    ></span>
                                                    <div
                                                        class="panel__update-status-text"
                                                    >
                                                        {{ updateStatusText }}
                                                    </div>
                                                </div>
                                                <div
                                                    v-else-if="
                                                        shouldShowUpdateButton
                                                    "
                                                    class="panel__update-action-wrap"
                                                >
                                                    <span
                                                        v-if="
                                                            shouldShowUpdateHint
                                                        "
                                                        class="panel__update-hint"
                                                    >
                                                        {{ updateHintText }}
                                                    </span>
                                                    <button
                                                        class="panel__action panel__action--glow"
                                                        type="button"
                                                        :disabled="
                                                            isUpdateButtonDisabled
                                                        "
                                                        @click="installUpdate"
                                                    >
                                                        <span
                                                            v-if="isUpdateRetry"
                                                            class="panel__status-icon panel__status-icon--failed"
                                                            aria-hidden="true"
                                                        >
                                                            <svg
                                                                class="panel__status-icon-svg"
                                                                viewBox="0 0 20 20"
                                                            >
                                                                <path
                                                                    d="M5.5 5.5l9 9M14.5 5.5l-9 9"
                                                                />
                                                            </svg>
                                                        </span>
                                                        <span>{{
                                                            updateButtonText
                                                        }}</span>
                                                    </button>
                                                </div>
                                            </div>
                                        </div>
                                        <div
                                            v-if="shouldShowSetDefaultMediaButton"
                                            class="panel__row panel__row--card"
                                        >
                                            <div class="panel__card-text">
                                                <div class="panel__card-title">
                                                    Default Media Player
                                                </div>
                                            </div>
                                            <div
                                                class="panel__control panel__control--card"
                                            >
                                                <button
                                                    class="panel__action panel__action--glow"
                                                    type="button"
                                                    :disabled="
                                                        isSetDefaultButtonDisabled ||
                                                        isApplyingMediaAssociation
                                                    "
                                                    @click="
                                                        setMediaAssociationToSoia
                                                    "
                                                >
                                                    <span
                                                        v-if="isSetDefaultSuccess"
                                                        class="panel__status-icon panel__status-icon--success"
                                                        aria-hidden="true"
                                                    >
                                                        <svg
                                                            class="panel__status-icon-svg"
                                                            viewBox="0 0 20 20"
                                                        >
                                                            <path
                                                                d="M4.5 10.5l3.4 3.4 7.6-7.8"
                                                            />
                                                        </svg>
                                                    </span>
                                                    <span>{{
                                                        setDefaultButtonText
                                                    }}</span>
                                                    <span
                                                        v-if="
                                                            isSetDefaultButtonLoading
                                                        "
                                                        class="panel__loading-dots"
                                                        aria-hidden="true"
                                                    >
                                                        <span
                                                            class="panel__loading-dot"
                                                        ></span>
                                                        <span
                                                            class="panel__loading-dot"
                                                        ></span>
                                                        <span
                                                            class="panel__loading-dot"
                                                        ></span>
                                                    </span>
                                                </button>
                                            </div>
                                        </div>
                                        <div class="panel__row panel__row--card">
                                            <div class="panel__card-text">
                                                <div class="panel__card-title">
                                                    Reset All Settings
                                                </div>
                                                <div class="panel__card-subtitle">
                                                    Restore every setting to its
                                                    default value.
                                                </div>
                                            </div>
                                            <div
                                                class="panel__control panel__control--card"
                                            >
                                                <button
                                                    class="panel__reset"
                                                    type="button"
                                                    @click="resetAllSettings"
                                                >
                                                    Reset
                                                </button>
                                            </div>
                                        </div>
                                        <div class="panel__row panel__row--card">
                                            <div class="panel__card-text">
                                                <div class="panel__card-title">
                                                    Clear All Local Data
                                                </div>
                                            </div>
                                            <div
                                                class="panel__control panel__control--card"
                                            >
                                                <button
                                                    class="panel__reset panel__reset--danger"
                                                    type="button"
                                                    :disabled="
                                                        isFactoryResetInProgress
                                                    "
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
                                </template>
                            </div>
                        </template>
                    </div>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped src="../styles/panels.css"></style>
<style scoped>
.settings-modal {
    position: fixed;
    inset: 0;
    z-index: 116;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 48px 24px 24px;
    box-sizing: border-box;
    background: rgba(6, 8, 12, 0.58);
    -webkit-backdrop-filter: saturate(1.05);
    backdrop-filter: saturate(1.05);
}

.settings-modal__card {
    display: flex;
    flex-direction: column;
    width: min(920px, 100%);
    height: min(660px, 100%);
    max-height: 100%;
    border-radius: 16px;
    border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.14));
    background: var(--panel-bg, rgba(22, 25, 32, 0.96));
    box-shadow: 0 24px 70px rgba(0, 0, 0, 0.5);
    overflow: hidden;
    outline: none;
}

.settings-modal__titlebar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 18px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    flex: 0 0 auto;
}

.settings-modal__title {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: 0.01em;
    color: var(--text-color, #f2f4f8);
}

.settings-modal__close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    border: 1px solid transparent;
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
    font-size: 15px;
    cursor: pointer;
    transition: background 0.15s ease, color 0.15s ease;
}

.settings-modal__close:hover {
    background: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.95);
}

.settings-modal__body {
    display: flex;
    min-height: 0;
    flex: 1 1 auto;
}

.settings-rail {
    flex: 0 0 168px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 10px;
    border-right: 1px solid rgba(255, 255, 255, 0.08);
    overflow-y: auto;
}

.settings-rail__item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 11px;
    border: none;
    border-radius: 9px;
    background: transparent;
    color: rgba(255, 255, 255, 0.7);
    font-size: 13px;
    font-weight: 550;
    text-align: left;
    cursor: pointer;
    transition: background 0.15s ease, color 0.15s ease;
}

.settings-rail__item:hover {
    background: rgba(255, 255, 255, 0.06);
    color: rgba(255, 255, 255, 0.92);
}

.settings-rail__item--active {
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
}

.settings-rail__icon {
    width: 18px;
    height: 18px;
    flex: 0 0 auto;
    opacity: 0.9;
}

.settings-rail__label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.settings-modal__content {
    flex: 1 1 auto;
    min-width: 0;
    padding: 20px 22px 26px;
    overflow-y: auto;
}

.settings-section + .settings-section {
    margin-top: 22px;
}

.settings-section__title {
    margin: 0 0 8px;
    font-size: 11px;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: rgba(255, 255, 255, 0.5);
}

.settings-modal-fade-enter-active,
.settings-modal-fade-leave-active {
    transition: opacity 0.16s ease;
}

.settings-modal-fade-enter-from,
.settings-modal-fade-leave-to {
    opacity: 0;
}

/* Light theme */
:root[data-theme="light"] .settings-modal {
    background: rgba(228, 232, 238, 0.62);
}

:root[data-theme="light"] .settings-modal__card {
    border-color: rgba(0, 0, 0, 0.12);
    background: rgba(248, 249, 251, 0.98);
}

:root[data-theme="light"] .settings-modal__titlebar {
    border-bottom-color: rgba(0, 0, 0, 0.08);
}

:root[data-theme="light"] .settings-modal__title {
    color: #1b2736;
}

:root[data-theme="light"] .settings-modal__close {
    color: rgba(27, 39, 54, 0.6);
}

:root[data-theme="light"] .settings-modal__close:hover {
    background: rgba(0, 0, 0, 0.06);
    color: rgba(27, 39, 54, 0.95);
}

:root[data-theme="light"] .settings-rail {
    border-right-color: rgba(0, 0, 0, 0.08);
}

:root[data-theme="light"] .settings-rail__item {
    color: rgba(27, 39, 54, 0.72);
}

:root[data-theme="light"] .settings-rail__item:hover {
    background: rgba(0, 0, 0, 0.05);
    color: rgba(27, 39, 54, 0.95);
}

:root[data-theme="light"] .settings-rail__item--active {
    background: rgba(47, 107, 216, 0.14);
    color: #1b2736;
}

:root[data-theme="light"] .settings-section__title {
    color: rgba(27, 39, 54, 0.55);
}

/* Graphite theme */
:root[data-theme="graphite"] .settings-modal__card {
    background: rgba(28, 31, 38, 0.97);
}

:root[data-theme="graphite"] .settings-section__title {
    color: rgba(220, 226, 234, 0.55);
}
</style>
