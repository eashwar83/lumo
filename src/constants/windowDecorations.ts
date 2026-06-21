import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { ENABLE_COMPACT_MODE_SETTING_LABEL } from "../mock/settings";
import { extractAppThemeFromSettingGroups } from "./theme";

type StoredSettingItemLike = { label?: string; value?: string };
type StoredSettingGroupLike = { title?: string; items?: StoredSettingItemLike[] };
const WINDOW_DECORATIONS_ENABLED = true;
const WINDOW_DECORATIONS_DISABLED = false;
const COMPACT_MODE_CORNER_RADIUS = 0;
const DEFAULT_COMPACT_MODE_ENABLED = true;

const isCompactModeEnabled = (groups?: StoredSettingGroupLike[]): boolean => {
    const value = groups
        ?.flatMap((group) => group.items ?? [])
        .find((item) => item.label === ENABLE_COMPACT_MODE_SETTING_LABEL)
        ?.value?.trim();
    if (value === "On") return true;
    if (value === "Off") return false;
    return DEFAULT_COMPACT_MODE_ENABLED;
};

export const applyWindowDecorationsFromSettingGroups = async (
    groups?: StoredSettingGroupLike[],
) => {
    if (typeof window === "undefined") return;
    const compactModeEnabled = isCompactModeEnabled(groups);
    const theme = extractAppThemeFromSettingGroups(groups);
    const isMacPlatform =
        typeof navigator !== "undefined" && /mac|darwin/i.test(navigator.userAgent);
    const isLinuxPlatform =
        typeof navigator !== "undefined" && /\blinux\b/i.test(navigator.userAgent);
    const shouldEnableDecorations = compactModeEnabled
        ? WINDOW_DECORATIONS_DISABLED
        : WINDOW_DECORATIONS_ENABLED;
    const effectiveShouldEnableDecorations = isLinuxPlatform
        ? WINDOW_DECORATIONS_DISABLED
        : shouldEnableDecorations;

    const currentWindow = getCurrentWindow();
    let alreadyEnabled: boolean | null = null;

    try {
        alreadyEnabled = await currentWindow.isDecorated();
    } catch (error) {
        console.warn("[windowDecorations] Failed to query decoration state", {
            compactModeEnabled,
            shouldEnableDecorations,
            error,
        });
    }

    if (!isMacPlatform && alreadyEnabled !== effectiveShouldEnableDecorations) {
        try {
            await currentWindow.setDecorations(effectiveShouldEnableDecorations);
        } catch (error) {
            console.warn("[windowDecorations] Failed to toggle window decorations", {
                compactModeEnabled,
                alreadyEnabled,
                shouldEnableDecorations,
                effectiveShouldEnableDecorations,
                isLinuxPlatform,
                isMacPlatform,
                error,
            });
        }
    }

    try {
        await invoke("apply_window_appearance", {
            compactMode: compactModeEnabled,
            cornerRadius: COMPACT_MODE_CORNER_RADIUS,
            theme,
        });
    } catch (error) {
        console.warn("[windowDecorations] Failed to apply window appearance", {
            compactModeEnabled,
            error,
        });
    }
};
