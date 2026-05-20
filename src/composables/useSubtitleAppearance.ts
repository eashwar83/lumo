import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SubtitleTarget } from "./useSubtitleState";
import { loadUiState, saveUiState } from "./useUiStateStore";

type TargetValuePayload<T> = {
    target: SubtitleTarget;
    value: T;
};

type PersistedSubtitleAppearanceState = {
    fontFamily?: string;
    fontSize?: number;
    fontColor?: string;
    primarySubPos?: number;
    secondarySubPos?: number;
};

const clamp = (value: number, min: number, max: number) =>
    Math.min(max, Math.max(min, value));

const optionForTarget = (target: SubtitleTarget, base: string) =>
    target === "primary" ? base : `secondary-${base}`;

const positionOptionForTarget = (target: SubtitleTarget) =>
    optionForTarget(target, "sub-pos");

const defaultFontSize = 38;
const defaultPrimarySubPos = 100;
const defaultSecondarySubPos = 0;
const defaultFontColor = "#ffffff";

const normalizeFontFamily = (value: unknown) =>
    typeof value === "string" ? value.trim() : "";

const normalizeFontSize = (value: unknown) =>
    typeof value === "number" && Number.isFinite(value)
        ? clamp(value, 8, 200)
        : defaultFontSize;

const normalizeFontColor = (value: unknown) =>
    typeof value === "string" && /^#[0-9a-f]{6}$/i.test(value.trim())
        ? value.trim()
        : defaultFontColor;

const normalizeSubPos = (value: unknown, fallback: number) =>
    typeof value === "number" && Number.isFinite(value)
        ? clamp(value, 0, 100)
        : fallback;

