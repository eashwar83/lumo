import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SubtitleTarget } from "./useSubtitleState";

type TargetValuePayload<T> = {
    target: SubtitleTarget;
    value: T;
};

const clamp = (value: number, min: number, max: number) =>
    Math.min(max, Math.max(min, value));

const optionForTarget = (target: SubtitleTarget, base: string) =>
    target === "primary" ? base : `secondary-${base}`;

const defaultFontSize = 55;
const defaultSubPos = 100;

export const useSubtitleAppearance = () => {
    const primaryFontFamily = ref("");
    const secondaryFontFamily = ref("");
    const primaryFontSize = ref(defaultFontSize);
    const secondaryFontSize = ref(defaultFontSize);
    const primarySubPos = ref(defaultSubPos);
    const secondarySubPos = ref(defaultSubPos);

    const getFontFamily = (target: SubtitleTarget) =>
        target === "primary" ? primaryFontFamily.value : secondaryFontFamily.value;

    const getFontSize = (target: SubtitleTarget) =>
        target === "primary" ? primaryFontSize.value : secondaryFontSize.value;

    const getSubPos = (target: SubtitleTarget) =>
        target === "primary" ? primarySubPos.value : secondarySubPos.value;

    const activeValues = computed(() => ({
        primary: {
            fontFamily: primaryFontFamily.value,
            fontSize: primaryFontSize.value,
            subPos: primarySubPos.value,
        },
        secondary: {
            fontFamily: secondaryFontFamily.value,
            fontSize: secondaryFontSize.value,
            subPos: secondarySubPos.value,
        },
    }));

    const setSubtitleFontFamily = async (
        payload: TargetValuePayload<string>,
    ) => {
        const fontFamily = payload.value.trim();
        if (payload.target === "primary") {
            primaryFontFamily.value = fontFamily;
        } else {
            secondaryFontFamily.value = fontFamily;
        }
        await invoke("mpv_set_option_string", {
            name: optionForTarget(payload.target, "sub-font"),
            value: fontFamily,
        });
    };

    const setSubtitleFontSize = async (
        payload: TargetValuePayload<number>,
    ) => {
        const next = clamp(payload.value, 8, 200);
        if (payload.target === "primary") {
            primaryFontSize.value = next;
        } else {
            secondaryFontSize.value = next;
        }
        await invoke("mpv_set_option_string", {
            name: optionForTarget(payload.target, "sub-font-size"),
            value: next,
        });
    };

    const setSubtitlePosition = async (payload: TargetValuePayload<number>) => {
        const next = clamp(payload.value, 0, 100);
        if (payload.target === "primary") {
            primarySubPos.value = next;
        } else {
            secondarySubPos.value = next;
        }
        await invoke("mpv_set_option_string", {
            name: optionForTarget(payload.target, "sub-pos"),
            value: next,
        });
    };

    const resetSubtitleAppearance = async (target?: SubtitleTarget) => {
        const targets: SubtitleTarget[] = target ? [target] : ["primary", "secondary"];
        for (const current of targets) {
            if (current === "primary") {
                primaryFontFamily.value = "";
                primaryFontSize.value = defaultFontSize;
                primarySubPos.value = defaultSubPos;
            } else {
                secondaryFontFamily.value = "";
                secondaryFontSize.value = defaultFontSize;
                secondarySubPos.value = defaultSubPos;
            }
            await invoke("mpv_set_option_string", {
                name: optionForTarget(current, "sub-font"),
                value: "",
            });
            await invoke("mpv_set_option_string", {
                name: optionForTarget(current, "sub-font-size"),
                value: defaultFontSize,
            });
            await invoke("mpv_set_option_string", {
                name: optionForTarget(current, "sub-pos"),
                value: defaultSubPos,
            });
        }
    };

    return {
        primaryFontFamily,
        secondaryFontFamily,
        primaryFontSize,
        secondaryFontSize,
        primarySubPos,
        secondarySubPos,
        activeValues,
        getFontFamily,
        getFontSize,
        getSubPos,
        setSubtitleFontFamily,
        setSubtitleFontSize,
        setSubtitlePosition,
        resetSubtitleAppearance,
    };
};

