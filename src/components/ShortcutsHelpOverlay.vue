<script setup lang="ts">
import { computed } from "vue";
import {
    chordKeyLabels,
    SHORTCUT_ACTIONS,
    SHORTCUT_GROUP_ORDER,
    UNBOUND_CHORD,
} from "../constants/shortcuts";

const props = withDefaults(
    defineProps<{
        open: boolean;
        bindings?: Record<string, string>;
    }>(),
    {
        bindings: () => ({}),
    },
);

const emit = defineEmits<{
    (e: "close"): void;
}>();

type ShortcutRow = {
    keys: string[];
    description: string;
};

type ShortcutGroup = {
    title: string;
    rows: ShortcutRow[];
};

// Rows are derived from the live binding map so the help overlay always
// reflects the user's current (possibly customized) shortcuts.
const groups = computed<ShortcutGroup[]>(() => {
    const byGroup = new Map<string, ShortcutRow[]>();
    SHORTCUT_ACTIONS.forEach((action) => {
        const chord = props.bindings[action.id];
        if (!chord || chord === UNBOUND_CHORD) return;
        const rows = byGroup.get(action.group) ?? [];
        rows.push({ keys: chordKeyLabels(chord), description: action.label });
        byGroup.set(action.group, rows);
    });

    const ordered: ShortcutGroup[] = [];
    SHORTCUT_GROUP_ORDER.forEach((title) => {
        const rows = byGroup.get(title);
        if (rows?.length) ordered.push({ title, rows });
    });

    ordered.push({
        title: "Reserved",
        rows: [
            { keys: ["Esc"], description: "Exit fullscreen / close help" },
            { keys: ["?", "F1"], description: "Show this help" },
        ],
    });

    return ordered;
});
</script>

<template>
    <transition name="shortcuts-fade">
        <div
            v-if="open"
            class="shortcuts-help"
            role="dialog"
            aria-label="Keyboard shortcuts"
            @click.self="emit('close')"
        >
            <div class="shortcuts-help__panel">
                <div class="shortcuts-help__header">
                    <h2 class="shortcuts-help__title">Keyboard shortcuts</h2>
                    <button
                        type="button"
                        class="shortcuts-help__close"
                        aria-label="Close"
                        @click="emit('close')"
                    >
                        ✕
                    </button>
                </div>
                <div class="shortcuts-help__groups">
                    <section
                        v-for="group in groups"
                        :key="group.title"
                        class="shortcuts-help__group"
                    >
                        <h3 class="shortcuts-help__group-title">
                            {{ group.title }}
                        </h3>
                        <div
                            v-for="row in group.rows"
                            :key="`${group.title}-${row.description}`"
                            class="shortcuts-help__row"
                        >
                            <span class="shortcuts-help__keys">
                                <kbd
                                    v-for="(key, index) in row.keys"
                                    :key="index"
                                    >{{ key }}</kbd
                                >
                            </span>
                            <span class="shortcuts-help__description">{{
                                row.description
                            }}</span>
                        </div>
                    </section>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.shortcuts-help {
    position: fixed;
    inset: 0;
    z-index: 220;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(4px);
}

.shortcuts-help__panel {
    width: min(860px, calc(100vw - 48px));
    max-height: min(78vh, 640px);
    display: flex;
    flex-direction: column;
    border-radius: 14px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    background: rgba(24, 27, 32, 0.92);
    color: rgba(255, 255, 255, 0.92);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
    overflow: hidden;
}

.shortcuts-help__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px 10px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.shortcuts-help__title {
    margin: 0;
    font-size: 15px;
    font-weight: 650;
    letter-spacing: 0.01em;
}

.shortcuts-help__close {
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.66);
    font-size: 14px;
    line-height: 1;
    padding: 6px 8px;
    border-radius: 6px;
    cursor: pointer;
}

.shortcuts-help__close:hover {
    background: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.95);
}

.shortcuts-help__groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 18px 26px;
    padding: 16px 18px 20px;
    overflow: auto;
}

.shortcuts-help__group-title {
    margin: 0 0 8px;
    font-size: 12px;
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: rgba(255, 255, 255, 0.55);
}

.shortcuts-help__row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 3px 0;
}

.shortcuts-help__keys {
    display: inline-flex;
    gap: 4px;
    flex: 0 0 118px;
}

.shortcuts-help__keys kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 20px;
    padding: 2px 6px;
    border-radius: 5px;
    border: 1px solid rgba(255, 255, 255, 0.22);
    background: rgba(255, 255, 255, 0.08);
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
    line-height: 1.4;
    color: rgba(255, 255, 255, 0.9);
}

.shortcuts-help__description {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.78);
}

.shortcuts-fade-enter-active,
.shortcuts-fade-leave-active {
    transition: opacity 0.18s ease;
}

.shortcuts-fade-enter-from,
.shortcuts-fade-leave-to {
    opacity: 0;
}

:root[data-theme="light"] .shortcuts-help {
    background: rgba(46, 58, 74, 0.28);
}

:root[data-theme="light"] .shortcuts-help__panel {
    border-color: rgba(0, 0, 0, 0.12);
    background: rgba(250, 251, 253, 0.96);
    color: rgba(27, 39, 54, 0.92);
    box-shadow: 0 24px 64px rgba(31, 43, 59, 0.28);
}

:root[data-theme="light"] .shortcuts-help__header {
    border-bottom-color: rgba(0, 0, 0, 0.08);
}

:root[data-theme="light"] .shortcuts-help__close {
    color: rgba(27, 39, 54, 0.6);
}

:root[data-theme="light"] .shortcuts-help__close:hover {
    background: rgba(0, 0, 0, 0.07);
    color: rgba(27, 39, 54, 0.92);
}

:root[data-theme="light"] .shortcuts-help__group-title {
    color: rgba(27, 39, 54, 0.55);
}

:root[data-theme="light"] .shortcuts-help__keys kbd {
    border-color: rgba(0, 0, 0, 0.18);
    background: rgba(0, 0, 0, 0.05);
    color: rgba(27, 39, 54, 0.9);
}

:root[data-theme="light"] .shortcuts-help__description {
    color: rgba(27, 39, 54, 0.78);
}

:root[data-theme="graphite"] .shortcuts-help__panel {
    border-color: rgba(146, 158, 175, 0.3);
    background: rgba(30, 33, 38, 0.95);
    color: rgba(237, 241, 246, 0.94);
}

:root[data-theme="graphite"] .shortcuts-help__group-title {
    color: rgba(220, 226, 234, 0.6);
}

:root[data-theme="graphite"] .shortcuts-help__keys kbd {
    border-color: rgba(188, 196, 208, 0.32);
    background: rgba(188, 196, 208, 0.12);
    color: rgba(237, 241, 246, 0.92);
}

:root[data-theme="graphite"] .shortcuts-help__description {
    color: rgba(221, 227, 236, 0.8);
}
</style>