export const useSubtitleAppearance = () => {
    const primaryFontFamily = ref("");
    const secondaryFontFamily = ref("");
    const primaryFontSize = ref(defaultFontSize);
    const secondaryFontSize = ref(defaultFontSize);
    const primaryFontColor = ref(defaultFontColor);
    const secondaryFontColor = ref(defaultFontColor);
    const primarySubPos = ref(defaultPrimarySubPos);
    const secondarySubPos = ref(defaultSecondarySubPos);

    const getFontFamily = (_target: SubtitleTarget) => primaryFontFamily.value;

    const getFontSize = (_target: SubtitleTarget) => primaryFontSize.value;

    const getFontColor = (_target: SubtitleTarget) => primaryFontColor.value;

    const getSubPos = (target: SubtitleTarget) =>
        target === "primary" ? primarySubPos.value : secondarySubPos.value;

    const activeValues = computed(() => ({
        primary: {
            fontFamily: primaryFontFamily.value,
            fontSize: primaryFontSize.value,
            fontColor: primaryFontColor.value,
            subPos: primarySubPos.value,
        },
        secondary: {
            fontFamily: secondaryFontFamily.value,
            fontSize: secondaryFontSize.value,
            fontColor: secondaryFontColor.value,
            subPos: secondarySubPos.value,
        },
    }));

    const buildPersistedSubtitleAppearanceState = () => ({
        subtitleAppearance: {
            fontFamily: primaryFontFamily.value,
            fontSize: primaryFontSize.value,
            fontColor: primaryFontColor.value,
            primarySubPos: primarySubPos.value,
            secondarySubPos: secondarySubPos.value,
        } satisfies PersistedSubtitleAppearanceState,
    });

    const persistSubtitleAppearanceNow = async () => {
        await saveUiState(buildPersistedSubtitleAppearanceState());
    };

    const applySubtitleAppearanceOptions = async () => {
        await Promise.all([
            invoke("mpv_set_option_string", {
                name: "sub-font",
                value: primaryFontFamily.value,
            }),
            invoke("mpv_set_option_string", {
                name: "sub-font-size",
                value: primaryFontSize.value,
            }),
            invoke("mpv_set_option_string", {
                name: "sub-color",
                value: primaryFontColor.value,
            }),
            invoke("mpv_set_option_string", {
                name: "sub-pos",
                value: primarySubPos.value,
            }),
            invoke("mpv_set_option_string", {
                name: "secondary-sub-pos",
                value: secondarySubPos.value,
            }),
        ]);
    };

    const setSubtitleFontFamily = async (
        payload: TargetValuePayload<string>,
    ) => {
        const fontFamily = payload.value.trim();
        primaryFontFamily.value = fontFamily;
        secondaryFontFamily.value = fontFamily;
        await invoke("mpv_set_option_string", {
            name: "sub-font",
            value: fontFamily,
        });
        await persistSubtitleAppearanceNow();
    };

    const setSubtitleFontSize = async (
        payload: TargetValuePayload<number>,
    ) => {
        const next = clamp(payload.value, 8, 200);
        primaryFontSize.value = next;
        secondaryFontSize.value = next;
        await invoke("mpv_set_option_string", {
            name: "sub-font-size",
            value: next,
        });
        await persistSubtitleAppearanceNow();
    };

    const setSubtitleFontColor = async (
        payload: TargetValuePayload<string>,
    ) => {
        const fontColor = payload.value.trim() || defaultFontColor;
        primaryFontColor.value = fontColor;
        secondaryFontColor.value = fontColor;
        await invoke("mpv_set_option_string", {
            name: "sub-color",
            value: fontColor,
        });
        await persistSubtitleAppearanceNow();
    };

    const setSubtitlePosition = async (payload: TargetValuePayload<number>) => {
        const next = clamp(payload.value, 0, 100);
        if (payload.target === "primary") {
            primarySubPos.value = next;
        } else {
            secondarySubPos.value = next;
        }
        await invoke("mpv_set_option_string", {
            name: positionOptionForTarget(payload.target),
            value: next,
        });
        await persistSubtitleAppearanceNow();
    };

    const resetSubtitleAppearance = async (target?: SubtitleTarget) => {
        primaryFontFamily.value = "";
        secondaryFontFamily.value = "";
        primaryFontSize.value = defaultFontSize;
        secondaryFontSize.value = defaultFontSize;
        primaryFontColor.value = defaultFontColor;
        secondaryFontColor.value = defaultFontColor;
        await invoke("mpv_set_option_string", {
            name: "sub-font",
            value: "",
        });
        await invoke("mpv_set_option_string", {
            name: "sub-font-size",
            value: defaultFontSize,
        });
        await invoke("mpv_set_option_string", {
            name: "sub-color",
            value: defaultFontColor,
        });

        const targets: SubtitleTarget[] = target ? [target] : ["primary", "secondary"];
        for (const current of targets) {
            if (current === "primary") {
                primarySubPos.value = defaultPrimarySubPos;
            } else {
                secondarySubPos.value = defaultSecondarySubPos;
            }
            await invoke("mpv_set_option_string", {
                name: positionOptionForTarget(current),
                value:
                    current === "primary"
                        ? defaultPrimarySubPos
                        : defaultSecondarySubPos,
            });
        }
        await persistSubtitleAppearanceNow();
    };

    void (async () => {
        const stored = await loadUiState<{
            subtitleAppearance?: PersistedSubtitleAppearanceState;
        }>();
        const persisted = stored?.subtitleAppearance;
        if (!persisted) return;

        const fontFamily = normalizeFontFamily(persisted.fontFamily);
        const fontSize = normalizeFontSize(persisted.fontSize);
        const fontColor = normalizeFontColor(persisted.fontColor);
        primaryFontFamily.value = fontFamily;
        secondaryFontFamily.value = fontFamily;
        primaryFontSize.value = fontSize;
        secondaryFontSize.value = fontSize;
        primaryFontColor.value = fontColor;
        secondaryFontColor.value = fontColor;
        primarySubPos.value = normalizeSubPos(
            persisted.primarySubPos,
            defaultPrimarySubPos,
        );
        secondarySubPos.value = normalizeSubPos(
            persisted.secondarySubPos,
            defaultSecondarySubPos,
        );
        try {
            await applySubtitleAppearanceOptions();
        } catch {
            // mpv may not be ready during startup; playback load reapplies these.
        }
    })();

    return {
        primaryFontFamily,
        secondaryFontFamily,
        primaryFontSize,
        secondaryFontSize,
        primaryFontColor,
        secondaryFontColor,
        primarySubPos,
        secondarySubPos,
        activeValues,
        getFontFamily,
        getFontSize,
        getFontColor,
        getSubPos,
        setSubtitleFontFamily,
        setSubtitleFontSize,
        setSubtitleFontColor,
        setSubtitlePosition,
        resetSubtitleAppearance,
        applySubtitleAppearanceOptions,
    };
};
