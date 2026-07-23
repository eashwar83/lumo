<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import type { KeybindSettingItem, SettingGroup } from "../mock/settings";
import {
    chordFromEvent,
    chordKeyLabels,
    getDefaultChord,
    isModifierCode,
    SHORTCUT_GROUP_ORDER,
    UNBOUND_CHORD,
    type ShortcutActionId,
} from "../constants/shortcuts";
import type { CommandDef } from "../types/commands";
import type { CustomShortcutsController } from "../composables/useCustomShortcuts";
import CustomShortcutBuilder from "./CustomShortcutBuilder.vue";

const props = defineProps<{
    group: SettingGroup;
    commands?: CommandDef[];
    custom?: CustomShortcutsController;
}>();

/** Chords held by built-in actions — those win over custom bindings. */
const builtInChords = computed(() => {
    const set = new Set<string>();
    keybindItems.value.forEach((item) => {
        if (item.value && item.value !== UNBOUND_CHORD) set.add(item.value);
    });
    return set;
});

const builtInLabelFor = (chord: string): string | undefined => {
    const match = keybindItems.value.find((item) => item.value === chord);
    return match ? (match.displayLabel ?? match.label) : undefined;
};

const keybindItems = computed(
    () =>
        props.group.items.filter(
            (item): item is KeybindSettingItem => item.type === "keybind",
        ),
);

type Section = { title: string; items: KeybindSettingItem[] };

const sections = computed<Section[]>(() => {
    const byGroup = new Map<string, KeybindSettingItem[]>();
    keybindItems.value.forEach((item) => {
        const key = item.group ?? "Other";
        const bucket = byGroup.get(key);
        if (bucket) {
            bucket.push(item);
        } else {
            byGroup.set(key, [item]);
        }
    });
    const ordered: Section[] = [];
    SHORTCUT_GROUP_ORDER.forEach((title) => {
        const items = byGroup.get(title);
        if (items?.length) {
            ordered.push({ title, items });
            byGroup.delete(title);
        }
    });
    byGroup.forEach((items, title) => ordered.push({ title, items }));
    return ordered;
});

const recordingLabel = ref<string | null>(null);
const reassignNote = ref<{ label: string; from: string } | null>(null);
let reassignTimer: number | null = null;
let listenersAttached = false;

const clearReassignNote = () => {
    if (reassignTimer !== null) {
        window.clearTimeout(reassignTimer);
        reassignTimer = null;
    }
    reassignNote.value = null;
};

const showReassignNote = (label: string, from: string) => {
    clearReassignNote();
    reassignNote.value = { label, from };
    reassignTimer = window.setTimeout(() => {
        reassignNote.value = null;
        reassignTimer = null;
    }, 4000);
};

const detachListeners = () => {
    if (!listenersAttached) return;
    window.removeEventListener("keydown", onCaptureKeydown, true);
    window.removeEventListener("pointerdown", onCapturePointerDown, true);
    listenersAttached = false;
};

const stopRecording = () => {
    recordingLabel.value = null;
    detachListeners();
};

const assignChord = (item: KeybindSettingItem, chord: string) => {
    // Warn & steal: if another action holds this chord, unbind it first so no
    // two actions ever resolve from the same key.
    const conflict = keybindItems.value.find(
        (candidate) =>
            candidate.label !== item.label && candidate.value === chord,
    );
    if (conflict) {
        conflict.value = UNBOUND_CHORD;
        showReassignNote(
            item.label,
            conflict.displayLabel ?? conflict.label,
        );
    } else {
        clearReassignNote();
    }
    item.value = chord;
};

function onCaptureKeydown(event: KeyboardEvent) {
    if (recordingLabel.value === null) return;
    event.preventDefault();
    event.stopImmediatePropagation();

    if (event.code === "Escape") {
        stopRecording();
        return;
    }
    // Wait for a non-modifier key so the chord always has a main key.
    if (isModifierCode(event.code)) return;

    const chord = chordFromEvent(event);
    if (!chord) return;

    const item = keybindItems.value.find(
        (candidate) => candidate.label === recordingLabel.value,
    );
    if (item) {
        assignChord(item, chord);
    }
    stopRecording();
}

function onCapturePointerDown(event: PointerEvent) {
    if (recordingLabel.value === null) return;
    const target = event.target as HTMLElement | null;
    // Clicking the active record button again (or its pill) shouldn't cancel.
    if (target?.closest("[data-recording-active='true']")) return;
    stopRecording();
}

const startRecording = (item: KeybindSettingItem) => {
    if (recordingLabel.value === item.label) {
        stopRecording();
        return;
    }
    clearReassignNote();
    recordingLabel.value = item.label;
    if (!listenersAttached) {
        // Defer so the click that started recording doesn't immediately cancel
        // via the pointerdown listener.
        window.setTimeout(() => {
            if (recordingLabel.value === null) return;
            window.addEventListener("keydown", onCaptureKeydown, true);
            window.addEventListener("pointerdown", onCapturePointerDown, true);
            listenersAttached = true;
        }, 0);
    }
};

