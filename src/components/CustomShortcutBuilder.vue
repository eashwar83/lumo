<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";
import {
    chordFromEvent,
    chordKeyLabels,
    isModifierCode,
} from "../constants/shortcuts";
import type { AdjustMode, CommandDef } from "../types/commands";
import { describeCustomShortcut } from "../types/commands";
import type { CustomShortcutsController } from "../composables/useCustomShortcuts";

// Build a shortcut for anything the app can do: a discrete command, a numeric
// setting (increase / decrease / set), or a raw mpv command for things with no
// Lumo UI at all.

const props = defineProps<{
    commands: CommandDef[];
    custom: CustomShortcutsController;
    /** Chords already claimed by built-in actions; those always win. */
    builtInChords: Set<string>;
    builtInLabelFor: (chord: string) => string | undefined;
}>();

type DraftKind = "action" | "adjust" | "mpv";

const kind = ref<DraftKind>("action");
const search = ref("");
const commandId = ref("");
const mode = ref<AdjustMode>("increase");
const amount = ref<number | null>(null);
const mpvCommand = ref("");
const chord = ref("");
const recording = ref(false);

const visibleCommands = computed(() => {
    const wanted = kind.value === "adjust" ? "adjust" : "action";
    const query = search.value.trim().toLowerCase();
    return props.commands
        .filter((command) => command.kind === wanted)
        .filter(
            (command) =>
                !query ||
                command.label.toLowerCase().includes(query) ||
                command.group.toLowerCase().includes(query),
        );
});

const grouped = computed(() => {
    const map = new Map<string, CommandDef[]>();
    visibleCommands.value.forEach((command) => {
        const bucket = map.get(command.group);
        if (bucket) bucket.push(command);
        else map.set(command.group, [command]);
    });
    return [...map.entries()].map(([title, items]) => ({ title, items }));
});

const selected = computed(() =>
    props.commands.find((command) => command.id === commandId.value),
);

const selectedSpec = computed(() =>
    selected.value?.kind === "adjust" ? selected.value.spec : null,
);

/** Falls back to the command's own step when the user hasn't typed one. */
const effectiveAmount = computed(() => {
    if (amount.value !== null && Number.isFinite(amount.value)) return amount.value;
    return selectedSpec.value?.step ?? 1;
});

const conflictLabel = computed(() => {
    if (!chord.value) return null;
    return props.builtInLabelFor(chord.value) ?? null;
});

const existingCustom = computed(() =>
    chord.value ? props.custom.findByChord(chord.value) : undefined,
);

const canAdd = computed(() => {
    if (!chord.value || conflictLabel.value) return false;
    if (kind.value === "mpv") return mpvCommand.value.trim().length > 0;
    return Boolean(selected.value);
});

// --- chord capture ---------------------------------------------------------

const onCaptureKeydown = (event: KeyboardEvent) => {
    if (!recording.value) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    if (event.code === "Escape") {
        stopRecording();
        return;
    }
    if (isModifierCode(event.code)) return;
    const captured = chordFromEvent(event);
    if (captured) chord.value = captured;
    stopRecording();
};

const onCapturePointerDown = (event: PointerEvent) => {
    if (!recording.value) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest("[data-builder-recording='true']")) return;
    stopRecording();
};

function stopRecording() {
    recording.value = false;
    window.removeEventListener("keydown", onCaptureKeydown, true);
    window.removeEventListener("pointerdown", onCapturePointerDown, true);
}

const startRecording = () => {
    if (recording.value) {
        stopRecording();
        return;
    }
    recording.value = true;
    window.setTimeout(() => {
        if (!recording.value) return;
        window.addEventListener("keydown", onCaptureKeydown, true);
        window.addEventListener("pointerdown", onCapturePointerDown, true);
    }, 0);
};

const reset = () => {
    search.value = "";
    commandId.value = "";
    mpvCommand.value = "";
    chord.value = "";
    amount.value = null;
    mode.value = "increase";
};

const onAdd = () => {
    if (!canAdd.value) return;
    props.custom.add({
        chord: chord.value,
        kind: kind.value,
        commandId: kind.value === "mpv" ? undefined : commandId.value,
        mode: kind.value === "adjust" ? mode.value : undefined,
        amount: kind.value === "adjust" ? effectiveAmount.value : undefined,
        mpvCommand: kind.value === "mpv" ? mpvCommand.value.trim() : undefined,
    });
    reset();
};

