import { onMounted, onUnmounted } from "vue";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import type { UnlistenFn } from "@tauri-apps/api/event";
import {
    SETTINGS_UPDATED_EVENT,
    WINDOW_SIZE_LOCK_SETTING_LABEL,
} from "../mock/settings";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";

// "Locked" window size: once the user sizes the window, keep that exact size
// (and position — we never re-center) for every subsequent video, until they
// resize again. Persists across restarts via the `windowLockedSize` ui-state
// slice. The first video with no locked size falls through to the app's native
// auto-resize, and the resulting size is captured as the lock.

type LockedSize = { width: number; height: number };

type WindowSizeLockOptions = {
    // Called when a genuine user resize (drag/step) updates the locked size, so
    // the caller can clear any per-file "fit" flag the user just overrode.
    onUserResize?: () => void;
};

type StoredLock = { windowLockedSize?: { width?: number; height?: number } };
type StoredSettingGroup = {
    title: string;
    items: Array<{ label: string; value: string }>;
};

const parseLockEnabled = (groups?: StoredSettingGroup[]): boolean =>
    groups
        ?.flatMap((group) => group.items)
        .find((item) => item.label === WINDOW_SIZE_LOCK_SETTING_LABEL)?.value !==
    "Off";

export const useWindowSizeLock = (options: WindowSizeLockOptions = {}) => {
    let lockedSize: LockedSize | null = null;
    // Defaults on; an explicit "Off" in settings disables the whole feature.
    let enabled = true;
    const saver = createDebouncedUiStateSaver(300);

    // Depth counter so the onResized events our own setSize triggers are not
    // mistaken for user drags. Decremented after a delay to swallow the trailing
    // resize event.
    let programmaticDepth = 0;
    let captureTimer: number | null = null;
    let unlistenResized: UnlistenFn | null = null;

    const persist = () => {
        saver.saveDebounced({
            windowLockedSize: lockedSize ?? undefined,
        });
    };

    const runProgrammatic = async (fn: () => Promise<void>) => {
        programmaticDepth += 1;
        try {
            await fn();
        } catch (error) {
            console.warn("[windowLock] programmatic resize failed", error);
        } finally {
            window.setTimeout(() => {
                programmaticDepth = Math.max(0, programmaticDepth - 1);
            }, 320);
        }
    };

    const setLocked = (width: number, height: number) => {
        if (width > 0 && height > 0) {
            lockedSize = { width: Math.round(width), height: Math.round(height) };
            persist();
        }
    };

    const hasLocked = () => lockedSize !== null;

    // The locked logical size, but only while the feature is enabled — callers
    // use this as a settled height baseline for fitting. null => use live size.
    const getLockedSize = (): LockedSize | null => (enabled ? lockedSize : null);

    // Read the window's current logical inner size and store it as the lock.
    const captureCurrentAsLocked = async () => {
        const win = getCurrentWindow();
        if (await win.isFullscreen().catch(() => false)) return;
        if (await win.isMaximized().catch(() => false)) return;
        const scale = await win.scaleFactor().catch(() => 1);
        const inner = await win.innerSize().catch(() => null);
        if (!inner) return;
        setLocked(inner.width / scale, inner.height / scale);
        options.onUserResize?.();
    };

    const onWindowResized = () => {
        if (!enabled) return;
        if (programmaticDepth > 0) return;
        if (captureTimer) window.clearTimeout(captureTimer);
        captureTimer = window.setTimeout(() => {
            captureTimer = null;
            void captureCurrentAsLocked();
        }, 260);
    };

    // Apply the locked size to the window without centering. Returns true when a
    // lock exists (so the caller skips the native auto-resize); false otherwise.
    const applyLocked = async (): Promise<boolean> => {
        if (!enabled) return false;
        if (!lockedSize) return false;
        const win = getCurrentWindow();
        if (await win.isFullscreen().catch(() => false)) return true;
        if (await win.isMaximized().catch(() => false)) return true;
        await runProgrammatic(async () => {
            await win.setSize(new LogicalSize(lockedSize!.width, lockedSize!.height));
        });
        return true;
    };

    const onSettingsUpdated = (event: Event) => {
        const detail = (event as CustomEvent<{ groups?: StoredSettingGroup[] }>)
            .detail;
        enabled = parseLockEnabled(detail?.groups);
    };

    onMounted(async () => {
        const stored = await loadUiState<
            StoredLock & { settings?: { groups?: StoredSettingGroup[] } }
        >();
        enabled = parseLockEnabled(stored?.settings?.groups);
        const size = stored?.windowLockedSize;
        if (
            size &&
            typeof size.width === "number" &&
            typeof size.height === "number" &&
            size.width > 0 &&
            size.height > 0
        ) {
            lockedSize = {
                width: Math.round(size.width),
                height: Math.round(size.height),
            };
        }
        window.addEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
        try {
            unlistenResized = await getCurrentWindow().onResized(() => {
                onWindowResized();
            });
        } catch {
            // Non-Tauri environment.
        }
    });

    onUnmounted(() => {
        window.removeEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
        unlistenResized?.();
        unlistenResized = null;
        if (captureTimer) window.clearTimeout(captureTimer);
    });

    return {
        runProgrammatic,
        setLocked,
        hasLocked,
        getLockedSize,
        applyLocked,
    };
};

export type WindowSizeLockController = ReturnType<typeof useWindowSizeLock>;