const resetOne = (item: KeybindSettingItem) => {
    if (recordingLabel.value !== null) stopRecording();
    item.value = getDefaultChord(item.label as ShortcutActionId);
};

const clearOne = (item: KeybindSettingItem) => {
    if (recordingLabel.value !== null) stopRecording();
    item.value = UNBOUND_CHORD;
};

const resetAll = () => {
    if (recordingLabel.value !== null) stopRecording();
    clearReassignNote();
    keybindItems.value.forEach((item) => {
        item.value = getDefaultChord(item.label as ShortcutActionId);
    });
};

const isUnbound = (item: KeybindSettingItem) =>
    !item.value || item.value === UNBOUND_CHORD;

const isDefault = (item: KeybindSettingItem) =>
    item.value === getDefaultChord(item.label as ShortcutActionId);

onBeforeUnmount(() => {
    detachListeners();
    if (reassignTimer !== null) window.clearTimeout(reassignTimer);
});
</script>

<template>
    <div class="shortcut-settings">
        <div class="shortcut-settings__intro">
            <p class="shortcut-settings__hint">
                Click a shortcut to record a new key. Modifiers (Ctrl / Alt /
                Shift) are supported. <kbd>Esc</kbd> and <kbd>F1</kbd> / <kbd>?</kbd>
                are reserved.
            </p>
            <button
                class="shortcut-settings__reset-all"
                type="button"
                @click="resetAll"
            >
                Reset to defaults
            </button>
        </div>

        <section
            v-for="section in sections"
            :key="section.title"
            class="shortcut-settings__section"
        >
            <h4 class="shortcut-settings__section-title">
                {{ section.title }}
            </h4>
            <div class="shortcut-settings__list">
                <div
                    v-for="item in section.items"
                    :key="item.label"
                    class="shortcut-settings__row"
                >
                    <div class="shortcut-settings__label">
                        <span class="shortcut-settings__name">
                            {{ item.displayLabel ?? item.label }}
                        </span>
                        <span
                            v-if="
                                reassignNote &&
                                reassignNote.label === item.label
                            "
                            class="shortcut-settings__note"
                        >
                            Reassigned from {{ reassignNote.from }}
                        </span>
                    </div>
                    <div class="shortcut-settings__control">
                        <button
                            class="shortcut-settings__capture"
                            :class="{
                                'shortcut-settings__capture--recording':
                                    recordingLabel === item.label,
                                'shortcut-settings__capture--unbound':
                                    isUnbound(item),
                            }"
                            type="button"
                            :data-recording-active="
                                recordingLabel === item.label ? 'true' : 'false'
                            "
                            @click="startRecording(item)"
                        >
                            <template v-if="recordingLabel === item.label">
                                <span class="shortcut-settings__recording-text">
                                    Press a key…
                                </span>
                            </template>
                            <template v-else-if="isUnbound(item)">
                                <span class="shortcut-settings__unbound-text">
                                    Not set
                                </span>
                            </template>
                            <template v-else>
                                <kbd
                                    v-for="(key, index) in chordKeyLabels(
                                        item.value,
                                    )"
                                    :key="index"
                                    class="shortcut-settings__kbd"
                                    >{{ key }}</kbd
                                >
                            </template>
                        </button>
                        <button
                            class="shortcut-settings__icon-btn"
                            type="button"
                            title="Reset to default"
                            aria-label="Reset to default"
                            :disabled="isDefault(item)"
                            @click="resetOne(item)"
                        >
                            <svg
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <path d="M3 12a9 9 0 1 0 3-6.7L3 8" />
                                <path d="M3 3v5h5" />
                            </svg>
                        </button>
                        <button
                            class="shortcut-settings__icon-btn"
                            type="button"
                            title="Clear shortcut"
                            aria-label="Clear shortcut"
                            :disabled="isUnbound(item)"
                            @click="clearOne(item)"
                        >
                            <svg
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <path d="M6 6l12 12M18 6l-12 12" />
                            </svg>
                        </button>
                    </div>
                </div>
            </div>
        </section>

        <CustomShortcutBuilder
            v-if="props.commands && props.custom"
            :commands="props.commands"
            :custom="props.custom"
            :built-in-chords="builtInChords"
            :built-in-label-for="builtInLabelFor"
        />
    </div>
</template>

<style scoped>
.shortcut-settings {
    display: flex;
    flex-direction: column;
    gap: 18px;
}

.shortcut-settings__intro {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
}

.shortcut-settings__hint {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.6);
    max-width: 640px;
}

.shortcut-settings__hint kbd {
    padding: 1px 5px;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.22);
    background: rgba(255, 255, 255, 0.08);
    font-family: inherit;
    font-size: 11px;
}

.shortcut-settings__reset-all {
    flex: 0 0 auto;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: transparent;
    color: rgba(255, 255, 255, 0.8);
    font-size: 12px;
    font-weight: 600;
    padding: 6px 12px;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease;
}

.shortcut-settings__reset-all:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.28);
}

.shortcut-settings__section {
    display: flex;
    flex-direction: column;
    gap: 6px;
}

.shortcut-settings__section-title {
    margin: 0 0 2px;
    font-size: 11px;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: rgba(255, 255, 255, 0.5);
}