const describe = (entry: Parameters<typeof describeCustomShortcut>[0]) =>
    describeCustomShortcut(
        entry,
        entry.commandId
            ? props.commands.find((command) => command.id === entry.commandId)
            : undefined,
    );

onBeforeUnmount(stopRecording);
</script>

<template>
    <section class="builder">
        <h4 class="builder__title">Custom shortcuts</h4>
        <p class="builder__hint">
            Bind a key to anything — including sliders like Sharpness and Film
            Grain, which can't appear in a menu. Built-in shortcuts always take
            priority over custom ones.
        </p>

        <div class="builder__kinds">
            <button
                v-for="option in [
                    { id: 'action', label: 'Command' },
                    { id: 'adjust', label: 'Adjust a value' },
                    { id: 'mpv', label: 'mpv command' },
                ]"
                :key="option.id"
                class="builder__kind"
                :class="{ 'builder__kind--active': kind === option.id }"
                type="button"
                @click="
                    kind = option.id as DraftKind;
                    commandId = '';
                "
            >
                {{ option.label }}
            </button>
        </div>

        <!-- Command / adjustable picker -->
        <template v-if="kind !== 'mpv'">
            <input
                v-model="search"
                class="builder__search"
                type="text"
                :placeholder="
                    kind === 'adjust'
                        ? 'Search values — sharpness, grain, contrast…'
                        : 'Search commands — panel, preset, crop…'
                "
            />
            <div class="builder__picker">
                <div v-for="group in grouped" :key="group.title">
                    <div class="builder__group">{{ group.title }}</div>
                    <button
                        v-for="command in group.items"
                        :key="command.id"
                        class="builder__option"
                        :class="{
                            'builder__option--active': commandId === command.id,
                        }"
                        type="button"
                        @click="commandId = command.id"
                    >
                        {{ command.label }}
                    </button>
                </div>
                <p v-if="!grouped.length" class="builder__empty">No matches</p>
            </div>
        </template>

        <!-- Raw mpv command -->
        <template v-else>
            <input
                v-model="mpvCommand"
                class="builder__search"
                type="text"
                placeholder="e.g. cycle deinterlace"
            />
            <p class="builder__hint builder__hint--small">
                Runs verbatim through mpv. Not validated here — if it's wrong,
                nothing happens and the error is logged.
            </p>
        </template>

        <!-- Adjust options -->
        <div v-if="kind === 'adjust' && selectedSpec" class="builder__adjust">
            <div class="builder__modes">
                <button
                    v-for="option in [
                        { id: 'increase', label: 'Increase' },
                        { id: 'decrease', label: 'Decrease' },
                        { id: 'set', label: 'Set to' },
                    ]"
                    :key="option.id"
                    class="builder__kind"
                    :class="{ 'builder__kind--active': mode === option.id }"
                    type="button"
                    @click="mode = option.id as AdjustMode"
                >
                    {{ option.label }}
                </button>
            </div>
            <label class="builder__amount">
                <span>{{ mode === "set" ? "Value" : "Step" }}</span>
                <input
                    v-model.number="amount"
                    type="number"
                    :min="mode === 'set' ? selectedSpec.min : 0"
                    :max="selectedSpec.max"
                    :step="selectedSpec.step"
                    :placeholder="String(selectedSpec.step)"
                />
                <span class="builder__unit">{{ selectedSpec.unit }}</span>
            </label>
            <p class="builder__range">
                Range {{ selectedSpec.min }}–{{ selectedSpec.max
                }}{{ selectedSpec.unit }}
            </p>
        </div>

        <!-- Chord + add -->
        <div class="builder__assign">
            <button
                class="builder__capture"
                :class="{ 'builder__capture--recording': recording }"
                type="button"
                :data-builder-recording="recording ? 'true' : 'false'"
                @click="startRecording"
            >
                <span v-if="recording">Press a key…</span>
                <template v-else-if="chord">
                    <kbd
                        v-for="(label, index) in chordKeyLabels(chord)"
                        :key="index"
                        >{{ label }}</kbd
                    >
                </template>
                <span v-else>Set key…</span>
            </button>
            <button
                class="builder__add"
                type="button"
                :disabled="!canAdd"
                @click="onAdd"
            >
                Add shortcut
            </button>
        </div>

        <p v-if="conflictLabel" class="builder__warn">
            That key is already used by the built-in “{{ conflictLabel }}”.
            Built-ins win, so pick another key — or rebind that action above.
        </p>
        <p v-else-if="existingCustom" class="builder__warn builder__warn--soft">
            Replaces your existing “{{ describe(existingCustom) }}” shortcut.
        </p>

        <!-- Existing custom shortcuts -->
        <div v-if="props.custom.shortcuts.value.length" class="builder__list">
            <div
                v-for="entry in props.custom.shortcuts.value"
                :key="entry.id"
                class="builder__row"
            >
                <span class="builder__row-label">{{ describe(entry) }}</span>
                <span class="builder__row-keys">
                    <kbd
                        v-for="(label, index) in chordKeyLabels(entry.chord)"
                        :key="index"
                        >{{ label }}</kbd
                    >
                </span>
                <button
                    class="builder__remove"
                    type="button"
                    aria-label="Remove shortcut"
                    @click="props.custom.remove(entry.id)"
                >
                    ✕
                </button>
            </div>
        </div>
    </section>
