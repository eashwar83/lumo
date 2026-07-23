import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// Ultra Slo-Mo: hold a key to smoothly ramp playback down to a chosen fraction
// of normal speed with mpv frame interpolation enabled, so slow motion stays
// fluid instead of stuttering (an iPhone-style "hold for slo-mo"). Release to
// ramp back. Everything mpv-side (interpolation, video-sync, tscale, mute) is
// saved on entry and restored on exit, so normal playback is never altered.
//
// Honest limitation: interpolation blends frames, so fast motion looks soft
// rather than razor-sharp — the realtime tradeoff (no AI optical flow).

export type SlomoFactor = 0.5 | 0.25 | 0.125;

export const SLOMO_FACTORS: { value: SlomoFactor; label: string }[] = [
    { value: 0.5, label: "½×" },
    { value: 0.25, label: "¼×" },
    { value: 0.125, label: "⅛×" },
];

const RAMP_MS = 260;
const RAMP_STEPS = 12;

type UltraSlomoOptions = {
    isFileLoaded: () => boolean;
};

const setOption = async (name: string, value: string) => {
    try {
        await invoke("mpv_set_option_string", { name, value });
    } catch (error) {
        console.warn("[slomo] set", name, "failed", error);
    }
};

const getProp = async (name: string): Promise<string | null> => {
    try {
        return await invoke<string | null>("mpv_get_property_string", { name });
    } catch {
        return null;
    }
};

export const useUltraSlomo = (options: UltraSlomoOptions) => {
    const factor = ref<SlomoFactor>(0.125);
    const isActive = ref(false);

    // mpv state captured on entry so we can put everything back on exit.
    let savedMute: string | null = null;
    let savedVideoSync: string | null = null;
    let savedInterpolation: string | null = null;
    let savedTscale: string | null = null;
    let baseSpeed = 1; // speed to ramp from / return to (the user's chosen rate)
    let liveSpeed = 1; // current (possibly mid-ramp) speed, so ramps chain smoothly

    let rampTimer: number | null = null;

    const clearRamp = () => {
        if (rampTimer !== null) {
            window.clearInterval(rampTimer);
            rampTimer = null;
        }
    };

    // Ease speed from the current live speed to `to` over RAMP_MS so it glides
    // in/out of slo-mo even if start/stop/factor-change overlap mid-ramp.
    const rampSpeed = (to: number): Promise<void> =>
        new Promise((resolve) => {
            clearRamp();
            const from = liveSpeed;
            let step = 0;
            rampTimer = window.setInterval(() => {
                step += 1;
                const t = Math.min(1, step / RAMP_STEPS);
                // easeInOutQuad
                const eased =
                    t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
                liveSpeed = from + (to - from) * eased;
                void setOption("speed", liveSpeed.toFixed(4));
                if (step >= RAMP_STEPS) {
                    clearRamp();
                    resolve();
                }
            }, RAMP_MS / RAMP_STEPS);
        });

    const setFactor = (value: SlomoFactor) => {
        factor.value = value;
        // Live-adjust the depth if we're already holding.
        if (isActive.value) void rampSpeed(value);
    };

    const label = computed(
        () =>
            `${SLOMO_FACTORS.find((f) => f.value === factor.value)?.label ?? ""} SLO-MO`,
    );

    const start = async () => {
        if (isActive.value) return;
        if (!options.isFileLoaded()) return;
        isActive.value = true;

        // Capture current mpv state.
        savedMute = await getProp("mute");
        savedVideoSync = await getProp("video-sync");
        savedInterpolation = await getProp("interpolation");
        savedTscale = await getProp("tscale");
        const speedNow = Number.parseFloat((await getProp("speed")) ?? "1");
        baseSpeed = Number.isFinite(speedNow) && speedNow > 0 ? speedNow : 1;
        liveSpeed = baseSpeed;

        // Enable smooth motion-interpolated slow motion + mute.
        await setOption("video-sync", "display-resample");
        await setOption("tscale", "mitchell");
        await setOption("interpolation", "yes");
        await setOption("mute", "yes");

        await rampSpeed(factor.value);
    };

    const stop = async () => {
        if (!isActive.value) return;
        isActive.value = false;

        await rampSpeed(baseSpeed);
        await setOption("speed", baseSpeed.toFixed(4));

        // Restore the saved mpv state.
        await setOption("interpolation", savedInterpolation ?? "no");
        await setOption("video-sync", savedVideoSync ?? "audio");
        if (savedTscale) await setOption("tscale", savedTscale);
        await setOption("mute", savedMute ?? "no");
    };

    // Safety net: if focus leaves the window mid-hold the keyup may never fire,
    // which would strand playback in slo-mo. Release on blur.
    const onWindowBlur = () => {
        if (isActive.value) void stop();
    };

    onMounted(() => {
        window.addEventListener("blur", onWindowBlur);
    });

    onBeforeUnmount(() => {
        window.removeEventListener("blur", onWindowBlur);
        clearRamp();
    });

    return {
        factor,
        isActive,
        label,
        setFactor,
        start,
        stop,
    };
};

export type UltraSlomoController = ReturnType<typeof useUltraSlomo>;