.shortcut-settings__list {
    display: flex;
    flex-direction: column;
    border-radius: 12px;
    border: 1px solid rgba(255, 255, 255, 0.08);
    overflow: hidden;
}

.shortcut-settings__row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 14px;
    background: rgba(255, 255, 255, 0.02);
}

.shortcut-settings__row + .shortcut-settings__row {
    border-top: 1px solid rgba(255, 255, 255, 0.06);
}

.shortcut-settings__label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
}

.shortcut-settings__name {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.86);
}

.shortcut-settings__note {
    font-size: 11px;
    color: #f0b45f;
}

.shortcut-settings__control {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 0 0 auto;
}

.shortcut-settings__capture {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 128px;
    min-height: 32px;
    justify-content: center;
    padding: 4px 10px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.92);
    font-family: inherit;
    font-size: 12px;
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease;
}

.shortcut-settings__capture:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.28);
}

.shortcut-settings__capture--recording {
    border-color: #4c8dff;
    border-style: dashed;
    background: rgba(76, 141, 255, 0.14);
}

.shortcut-settings__capture--unbound {
    color: rgba(255, 255, 255, 0.4);
}

.shortcut-settings__recording-text {
    color: #7fb0ff;
    font-weight: 600;
}

.shortcut-settings__unbound-text {
    font-style: italic;
}

.shortcut-settings__kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    padding: 2px 6px;
    border-radius: 5px;
    border: 1px solid rgba(255, 255, 255, 0.24);
    background: rgba(255, 255, 255, 0.1);
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
    line-height: 1.3;
}

.shortcut-settings__icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border-radius: 8px;
    border: 1px solid transparent;
    background: transparent;
    color: rgba(255, 255, 255, 0.55);
    cursor: pointer;
    transition: background 0.15s ease, color 0.15s ease;
}

.shortcut-settings__icon-btn svg {
    width: 15px;
    height: 15px;
}

.shortcut-settings__icon-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.9);
}

.shortcut-settings__icon-btn:disabled {
    opacity: 0.3;
    cursor: default;
}

/* Light theme */
:root[data-theme="light"] .shortcut-settings__hint {
    color: rgba(27, 39, 54, 0.62);
}

:root[data-theme="light"] .shortcut-settings__hint kbd {
    border-color: rgba(0, 0, 0, 0.18);
    background: rgba(0, 0, 0, 0.05);
    color: rgba(27, 39, 54, 0.9);
}

:root[data-theme="light"] .shortcut-settings__reset-all {
    border-color: rgba(0, 0, 0, 0.16);
    color: rgba(27, 39, 54, 0.82);
}

:root[data-theme="light"] .shortcut-settings__reset-all:hover {
    background: rgba(0, 0, 0, 0.05);
    border-color: rgba(0, 0, 0, 0.26);
}

:root[data-theme="light"] .shortcut-settings__section-title {
    color: rgba(27, 39, 54, 0.5);
}

:root[data-theme="light"] .shortcut-settings__list {
    border-color: rgba(0, 0, 0, 0.1);
}

:root[data-theme="light"] .shortcut-settings__row {
    background: rgba(0, 0, 0, 0.015);
}

:root[data-theme="light"] .shortcut-settings__row + .shortcut-settings__row {
    border-top-color: rgba(0, 0, 0, 0.07);
}

:root[data-theme="light"] .shortcut-settings__name {
    color: rgba(27, 39, 54, 0.88);
}

:root[data-theme="light"] .shortcut-settings__capture {
    border-color: rgba(0, 0, 0, 0.16);
    background: rgba(0, 0, 0, 0.03);
    color: rgba(27, 39, 54, 0.92);
}

:root[data-theme="light"] .shortcut-settings__capture:hover {
    background: rgba(0, 0, 0, 0.06);
    border-color: rgba(0, 0, 0, 0.26);
}

:root[data-theme="light"] .shortcut-settings__capture--recording {
    border-color: #2f6bd8;
    background: rgba(47, 107, 216, 0.1);
}

:root[data-theme="light"] .shortcut-settings__recording-text {
    color: #2f6bd8;
}

:root[data-theme="light"] .shortcut-settings__kbd {
    border-color: rgba(0, 0, 0, 0.2);
    background: rgba(0, 0, 0, 0.05);
    color: rgba(27, 39, 54, 0.9);
}

:root[data-theme="light"] .shortcut-settings__icon-btn {
    color: rgba(27, 39, 54, 0.5);
}

:root[data-theme="light"] .shortcut-settings__icon-btn:hover:not(:disabled) {
    background: rgba(0, 0, 0, 0.06);
    color: rgba(27, 39, 54, 0.85);
}

/* Graphite theme */
:root[data-theme="graphite"] .shortcut-settings__section-title {
    color: rgba(220, 226, 234, 0.55);
}

:root[data-theme="graphite"] .shortcut-settings__kbd {
    border-color: rgba(188, 196, 208, 0.32);
    background: rgba(188, 196, 208, 0.14);
    color: rgba(237, 241, 246, 0.92);
}
</style>
