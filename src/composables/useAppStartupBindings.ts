import { onBeforeUnmount, onMounted, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { applyThemeFromSettingGroups } from "../constants/theme";
import type { UpdateNotePrompt } from "../utils/parseUpdateNoteContent";
import { filterDroppedMediaPaths } from "./useMediaDropOpen";

type AppStartupBindingsOptions<PanelId extends string> = {
    activePanel: Ref<PanelId>;
    hideHistory: Ref<boolean>;
    clearNavSelectionDuringLoad: Ref<boolean>;
    settingsPanelId: PanelId;
    isFileLoaded: () => boolean;
    openSelectedPaths: (paths: string[]) => Promise<void>;
    loadActivePanel: () => Promise<void>;
    restorePersistedManualWindow: () => Promise<void>;
    schedulePersistManualWindow: () => void;
    persistBeforeUnload: () => void;
    showUpdateNotePrompt: (prompt: UpdateNotePrompt | null) => void;
};

export const useAppStartupBindings = <PanelId extends string>({
    activePanel,
    hideHistory,
    clearNavSelectionDuringLoad,
    settingsPanelId,
    isFileLoaded,
    openSelectedPaths,
    loadActivePanel,
    restorePersistedManualWindow,
    schedulePersistManualWindow,
    persistBeforeUnload,
    showUpdateNotePrompt,
}: AppStartupBindingsOptions<PanelId>) => {
    let unlistenAppOpenFiles: UnlistenFn | null = null;
    let unlistenWindowDragDrop: UnlistenFn | null = null;
    let unlistenOpenSettingsPanel: UnlistenFn | null = null;
    let unlistenUpdateNotePrompt: UnlistenFn | null = null;
    let unlistenWindowMoved: UnlistenFn | null = null;
    let unlistenWindowResizedForPersistence: UnlistenFn | null = null;
    let drainPendingOpenFilesRef: (() => Promise<void>) | null = null;

    const drainPendingOpenFiles = async () => {
        const pending = await invoke<string[]>("consume_pending_open_files");
        if (Array.isArray(pending) && pending.length) {
            await openSelectedPaths(pending);
        }
    };

    function onWindowFocusDrainPendingFiles() {
        if (!drainPendingOpenFilesRef) return;
        void (async () => {
            try {
                await drainPendingOpenFilesRef?.();
            } catch {
                // Ignore focus-triggered drain failures.
            }
        })();
    }

    onMounted(() => {
        applyThemeFromSettingGroups();
        void loadActivePanel();
        void (async () => {
            const currentWindow = getCurrentWindow();
            await restorePersistedManualWindow().catch(() => {});

            try {
                unlistenWindowMoved = await currentWindow.onMoved(() => {
                    schedulePersistManualWindow();
                });
            } catch {
                // Ignore window move binding failures in unsupported environments.
            }

            try {
                unlistenWindowResizedForPersistence = await currentWindow.onResized(
                    () => {
                        schedulePersistManualWindow();
                    },
                );
            } catch {
                // Ignore window resize binding failures in unsupported environments.
            }

            drainPendingOpenFilesRef = drainPendingOpenFiles;

            try {
                await drainPendingOpenFiles();
            } catch {
                // Ignore if there are no startup-open files.
            }

            try {
                unlistenAppOpenFiles = await listen("app-open-files", () => {
                    void (async () => {
                        try {
                            await drainPendingOpenFiles();
                        } catch {
                            // Ignore transient queue-drain errors.
                        }
                    })();
                });
            } catch {
                // Ignore event-listener failures in unsupported environments.
            }

            try {
                unlistenOpenSettingsPanel = await listen(
                    "soia-open-settings-panel",
                    () => {
                        if (isFileLoaded()) return;
                        clearNavSelectionDuringLoad.value = false;
                        activePanel.value = settingsPanelId;
                        hideHistory.value = false;
                    },
                );
            } catch {
                // Ignore event-listener failures in unsupported environments.
            }

            try {
                unlistenUpdateNotePrompt = await listen<UpdateNotePrompt>(
                    "soia-update-note-prompt",
                    (event) => {
                        void invoke("consume_pending_update_note_prompt").catch(
                            () => {},
                        );
                        showUpdateNotePrompt(event.payload);
                    },
                );
                const pendingPrompt = await invoke<UpdateNotePrompt | null>(
                    "consume_pending_update_note_prompt",
                );
                showUpdateNotePrompt(pendingPrompt);
            } catch {
                // Ignore event-listener failures in unsupported environments.
            }

            try {
                unlistenWindowDragDrop = await getCurrentWindow().onDragDropEvent(
                    ({ payload }) => {
                        if (payload.type !== "drop") return;
                        const droppedPaths = filterDroppedMediaPaths(payload.paths);
                        if (!droppedPaths.length) return;
                        void openSelectedPaths(droppedPaths);
                    },
                );
            } catch {
                // Ignore drag-drop binding failures in unsupported environments.
            }
        })();

        window.addEventListener("focus", onWindowFocusDrainPendingFiles);
        window.addEventListener("beforeunload", persistBeforeUnload);
    });

    onBeforeUnmount(() => {
        unlistenAppOpenFiles?.();
        unlistenAppOpenFiles = null;
        unlistenWindowDragDrop?.();
        unlistenWindowDragDrop = null;
        unlistenOpenSettingsPanel?.();
        unlistenOpenSettingsPanel = null;
        unlistenUpdateNotePrompt?.();
        unlistenUpdateNotePrompt = null;
        unlistenWindowMoved?.();
        unlistenWindowMoved = null;
        unlistenWindowResizedForPersistence?.();
        unlistenWindowResizedForPersistence = null;
        drainPendingOpenFilesRef = null;
        window.removeEventListener("focus", onWindowFocusDrainPendingFiles);
        window.removeEventListener("beforeunload", persistBeforeUnload);
    });
};
