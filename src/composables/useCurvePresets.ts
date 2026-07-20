import { onMounted, ref } from "vue";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";
import { parseCurves, serializeCurves, type Curves } from "../utils/curves";

// User-saved curve presets: a named snapshot of the four channel curves,
// persisted globally (a reusable library, like custom video presets). Stored as
// the serialized curves string so it round-trips through the same validation as
// per-file curve memory.

export type CustomCurvePreset = {
    id: string;
    name: string;
    curves: Curves;
};

export const useCurvePresets = () => {
    const customPresets = ref<CustomCurvePreset[]>([]);
    const saver = createDebouncedUiStateSaver(300);

    const persist = () => {
        saver.saveDebounced({
            customCurvePresets: customPresets.value.map((p) => ({
                id: p.id,
                name: p.name,
                data: serializeCurves(p.curves),
            })),
        });
    };

    // Deterministic-enough id without Date.now/Math.random (unavailable here).
    let seq = 0;
    const nextId = () => {
        seq += 1;
        return `curve-custom-${customPresets.value.length}-${seq}`;
    };

    const saveCurrent = (rawName: string, curves: Curves) => {
        const name = rawName.trim();
        if (!name) return;
        customPresets.value = [
            ...customPresets.value,
            { id: nextId(), name, curves: parseCurves(serializeCurves(curves)) },
        ];
        persist();
    };

    const remove = (id: string) => {
        customPresets.value = customPresets.value.filter((p) => p.id !== id);
        persist();
    };

    onMounted(async () => {
        const stored = await loadUiState<{
            customCurvePresets?: Array<{
                id?: string;
                name?: string;
                data?: string;
            }>;
        }>();
        const list = stored?.customCurvePresets;
        if (Array.isArray(list)) {
            customPresets.value = list
                .filter((p) => p && typeof p.name === "string" && p.name.trim())
                .map((p, index) => ({
                    id: p.id?.trim() || `curve-custom-restored-${index}`,
                    name: (p.name as string).trim(),
                    curves: parseCurves(p.data ?? ""),
                }));
        }
    });

    return { customPresets, saveCurrent, remove };
};

export type CurvePresetsController = ReturnType<typeof useCurvePresets>;
