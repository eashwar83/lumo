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

// Windows parks minimized windows at -32000,-32000 with a degenerate size;
// geometry captured in that state must never be saved or restored (it makes
// the window invisible on next launch).
const MIN_SANE_DIMENSION = 200;
const MIN_SANE_COORDINATE = -20000;

const isSaneCoordinate = (value: number | undefined): value is number =>
    typeof value === "number" &&
    Number.isFinite(value) &&
    value > MIN_SANE_COORDINATE;

const normalizePersistedWindowState = (
    value: unknown,
): ManualWindowState | null => {
    if (!value || typeof value !== "object") return null;
    const candidate = value as ManualWindowState;
    let width = normalizeDimension(candidate.width);
    let height = normalizeDimension(candidate.height);
    if (
        width !== undefined &&
        height !== undefined &&
        (width < MIN_SANE_DIMENSION || height < MIN_SANE_DIMENSION)
    ) {
        width = undefined;
        height = undefined;
    }
    const x = isSaneCoordinate(candidate.x) ? Math.round(candidate.x) : undefined;
    const y = isSaneCoordinate(candidate.y) ? Math.round(candidate.y) : undefined;
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
        if (await currentWindow.isMinimized().catch(() => false)) return;

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
            nextState.x = isSaneCoordinate(outerPosition?.x)
                ? Math.round(outerPosition.x)
                : undefined;
            nextState.y = isSaneCoordinate(outerPosition?.y)
                ? Math.round(outerPosition.y)
                : undefined;
            if (
                (nextState.width !== undefined &&
                    nextState.width < MIN_SANE_DIMENSION) ||
                (nextState.height !== undefined &&
                    nextState.height < MIN_SANE_DIMENSION)
            ) {
                // Degenerate capture (e.g. mid-minimize); keep the last
                // good geometry instead.
                return;
            }
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
