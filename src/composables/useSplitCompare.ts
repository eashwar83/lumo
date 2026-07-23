import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// Before/after wipe. The left of the line shows the untouched frame, the right
// shows everything the GPU enhancement chain does to it.
//
// The position is baked into the shader, so moving the line means rewriting and
// reloading it — libplacebo here rejects `//!PARAM`, which would otherwise have
// allowed a free runtime uniform. Drag updates are therefore throttled, with a
// final write on release so the line always ends up exactly where it was left.

export const useSplitCompare = () => {
    const isEnabled = ref(false);
    /** 0..1 across the video. */
    const position = ref(0.5);

    const applyShaders = async () => {
        try {
            await invoke("apply_split_compare", {
                enabled: isEnabled.value,
                position: position.value,
            });
        } catch (error) {
            console.warn("[compare] apply failed", error);
        }
    };

    const pushPosition = async () => {
        if (!isEnabled.value) return;
        try {
            await invoke("set_split_compare_position", {
                position: position.value,
            });
        } catch (error) {
            console.warn("[compare] position failed", error);
        }
    };

    // Each move recompiles a shader, so cap the rate and always send the last
    // value — dropping the final update would leave the line off from the handle.
    const THROTTLE_MS = 70;
    let lastSent = 0;
    let trailing: number | null = null;

    const flushTrailing = () => {
        if (trailing === null) return;
        window.clearTimeout(trailing);
        trailing = null;
    };

    const setPosition = (value: number) => {
        position.value = Math.min(1, Math.max(0, value));
        const now = Date.now();
        if (now - lastSent >= THROTTLE_MS) {
            lastSent = now;
            flushTrailing();
            void pushPosition();
            return;
        }
        flushTrailing();
        trailing = window.setTimeout(() => {
            trailing = null;
            lastSent = Date.now();
            void pushPosition();
        }, THROTTLE_MS);
    };

    /** Call when a drag ends so the final position is definitely applied. */
    const commitPosition = () => {
        if (!isEnabled.value) return;
        flushTrailing();
        lastSent = Date.now();
        void pushPosition();
    };

    const setEnabled = async (enabled: boolean) => {
        isEnabled.value = enabled;
        await applyShaders();
    };

    const toggle = async () => {
        await setEnabled(!isEnabled.value);
    };

    /** mpv drops user shaders on some file loads; re-assert if we're on. */
    const onFileLoaded = async () => {
        if (!isEnabled.value) return;
        await applyShaders();
    };

    return {
        isEnabled,
        position,
        setPosition,
        commitPosition,
        setEnabled,
        toggle,
        onFileLoaded,
    };
};

export type SplitCompareController = ReturnType<typeof useSplitCompare>;
