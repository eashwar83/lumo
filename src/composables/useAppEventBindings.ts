import { onMounted, onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
    Window,
    LogicalSize,
    PhysicalPosition,
    currentMonitor,
    primaryMonitor,
} from "@tauri-apps/api/window";
import type { ProgressPayload, MediaTrack } from "../types/media";
import type { PlayerApi } from "./usePlaybackController";

type TracksUpdatePayload = {
    tracks: MediaTrack[];
};

type EndFilePayload = {
    reason?: string;
};

type PlayerEventApi = Pick<PlayerApi, "state" | "syncFullscreen">;

type TracksApi = {
    handleTracksUpdate: (payload: { tracks: MediaTrack[] }) => void;
};

type UiApi = {
    showControls: { value: boolean };
    onUserInteraction: () => void;
    resetInactivityTimer: () => void;
    cleanup: () => void;
};

type AppEventBindingsOptions = {
    player: PlayerEventApi;
    tracks: TracksApi;
    ui: UiApi;
    onFullscreenTransition: () => void;
    onFullscreenTransitionEnd: () => void;
    onCloseAllMenus: (event: MouseEvent) => void;
    onKeydown: (event: KeyboardEvent) => void;
    onDoubleClick: (event: MouseEvent) => void;
    setWindowControlsVisible: (visible: boolean) => Promise<void>;
    onFileLoaded?: () => void | Promise<void>;
    onPlaybackRestart?: () => void | Promise<void>;
    onProgress?: (payload: ProgressPayload) => void;
    onEndFile?: (payload: EndFilePayload) => void | Promise<void>;
};

const AUTO_RESIZE_MIN_WIDTH = 720;
const AUTO_RESIZE_MIN_HEIGHT = 480;