</template>

<style scoped>
.builder {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    margin-top: 8px;
    border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.12));
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.03);
}

.builder__title {
    margin: 0;
    font-size: 13px;
    font-weight: 700;
}

.builder__hint {
    margin: 0;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-muted, rgba(255, 255, 255, 0.55));
}

.builder__hint--small {
    font-size: 11.5px;
}

.builder__kinds,
.builder__modes {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
}

.builder__kind {
    padding: 6px 12px;
    border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.14));
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}

.builder__kind--active {
    background: var(--progress-color, #4a9eff);
    border-color: transparent;
    color: #0b0d10;
}

.builder__search,
.builder__amount input {
    padding: 8px 10px;
    border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.14));
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.22);
    color: inherit;
    font-size: 12.5px;
}

.builder__picker {
    max-height: 220px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.1));
    border-radius: 8px;
}

.builder__group {
    padding: 6px 8px 3px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--text-muted, rgba(255, 255, 255, 0.45));
}

.builder__option {
    padding: 6px 9px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
}

.builder__option:hover {
    background: rgba(255, 255, 255, 0.1);
}

.builder__option--active {
    background: var(--progress-color, #4a9eff);
    color: #0b0d10;
    font-weight: 600;
}

.builder__empty {
    margin: 8px;
    font-size: 12px;
    color: var(--text-muted, rgba(255, 255, 255, 0.45));
}

.builder__adjust {
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.builder__amount {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
}

.builder__amount input {
    width: 96px;
}

.builder__unit,
.builder__range {
    font-size: 11.5px;
    color: var(--text-muted, rgba(255, 255, 255, 0.5));
}

.builder__range {
    margin: 0;
}

.builder__assign {
    display: flex;
    align-items: center;
    gap: 8px;
}

.builder__capture {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    min-height: 34px;
    padding: 6px 12px;
    border: 1px dashed var(--glass-border, rgba(255, 255, 255, 0.22));
    border-radius: 8px;
    background: transparent;
    color: inherit;
    font-size: 12.5px;
    cursor: pointer;
}

.builder__capture--recording {
    border-style: solid;
    border-color: var(--progress-color, #4a9eff);
    color: var(--progress-color, #4a9eff);
}

.builder__capture kbd,
.builder__row-keys kbd {
    padding: 2px 7px;
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.14);
    font-family: inherit;
    font-size: 11.5px;
}

.builder__add {
    padding: 8px 16px;
    border: none;
    border-radius: 8px;
    background: var(--progress-color, #4a9eff);
    color: #0b0d10;
    font-size: 12.5px;
    font-weight: 700;
    cursor: pointer;
}

.builder__add:disabled {
    opacity: 0.4;
    cursor: default;
}

.builder__warn {
    margin: 0;
    font-size: 11.5px;
    line-height: 1.45;
    color: #ffb454;
}

.builder__warn--soft {
    color: var(--text-muted, rgba(255, 255, 255, 0.55));
}

.builder__list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 4px;
}

.builder__row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.05);
}

.builder__row-label {
    flex: 1;
    font-size: 12.5px;
}

.builder__row-keys {
    display: flex;
    gap: 3px;
}

.builder__remove {
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.08);
    color: inherit;
    font-size: 11px;
    cursor: pointer;
}

.builder__remove:hover {
    background: rgba(255, 90, 90, 0.35);
}
</style>
