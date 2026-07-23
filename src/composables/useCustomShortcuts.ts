import { onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { CommandDef, CustomShortcut } from "../types/commands";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";

// User-defined shortcuts, stored separately from the built-in bindings so the
// two can't corrupt each other. Dispatch is chord-keyed and runs only when no
// built-in action claimed the key first.

type UseCustomShortcutsOptions = {
    /** Looks a command id up in the registry; may be absent if it went away. */
    resolve: (commandId: string) => CommandDef | undefined;
    onMessage?: (text: string) => void;
};

const clamp = (value: number, min: number, max: number) =>
    Math.min(max, Math.max(min, value));

const formatValue = (value: number, precision = 0, unit = "") => {
    const rounded = precision > 0 ? value.toFixed(precision) : Math.round(value);
    return `${rounded}${unit}`;
};

/** Ids only need to be unique within this list. */
const nextId = (existing: CustomShortcut[]): string => {
    let n = 1;
    while (existing.some((entry) => entry.id === `cs${n}`)) n += 1;
    return `cs${n}`;
};

export const useCustomShortcuts = ({
    resolve,
    onMessage,
}: UseCustomShortcutsOptions) => {
    const shortcuts = ref<CustomShortcut[]>([]);
    const saver = createDebouncedUiStateSaver(300);
    let loaded = false;

    const persist = () => {
        if (!loaded) return;
        saver.saveDebounced({
            customShortcuts: shortcuts.value.map((entry) => ({ ...entry })),
        });
    };

    const add = (entry: Omit<CustomShortcut, "id">) => {
        // A chord can only mean one thing, so replace any existing binding.
        shortcuts.value = shortcuts.value.filter(
            (existing) => existing.chord !== entry.chord,
        );
        shortcuts.value.push({ ...entry, id: nextId(shortcuts.value) });
        persist();
    };

    const remove = (id: string) => {
        shortcuts.value = shortcuts.value.filter((entry) => entry.id !== id);
        persist();
    };

    const update = (id: string, patch: Partial<CustomShortcut>) => {
        shortcuts.value = shortcuts.value.map((entry) =>
            entry.id === id ? { ...entry, ...patch } : entry,
        );
        persist();
    };

    const clearAll = () => {
        shortcuts.value = [];
        persist();
    };

    /** True when the chord is already taken by a custom shortcut. */
    const findByChord = (chord: string) =>
        shortcuts.value.find((entry) => entry.chord === chord);

    const runMpv = async (command: string) => {
        // mpv takes an argv array; split on whitespace but keep quoted spans.
        const args = command
            .trim()
            .match(/"[^"]*"|\S+/g)
            ?.map((part) => part.replace(/^"|"$/g, ""));
        if (!args?.length) return;
        try {
            await invoke("mpv_run_command", { args });
        } catch (error) {
            console.warn("[custom-shortcut] mpv command failed", { command, error });
            onMessage?.(`mpv: ${command} failed`);
        }
    };

    const runEntry = async (entry: CustomShortcut) => {
        if (entry.kind === "mpv") {
            await runMpv(entry.mpvCommand ?? "");
            return;
        }
        const command = entry.commandId ? resolve(entry.commandId) : undefined;
        if (!command) {
            onMessage?.("That shortcut's command is no longer available");
            return;
        }
        if (command.kind === "action") {
            await command.run();
            return;
        }
        // Adjustable: apply the delta (or absolute), clamp, and report where we
        // landed — without a readout you can't tell where you are in the range.
        const { min, max, unit, precision } = command.spec;
        const amount = entry.amount ?? command.spec.step;
        const current = command.get();
        const next =
            entry.mode === "set"
                ? amount
                : entry.mode === "decrease"
                  ? current - amount
                  : current + amount;
        const applied = clamp(next, min, max);
        await command.set(applied);
        onMessage?.(`${command.label} ${formatValue(applied, precision, unit)}`);
    };

    /** Dispatch a chord. Returns true when a custom shortcut handled it. */
    const runChord = (chord: string): boolean => {
        const entry = findByChord(chord);
        if (!entry) return false;
        void runEntry(entry);
        return true;
    };

    const load = (stored?: CustomShortcut[]) => {
        if (Array.isArray(stored)) {
            shortcuts.value = stored
                .filter((entry) => entry && typeof entry.chord === "string")
                .map<CustomShortcut>((entry) => ({
                    id: String(entry.id ?? ""),
                    chord: entry.chord,
                    kind:
                        entry.kind === "adjust" || entry.kind === "mpv"
                            ? entry.kind
                            : "action",
                    commandId: entry.commandId,
                    mode: entry.mode,
                    amount:
                        typeof entry.amount === "number" ? entry.amount : undefined,
                    mpvCommand: entry.mpvCommand,
                }))
                .map((entry, index) => ({
                    ...entry,
                    id: entry.id || `cs${index + 1}`,
                }));
        }
        loaded = true;
    };

    onMounted(async () => {
        const state = await loadUiState<{ customShortcuts?: CustomShortcut[] }>();
        load(state?.customShortcuts);
    });

    return {
        shortcuts,
        add,
        remove,
        update,
        clearAll,
        findByChord,
        runChord,
    };
};

export type CustomShortcutsController = ReturnType<typeof useCustomShortcuts>;
