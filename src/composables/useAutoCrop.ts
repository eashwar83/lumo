import { onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, PhysicalSize } from "@tauri-apps/api/window";
import {
    AUTO_CROP_SETTING_LABEL,
    SETTINGS_UPDATED_EVENT,
} from "../mock/settings";
import { loadUiState } from "./useUiStateStore";

// Port of the mpv `autocrop.lua` script: insert a lavfi `cropdetect` filter,
// let it gather data for a moment, read the detected rectangle from
// `vf-metadata`, then apply it via the `video-crop` property. Runs entirely
// through the frontend mpv bridge (run command + get property).

type StoredSettingGroup = {
    title: string;
    items: Array<{ label: string; value: string }>;
};

type AutoCropOptions = {
    isFileLoaded: () => boolean;
    onMessage?: (text: string) => void;
};

const CROPDETECT_LABEL = "soia-cropdetect";
// Defaults mirror autocrop.lua.
const AUTO_DELAY_SECONDS = 4;
const DETECT_SECONDS = 1;
const DETECT_LIMIT = "24/255";
const DETECT_ROUND = 2;
const DETECT_MIN_RATIO = 0.5;

const parseAutoCropEnabled = (groups?: StoredSettingGroup[]): boolean =>
    groups
        ?.flatMap((group) => group.items)
        .find((item) => item.label === AUTO_CROP_SETTING_LABEL)?.value === "On";

const getProp = async (name: string): Promise<string | null> => {
    try {
        return await invoke<string | null>("mpv_get_property_string", { name });
    } catch {
        return null;
    }
};

const getNumberProp = async (name: string): Promise<number | null> => {
    const raw = await getProp(name);
    if (raw === null) return null;
    const parsed = Number.parseFloat(raw);
    return Number.isFinite(parsed) ? parsed : null;
};

const runCommand = async (args: Array<string | number>): Promise<boolean> => {
    try {
        await invoke("mpv_run_command", { args });
        return true;
    } catch (error) {
        console.warn("[autocrop] mpv command failed", { args, error });
        return false;
    }
};

