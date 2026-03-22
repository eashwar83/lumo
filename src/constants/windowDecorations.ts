import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { ENABLE_COMPACT_MODE_SETTING_LABEL } from "../mock/settings";

type StoredSettingItemLike = { label?: string; value?: string };
type StoredSettingGroupLike = { title?: string; items?: StoredSettingItemLike[] };
const WINDOW_DECORATIONS_ENABLED = true;
const WINDOW_DECORATIONS_DISABLED = false;
const COMPACT_MODE_CORNER_RADIUS = 0;

const isCompactModeEnabled = (
    groups?: StoredSettingGroupLike[],
): boolean =>
    groups
        ?.flatMap((group) => group.items ?? [])
        .find((item) => item.label === ENABLE_COMPACT_MODE_SETTING_LABEL)?.value ===
    "On";

export const applyWindowDecorationsFromSettingGroups = async (
    groups?: StoredSettingGroupLike[],
) => {
    if (typeof window === "undefined") return;
    const compactModeEnabled = isCompactModeEnabled(groups);
    const isLinuxPlatform =
        typeof navigator !== "undefined" && /\blinux\b/i.test(navigator.userAgent);
    const isDesktopCustomChromePlatform =
        typeof navigator !== "undefined" &&
        /\b(windows|linux)\b/i.test(navigator.userAgent);
    const shouldEnableDecorations =
        isDesktopCustomChromePlatform && compactModeEnabled
            ? WINDOW_DECORATIONS_DISABLED
            : WINDOW_DECORATIONS_ENABLED;
    const shouldForceLinuxDecorationRestore =
        isLinuxPlatform && !compactModeEnabled;

    const currentWindow = getCurrentWindow();
    let alreadyEnabled: boolean | null = null;

    try {
        alreadyEnabled = await currentWindow.isDecorated();
    } catch (error) {
        console.warn("[windowDecorations] Failed to query decoration state", {
            compactModeEnabled,
            shouldEnableDecorations,
            isLinuxPlatform,
            error,
        });
    }

    if (shouldForceLinuxDecorationRestore) {
        try {
            await currentWindow.setDecorations(WINDOW_DECORATIONS_ENABLED);
        } catch (error) {
            console.warn(
                "[windowDecorations] Linux non-compact: forced setDecorations(true) failed",
                {
                    compactModeEnabled,
                    alreadyEnabled,
                    shouldEnableDecorations,
                    error,
                },
            );
        }
    } else if (alreadyEnabled !== shouldEnableDecorations) {
        try {
            await currentWindow.setDecorations(shouldEnableDecorations);
        } catch (error) {
            console.warn("[windowDecorations] Failed to toggle window decorations", {
                compactModeEnabled,
                alreadyEnabled,
                shouldEnableDecorations,
                isLinuxPlatform,
                error,
            });
        }
    }

    try {
        await invoke("apply_window_appearance", {
            compactMode: compactModeEnabled,
            cornerRadius: COMPACT_MODE_CORNER_RADIUS,
        });
    } catch (error) {
        console.warn("[windowDecorations] Failed to apply window appearance", {
            compactModeEnabled,
            isLinuxPlatform,
            error,
        });
    }
};
