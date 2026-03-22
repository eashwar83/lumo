import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

const clamp = (value: number, min: number, max: number) =>
    Math.min(max, Math.max(min, value));

export const usePlaybackAdjustments = () => {
    const showSettingsMenu = ref(false);
    const audioDelay = ref(0);
    const subDelay = ref(0);
    const brightness = ref(0);
    const contrast = ref(0);
    const saturation = ref(0);
    const gamma = ref(0);
    const hue = ref(0);

    const setAudioDelay = async (value: number) => {
        const next = clamp(value, -5, 5);
        audioDelay.value = next;
        await invoke("mpv_set_option_string", {
            name: "audio-delay",
            value: next,
        });
    };

    const setSubDelay = async (value: number) => {
        const next = clamp(value, -10, 10);
        subDelay.value = next;
        await invoke("mpv_set_option_string", { name: "sub-delay", value: next });
    };

    const setBrightness = async (value: number) => {
        const next = clamp(value, -100, 100);
        brightness.value = next;
        await invoke("mpv_set_option_string", { name: "brightness", value: next });
    };

    const setContrast = async (value: number) => {
        const next = clamp(value, -100, 100);
        contrast.value = next;
        await invoke("mpv_set_option_string", { name: "contrast", value: next });
    };

    const setSaturation = async (value: number) => {
        const next = clamp(value, -100, 100);
        saturation.value = next;
        await invoke("mpv_set_option_string", {
            name: "saturation",
            value: next,
        });
    };

    const setGamma = async (value: number) => {
        const next = clamp(value, -100, 100);
        gamma.value = next;
        await invoke("mpv_set_option_string", { name: "gamma", value: next });
    };

    const setHue = async (value: number) => {
        const next = clamp(value, -100, 100);
        hue.value = next;
        await invoke("mpv_set_option_string", { name: "hue", value: next });
    };

    return {
        showSettingsMenu,
        audioDelay,
        subDelay,
        brightness,
        contrast,
        saturation,
        gamma,
        hue,
        setAudioDelay,
        setSubDelay,
        setBrightness,
        setContrast,
        setSaturation,
        setGamma,
        setHue,
    };
};