export const useAppEventBindings = ({
    player,
    tracks,
    ui,
    onFullscreenTransition,
    onFullscreenTransitionEnd,
    onCloseAllMenus,
    onKeydown,
    onDoubleClick,
    setWindowControlsVisible,
    onFileLoaded,
    onPlaybackRestart,
    onProgress,
    onEndFile,
}: AppEventBindingsOptions) => {
    // 事件监听器引用
    let unlistenProgress: UnlistenFn | null = null;
    let unlistenFileLoaded: UnlistenFn | null = null;
    let unlistenPlaybackRestart: UnlistenFn | null = null;
    let unlistenResize: UnlistenFn | null = null;
    let unlistenTracksUpdate: UnlistenFn | null = null;
    let unlistenWindowResized: UnlistenFn | null = null;
    let unlistenFullscreenWill: UnlistenFn | null = null;
    let unlistenEndFile: UnlistenFn | null = null;
    let unlistenMediaTitle: UnlistenFn | null = null;
    let unlistenHwdecCurrent: UnlistenFn | null = null;

    const windowEventHandlers: Array<[keyof WindowEventMap, EventListener]> = [
        ["mousemove", () => ui.onUserInteraction()],
        ["mousedown", () => ui.onUserInteraction()],
        ["click", (event) => onCloseAllMenus(event as MouseEvent)],
        ["keydown", (event) => onKeydown(event as KeyboardEvent)],
        ["dblclick", (event) => onDoubleClick(event as MouseEvent)],
    ];

    onMounted(async () => {
        const currentWindow = Window.getCurrent();
        unlistenWindowResized = await currentWindow.onResized(async () => {
            await player.syncFullscreen();
            onFullscreenTransitionEnd();
        });

        unlistenFullscreenWill = await listen("fullscreen-will-change", () => {
            onFullscreenTransition();
        });

        // 监听播放进度
        unlistenProgress = await listen<ProgressPayload>(
            "mpv-progress-update",
            (event) => {
                player.state.playback.currentTime = event.payload.time_pos;
                player.state.playback.duration = event.payload.duration;
                player.state.playback.bufferedTime =
                    typeof event.payload.buffered_pos === "number" &&
                    Number.isFinite(event.payload.buffered_pos)
                        ? event.payload.buffered_pos
                        : event.payload.time_pos;
                player.state.playback.isPlaying = event.payload.is_playing;
                player.state.playback.isBuffering =
                    event.payload.is_buffering === true;
                player.state.playback.downloadSpeedBps =
                    typeof event.payload.download_speed_bps === "number" &&
                    Number.isFinite(event.payload.download_speed_bps) &&
                    event.payload.download_speed_bps > 0
                        ? event.payload.download_speed_bps
                        : 0;
                player.state.playback.videoBitrate =
                    typeof event.payload.video_bitrate === "number" &&
                    Number.isFinite(event.payload.video_bitrate) &&
                    event.payload.video_bitrate > 0
                        ? event.payload.video_bitrate
                        : 0;
                if (onProgress) {
                    onProgress(event.payload);
                }
            },
        );

        // 监听文件加载完成
        unlistenFileLoaded = await listen("file_loaded", () => {
            player.state.media.isFileLoaded = true;
            player.state.media.lastLoadedUrl = player.state.media.url;
            ui.resetInactivityTimer();
            if (onFileLoaded) {
                void onFileLoaded();
            }
        });

        unlistenPlaybackRestart = await listen("mpv-playback-restart", () => {
            if (onPlaybackRestart) {
                void onPlaybackRestart();
            }
        });

        unlistenEndFile = await listen<EndFilePayload>("mpv-end-file", (event) => {
            if (onEndFile) {
                void onEndFile(event.payload ?? {});
            }
        });

        unlistenMediaTitle = await listen<string>("mpv-media-title", (event) => {
            const title = typeof event.payload === "string" ? event.payload.trim() : "";
            player.state.media.title = title;
        });

        unlistenHwdecCurrent = await listen<string>(
            "mpv-hwdec-current",
            (event) => {
                const hwdec =
                    typeof event.payload === "string"
                        ? event.payload.trim()
                        : "";
                player.state.playback.hwdecCurrent = hwdec;
            },
        );

        // Listen for resize events
        unlistenResize = await listen<[number, number]>(
            "resize_window",
            async ({ payload }) => {
                const [width, height] = payload;
                if (width <= 0 || height <= 0) return;

                const monitor =
                    (await currentMonitor()) ?? (await primaryMonitor());
                const scale = await currentWindow.scaleFactor();
                const [innerSize, outerSize] = await Promise.all([
                    currentWindow.innerSize(),
                    currentWindow.outerSize(),
                ]);
                const frameW = Math.max(
                    0,
                    (outerSize.width - innerSize.width) / scale,
                );
                const frameH = Math.max(
                    0,
                    (outerSize.height - innerSize.height) / scale,
                );
                const workAreaSize = monitor?.workArea?.size ?? monitor?.size;
                const workAreaPos =
                    monitor?.workArea?.position ?? monitor?.position;
                const workAreaW = workAreaSize
                    ? workAreaSize.width / scale
                    : null;
                const workAreaH = workAreaSize
                    ? workAreaSize.height / scale
                    : null;
                const maxW =
                    workAreaW !== null
                        ? Math.max(1, Math.floor(workAreaW - frameW))
                        : Math.max(1, Math.floor(width / scale));
                const maxH =
                    workAreaH !== null
                        ? Math.max(1, Math.floor(workAreaH - frameH))
                        : Math.max(1, Math.floor(height / scale));
                const minW = Math.min(AUTO_RESIZE_MIN_WIDTH, maxW);
                const minH = Math.min(AUTO_RESIZE_MIN_HEIGHT, maxH);

                // MPV width/height are video pixel dimensions. Convert to logical size
                // so 1920x1080 maps to 960x540 on a 2x Retina display.
                const logicalWidth = width / scale;
                const logicalHeight = height / scale;
                const ratio = logicalWidth / logicalHeight;
                let targetW = Math.min(logicalWidth, maxW);
                let targetH = targetW / ratio;
                if (targetH > maxH) {
                    targetH = maxH;
                    targetW = targetH * ratio;
                }
                if (targetW < minW || targetH < minH) {
                    const scaleUp = Math.max(minW / targetW, minH / targetH);
                    targetW *= scaleUp;
                    targetH *= scaleUp;
                    if (targetW > maxW || targetH > maxH) {
                        const scaleDown = Math.min(maxW / targetW, maxH / targetH);
                        targetW *= scaleDown;
                        targetH *= scaleDown;
                    }
                }
                targetW = Math.min(maxW, Math.max(minW, targetW));
                targetH = Math.min(maxH, Math.max(minH, targetH));

                await currentWindow.setSize(
                    new LogicalSize(Math.floor(targetW), Math.floor(targetH)),
                );
                await currentWindow.center();

                if (
                    workAreaSize !== undefined &&
                    workAreaPos !== undefined &&
                    workAreaSize !== null &&
                    workAreaPos !== null
                ) {
                    const [outerPos, outerSizeAfterResize] = await Promise.all([
                        currentWindow.outerPosition(),
                        currentWindow.outerSize(),
                    ]);

                    const minX = workAreaPos.x;
                    const minY = workAreaPos.y;
                    const maxX =
                        workAreaPos.x +
                        Math.max(0, workAreaSize.width - outerSizeAfterResize.width);
                    const maxY =
                        workAreaPos.y +
                        Math.max(0, workAreaSize.height - outerSizeAfterResize.height);
                    const clampedX = Math.min(Math.max(outerPos.x, minX), maxX);
                    const clampedY = Math.min(Math.max(outerPos.y, minY), maxY);

                    if (clampedX !== outerPos.x || clampedY !== outerPos.y) {
                        await currentWindow.setPosition(
                            new PhysicalPosition(clampedX, clampedY),
                        );
                    }
                }
            },
        );

        // 监听轨道更新
        unlistenTracksUpdate = await listen<TracksUpdatePayload>(
            "mpv-tracks-update",
            (event) => {
                tracks.handleTracksUpdate(event.payload);
            },
        );

        // 全局交互监听
        windowEventHandlers.forEach(([eventName, handler]) => {
            window.addEventListener(eventName, handler);
        });
        await setWindowControlsVisible(ui.showControls.value);
    });

    onUnmounted(() => {
        unlistenProgress?.();
        unlistenFileLoaded?.();
        unlistenPlaybackRestart?.();
        unlistenResize?.();
        unlistenTracksUpdate?.();
        unlistenWindowResized?.();
        unlistenFullscreenWill?.();
        unlistenEndFile?.();
        unlistenMediaTitle?.();
        unlistenHwdecCurrent?.();
        ui.cleanup();
        windowEventHandlers.forEach(([eventName, handler]) => {
            window.removeEventListener(eventName, handler);
        });
    });
};
