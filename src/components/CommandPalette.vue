<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { CommandDef } from "../types/commands";
import { chordKeyLabels } from "../constants/shortcuts";

// Search-and-run over the whole command registry. Adjustables are offered as
// "nudge up / nudge down" by their own default step, since a palette has no
// room for a value editor — bind a key in the builder for a specific amount.

const props = defineProps<{
    open: boolean;
    commands: CommandDef[];
    /** Formatted accelerator for a command, when one is bound. */
    chordFor: (commandId: string) => string | undefined;
}>();

const emit = defineEmits<{ (e: "close"): void }>();

type Row = {
    key: string;
    label: string;
    group: string;
    hint?: string;
    run: () => void | Promise<void>;
};

const query = ref("");
const activeIndex = ref(0);
const inputRef = ref<HTMLInputElement | null>(null);
const listRef = ref<HTMLElement | null>(null);

const rows = computed<Row[]>(() => {
    const out: Row[] = [];
    props.commands.forEach((command) => {
        if (command.kind === "action") {
            out.push({
                key: command.id,
                label: command.label,
                group: command.group,
                hint: props.chordFor(command.id),
                run: command.run,
            });
            return;
        }
        const step = command.spec.step;
        const unit = command.spec.unit ?? "";
        const clamp = (value: number) =>
            Math.min(command.spec.max, Math.max(command.spec.min, value));
        out.push({
            key: `${command.id}:up`,
            label: `${command.label} +${step}${unit}`,
            group: command.group,
            run: () => command.set(clamp(command.get() + step)),
        });
        out.push({
            key: `${command.id}:down`,
            label: `${command.label} −${step}${unit}`,
            group: command.group,
            run: () => command.set(clamp(command.get() - step)),
        });
    });
    return out;
});

// Ranks whole-word and prefix hits above mid-word ones so typing "night" puts
// the night-mode entries first.
const score = (row: Row, needle: string): number => {
    const label = row.label.toLowerCase();
    const group = row.group.toLowerCase();
    if (label.startsWith(needle)) return 0;
    const wordHit = label.split(/\s+/).some((word) => word.startsWith(needle));
    if (wordHit) return 1;
    if (label.includes(needle)) return 2;
    if (group.includes(needle)) return 3;
    return -1;
};

const results = computed(() => {
    const needle = query.value.trim().toLowerCase();
    if (!needle) return rows.value.slice(0, 60);
    return rows.value
        .map((row) => ({ row, rank: score(row, needle) }))
        .filter((entry) => entry.rank >= 0)
        .sort((a, b) => a.rank - b.rank)
        .slice(0, 60)
        .map((entry) => entry.row);
});

watch(results, () => {
    activeIndex.value = 0;
});

watch(
    () => props.open,
    async (open) => {
        if (!open) return;
        query.value = "";
        activeIndex.value = 0;
        await nextTick();
        inputRef.value?.focus();
    },
);

const scrollActiveIntoView = async () => {
    await nextTick();
    listRef.value
        ?.querySelector<HTMLElement>("[data-active='true']")
        ?.scrollIntoView({ block: "nearest" });
};

const move = (delta: number) => {
    if (!results.value.length) return;
    const count = results.value.length;
    activeIndex.value = (activeIndex.value + delta + count) % count;
    void scrollActiveIntoView();
};

const runRow = (row: Row) => {
    emit("close");
    void row.run();
};

const onEnter = () => {
    const row = results.value[activeIndex.value];
    if (row) runRow(row);
};
</script>

<template>
    <transition name="palette">
        <div v-if="props.open" class="palette" data-window-no-drag>
            <div class="palette__backdrop" @click="emit('close')"></div>
            <div class="palette__box" role="dialog" aria-label="Command palette">
                <input
                    ref="inputRef"
                    v-model="query"
                    class="palette__input"
                    type="text"
                    placeholder="Run a command…"
                    spellcheck="false"
                    @keydown.down.prevent="move(1)"
                    @keydown.up.prevent="move(-1)"
                    @keydown.enter.prevent="onEnter"
                    @keydown.esc.prevent.stop="emit('close')"
                    @keydown.stop
                />
                <div ref="listRef" class="palette__list">
                    <button
                        v-for="(row, index) in results"
                        :key="row.key"
                        class="palette__row"
                        :class="{ 'palette__row--active': index === activeIndex }"
                        :data-active="index === activeIndex ? 'true' : 'false'"
                        type="button"
                        @mouseenter="activeIndex = index"
                        @click="runRow(row)"
                    >
                        <span class="palette__label">{{ row.label }}</span>
                        <span class="palette__group">{{ row.group }}</span>
                        <span v-if="row.hint" class="palette__keys">
                            <kbd
                                v-for="(label, i) in chordKeyLabels(row.hint)"
                                :key="i"
                                >{{ label }}</kbd
                            >
                        </span>
                    </button>
                    <p v-if="!results.length" class="palette__empty">
                        No matching command
                    </p>
                </div>
                <div class="palette__footer">
                    <span><kbd>↑</kbd><kbd>↓</kbd> navigate</span>
                    <span><kbd>Enter</kbd> run</span>
                    <span><kbd>Esc</kbd> close</span>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.palette {
    position: fixed;
    inset: 0;
    z-index: 210;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
}

.palette__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}

.palette__box {
    position: relative;
    width: min(620px, 92vw);
    max-height: 66vh;
    display: flex;
    flex-direction: column;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    overflow: hidden;
}

.palette__input {
    padding: 15px 18px;
    border: none;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    background: transparent;
    color: #fff;
    font-size: 15px;
    outline: none;
}

.palette__list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
}

.palette__row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 11px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: rgba(255, 255, 255, 0.9);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
}

.palette__row--active {
    background: rgba(255, 255, 255, 0.14);
}

.palette__label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.palette__group {
    flex: none;
    font-size: 11px;
    color: rgba(255, 255, 255, 0.4);
}

.palette__keys {
    display: flex;
    gap: 3px;
}

.palette__keys kbd,
.palette__footer kbd {
    padding: 1px 6px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.14);
    font-family: inherit;
    font-size: 10.5px;
}

.palette__empty {
    margin: 14px;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.45);
}

.palette__footer {
    display: flex;
    gap: 14px;
    padding: 8px 14px;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    font-size: 11px;
    color: rgba(255, 255, 255, 0.42);
}

.palette__footer kbd {
    margin-right: 3px;
}

.palette-enter-active,
.palette-leave-active {
    transition: opacity 0.14s ease;
}

.palette-enter-from,
.palette-leave-to {
    opacity: 0;
}
</style>
