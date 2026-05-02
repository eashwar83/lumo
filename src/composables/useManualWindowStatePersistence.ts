import { onBeforeUnmount, type Ref } from "vue";
import {
    getCurrentWindow,
    LogicalSize,
    PhysicalPosition,
} from "@tauri-apps/api/window";
import { loadUiState, saveUiState } from "./useUiStateStore";

type ManualWindowState = {
    width?: number;
    height?: number;
    x?: number;
    y?: number;
    isMaximized?: boolean;
};

type StoredWindowUiState = {
    windowState?: ManualWindowState;
};

type ManualWindowStatePersistenceOptions = {
    isLoading: Ref<boolean>;
    isPlaybackActive: Ref<boolean>;
    isFileLoaded: () => boolean;
};

const normalizeDimension = (value: unknown): number | undefined => {
    if (typeof value !== "number" || !Number.isFinite(value)) return undefined;
    const rounded = Math.floor(value);
    return rounded > 0 ? rounded : undefined;
};

const normalizePersistedWindowState = (
    value: unknown,
): ManualWindowState | null => {
    if (!value || typeof value !== "object") return null;
    const candidate = value as ManualWindowState;
    const width = normalizeDimension(candidate.width);
    const height = normalizeDimension(candidate.height);
    const x =
        typeof candidate.x === "number" && Number.isFinite(candidate.x)
            ? Math.round(candidate.x)
            : undefined;
    const y =
        typeof candidate.y === "number" && Number.isFinite(candidate.y)
            ? Math.round(candidate.y)
            : undefined;
    const isMaximized =
        typeof candidate.isMaximized === "boolean"
            ? candidate.isMaximized
            : undefined;
    if (
        width === undefined &&
        height === undefined &&
        x === undefined &&
        y === undefined &&
        isMaximized === undefined
    ) {
        return null;
    }
    return {
        width,
        height,
        x,
        y,
        isMaximized,
    };
};

export const useManualWindowStatePersistence = ({
    isLoading,
    isPlaybackActive,
    isFileLoaded,
}: ManualWindowStatePersistenceOptions) => {
    let persistManualWindowTimer: number | null = null;

    const shouldPersistManualWindow = async () => {
        if (isLoading.value) return false;
        if (isPlaybackActive.value) return false;
        if (isFileLoaded()) return false;
        return !(await getCurrentWindow().isFullscreen().catch(() => false));
    };

    const persistCurrentManualWindow = async () => {
        const currentWindow = getCurrentWindow();
        if (!(await shouldPersistManualWindow())) return;

        const isFullscreen = await currentWindow.isFullscreen().catch(() => false);
        if (isFullscreen) return;

        const scale = await currentWindow.scaleFactor().catch(() => 1);
        const isMaximized = await currentWindow.isMaximized().catch(() => false);
        const nextState: ManualWindowState = {
            isMaximized,
        };

        if (!isMaximized) {
            const [innerSize, outerPosition] = await Promise.all([
                currentWindow.innerSize().catch(() => null),
                currentWindow.outerPosition().catch(() => null),
            ]);
            nextState.width = normalizeDimension(
                innerSize ? innerSize.width / scale : undefined,
            );
            nextState.height = normalizeDimension(
                innerSize ? innerSize.height / scale : undefined,
            );
            nextState.x =
                typeof outerPosition?.x === "number"
                    ? Math.round(outerPosition.x)
                    : undefined;
            nextState.y =
                typeof outerPosition?.y === "number"
                    ? Math.round(outerPosition.y)
                    : undefined;
        }

        await saveUiState({
            windowState: nextState,
        });
    };

    const restorePersistedManualWindow = async () => {
        const currentWindow = getCurrentWindow();
        const stored = await loadUiState<StoredWindowUiState>();
        const persisted = normalizePersistedWindowState(stored?.windowState);
        if (!persisted) return;

        const isFullscreen = await currentWindow.isFullscreen().catch(() => false);
        if (isFullscreen) return;

        const isMaximized = await currentWindow.isMaximized().catch(() => false);
        if (isMaximized && !persisted.isMaximized) {
            await currentWindow.unmaximize().catch(() => {});
        }

        if (persisted.width !== undefined && persisted.height !== undefined) {
            await currentWindow.setSize(
                new LogicalSize(persisted.width, persisted.height),
            );
        }

        if (persisted.x !== undefined && persisted.y !== undefined) {
            await currentWindow.setPosition(
                new PhysicalPosition(persisted.x, persisted.y),
            );
        }

        if (persisted.isMaximized) {
            await currentWindow.maximize().catch(() => {});
        }
    };

    const clearPersistManualWindowTimer = () => {
        if (persistManualWindowTimer === null) return;
        window.clearTimeout(persistManualWindowTimer);
        persistManualWindowTimer = null;
    };

    const schedulePersistManualWindow = () => {
        clearPersistManualWindowTimer();
        persistManualWindowTimer = window.setTimeout(() => {
            persistManualWindowTimer = null;
            void persistCurrentManualWindow();
        }, 220);
    };

    const persistBeforeUnload = () => {
        clearPersistManualWindowTimer();
        void persistCurrentManualWindow();
    };

    onBeforeUnmount(() => {
        clearPersistManualWindowTimer();
    });

    return {
        persistCurrentManualWindow,
        restorePersistedManualWindow,
        schedulePersistManualWindow,
        clearPersistManualWindowTimer,
        persistBeforeUnload,
    };
};
