import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export const usePlaybackSpeed = () => {
    const playbackRates = [2.0, 1.75, 1.5, 1.25, 1.0, 0.75, 0.5, 0.25];
    const currentSpeed = ref(1.0);
    const showSpeedMenu = ref(false);

    const setSpeed = async (rate: number) => {
        currentSpeed.value = rate;
        showSpeedMenu.value = false;
        await invoke("mpv_set_option_string", { name: "speed", value: rate });
    };

    return {
        playbackRates,
        currentSpeed,
        showSpeedMenu,
        setSpeed,
    };
};