export const useAutoCrop = (options: AutoCropOptions) => {
    const enabled = ref(false);

    let autoDelayTimer: number | null = null;
    let detectTimer: number | null = null;
    let hwdecBackup: string | null = null;
    let isDetecting = false;

    const clearTimers = () => {
        if (autoDelayTimer !== null) {
            window.clearTimeout(autoDelayTimer);
            autoDelayTimer = null;
        }
        if (detectTimer !== null) {
            window.clearTimeout(detectTimer);
            detectTimer = null;
        }
    };

    const removeCropdetect = async () => {
        // `vf remove` is a no-op (and errors) when the label isn't present;
        // that's fine — runCommand swallows the error.
        await runCommand(["vf", "remove", `@${CROPDETECT_LABEL}`]);
    };

    const restoreHwdec = async () => {
        if (hwdecBackup) {
            await runCommand(["set", "hwdec", hwdecBackup]);
            hwdecBackup = null;
        }
    };

    const clearCrop = async () => {
        // Set the property directly so it applies to the current frame
        // immediately (file-local-options only reliably apply at load time).
        await runCommand(["set", "video-crop", ""]);
    };

    // After cropping, reshape the window to the cropped picture's aspect ratio
    // so the video fills it with no letterbox. Skipped when fullscreen or
    // maximized (the screen is a fixed shape there, so bars are unavoidable).
    const fitWindowToAspect = async (cropW: number, cropH: number) => {
        if (cropW <= 0 || cropH <= 0) return;
        try {
            const win = getCurrentWindow();
            if ((await win.isFullscreen()) || (await win.isMaximized())) return;
            const size = await win.innerSize();
            if (!size.height) return;
            const targetWidth = Math.round(size.height * (cropW / cropH));
            if (targetWidth <= 0 || targetWidth === size.width) return;
            await win.setSize(new PhysicalSize(targetWidth, size.height));
        } catch (error) {
            console.warn("[autocrop] window fit failed", error);
        }
    };

    const cleanup = async () => {
        clearTimers();
        isDetecting = false;
        await removeCropdetect();
        await restoreHwdec();
    };

    const isCropable = async (secondsNeeded: number): Promise<boolean> => {
        if (!options.isFileLoaded()) return false;
        // cropdetect only makes sense for real video, not cover art / images.
        const isImage = await getProp("current-tracks/video/image");
        if (isImage === "yes") return false;
        const remaining = await getNumberProp("playtime-remaining");
        // Live streams report no remaining time; allow those through.
        if (remaining === null) return true;
        return secondsNeeded + 1 < remaining;
    };

    const applyCrop = async (meta: {
        w: number;
        h: number;
        x: number;
        y: number;
        maxW: number;
        maxH: number;
    }) => {
        const minW = meta.maxW * DETECT_MIN_RATIO;
        const minH = meta.maxH * DETECT_MIN_RATIO;

        const isEffective =
            meta.x > 0 || meta.y > 0 || meta.w < meta.maxW || meta.h < meta.maxH;
        const isExcessive =
            isEffective && (meta.w < minW || meta.h < minH);

        if (!isEffective) {
            await clearCrop();
            options.onMessage?.("Auto-crop: no black bars detected");
            return;
        }
        if (isExcessive) {
            await clearCrop();
            options.onMessage?.("Auto-crop: detected area too large, skipped");
            return;
        }

        const value = `${meta.w}x${meta.h}+${meta.x}+${meta.y}`;
        const applied = await runCommand(["set", "video-crop", value]);
        if (applied) {
            options.onMessage?.(`Auto-crop · ${value}`);
            await fitWindowToAspect(meta.w, meta.h);
        } else {
            options.onMessage?.(
                "Auto-crop failed: video-crop unsupported by this mpv",
            );
        }
    };

    const detectEnd = async () => {
        detectTimer = null;

        const w = await getNumberProp(
            `vf-metadata/${CROPDETECT_LABEL}/lavfi.cropdetect.w`,
        );
        const h = await getNumberProp(
            `vf-metadata/${CROPDETECT_LABEL}/lavfi.cropdetect.h`,
        );
        const x = await getNumberProp(
            `vf-metadata/${CROPDETECT_LABEL}/lavfi.cropdetect.x`,
        );
        const y = await getNumberProp(
            `vf-metadata/${CROPDETECT_LABEL}/lavfi.cropdetect.y`,
        );

        await removeCropdetect();
        await restoreHwdec();
        isDetecting = false;

        if (w === null || h === null || x === null || y === null) {
            options.onMessage?.(
                "Auto-crop: no crop data (cropdetect produced no metadata)",
            );
            return;
        }

        const width = await getNumberProp("width");
        const height = await getNumberProp("height");
        if (!width || !height) {
            options.onMessage?.("Auto-crop: could not read video size");
            return;
        }

        await applyCrop({ w, h, x, y, maxW: width, maxH: height });
    };

    const detectCrop = async () => {
        if (isDetecting) return;
        if (!(await isCropable(DETECT_SECONDS))) {
            options.onMessage?.("Auto-crop: not applicable to this media");
            return;
        }

        isDetecting = true;
        options.onMessage?.("Auto-crop: detecting…");

        // Non-copy hardware decoding breaks lavfi filters; temporarily disable
        // it for the duration of detection, then restore.
        const hwdecCurrent = await getProp("hwdec-current");
        if (
            hwdecCurrent &&
            hwdecCurrent !== "no" &&
            hwdecCurrent !== "crystalhd" &&
            hwdecCurrent !== "rkmpp" &&
            !hwdecCurrent.endsWith("-copy")
        ) {
            hwdecBackup = (await getProp("hwdec")) ?? "auto-safe";
            await runCommand(["set", "hwdec", "no"]);
        }

        const inserted = await runCommand([
            "vf",
            "pre",
            `@${CROPDETECT_LABEL}:cropdetect=limit=${DETECT_LIMIT}:round=${DETECT_ROUND}:reset=0`,
        ]);
        if (!inserted) {
            await restoreHwdec();
            isDetecting = false;
            options.onMessage?.("Auto-crop: failed to insert cropdetect filter");
            return;
        }

        detectTimer = window.setTimeout(() => {
            void detectEnd();
        }, DETECT_SECONDS * 1000);
    };

    const onFileLoaded = async () => {
        await cleanup();
        if (!enabled.value) return;
        if (!(await isCropable(AUTO_DELAY_SECONDS + DETECT_SECONDS))) return;

        autoDelayTimer = window.setTimeout(() => {
            autoDelayTimer = null;
            void detectCrop();
        }, AUTO_DELAY_SECONDS * 1000);
    };

    // Manual re-detect at the current frame (bypasses the auto delay). Lets the
    // user trigger detection on a clean, bright frame — the reason autocrop.lua
    // binds a key for this.
    const detectNow = async () => {
        clearTimers();
        await detectCrop();
    };

    const clear = async () => {
        await cleanup();
        await clearCrop();
        // Restore the window to the video's native aspect so clearing a crop
        // doesn't leave a mismatched (e.g. 4:3) window letterboxing the video.
        const width = await getNumberProp("width");
        const height = await getNumberProp("height");
        if (width && height) {
            await fitWindowToAspect(width, height);
        }
        options.onMessage?.("Crop cleared");
    };

    // Toggling the setting mid-playback should take effect immediately.
    watch(enabled, async (isEnabled, wasEnabled) => {
        if (isEnabled === wasEnabled) return;
        if (isEnabled) {
            if (options.isFileLoaded()) {
                await detectCrop();
            }
        } else {
            await clear();
        }
    });

    const applyStoredGroups = (groups?: StoredSettingGroup[]) => {
        enabled.value = parseAutoCropEnabled(groups);
    };

    const refreshFromSettings = async () => {
        const stored = await loadUiState<{
            settings?: { groups?: StoredSettingGroup[] };
        }>();
        applyStoredGroups(stored?.settings?.groups);
    };

    const onSettingsUpdated = (event: Event) => {
        const customEvent = event as CustomEvent<{
            groups?: StoredSettingGroup[];
        }>;
        applyStoredGroups(customEvent.detail?.groups);
    };

    onMounted(() => {
        void refreshFromSettings();
        window.addEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    onUnmounted(() => {
        window.removeEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
        clearTimers();
    });

    return {
        enabled,
        onFileLoaded,
        detectNow,
        clear,
    };
};
