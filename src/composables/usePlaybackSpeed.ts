import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

export const usePlaybackSpeed = () => {
    const playbackRates = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
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
