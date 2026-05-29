import { onMounted, onUnmounted, ref } from "vue";
import {
    SEEK_STEP_SETTING_LABEL,
    SETTINGS_UPDATED_EVENT,
} from "../mock/settings";
import { loadUiState } from "./useUiStateStore";

type PlaybackShortcutApi = {
    state: {
        media: {
            isFileLoaded: boolean;
            isLivePlayback: boolean;
        };
        playback: {
            duration: number;
            volume: number;
        };
        window: {
            isFullscreen: boolean;
        };
    };
    togglePlayPause: () => Promise<void>;
    seekRelative: (position: number) => Promise<void>;
    setVolume: (volume: number) => Promise<void>;
};

type StoredSettingGroup = {
    title: string;
    items: Array<{ label: string; value: string }>;
};

const DEFAULT_SEEK_STEP_SECONDS = 5;
const VOLUME_STEP = 5;

const parseSeekStepSeconds = (groups?: StoredSettingGroup[]): number => {
    const rawValue = groups
        ?.flatMap((group) => group.items)
        .find((item) => item.label === SEEK_STEP_SETTING_LABEL)?.value;
    const parsed = Number.parseFloat(rawValue ?? "");
    if (!Number.isFinite(parsed) || parsed <= 0) return DEFAULT_SEEK_STEP_SECONDS;
    return parsed;
};

export const usePlaybackShortcuts = (
    player: PlaybackShortcutApi,
    onToggleFullscreen: () => Promise<void>,
    onToggleInfo: () => void,
    onSeekByArrow?: (deltaSeconds: number) => void,
    onVolumeByArrow?: (volume: number) => void,
) => {
    let clickTimer: number | null = null;
    const seekStepSeconds = ref(DEFAULT_SEEK_STEP_SECONDS);

    const refreshSeekStepFromSettings = async () => {
        const stored = await loadUiState<{
            settings?: {
                groups?: StoredSettingGroup[];
            };
        }>();
        seekStepSeconds.value = parseSeekStepSeconds(stored?.settings?.groups);
    };

    const onSettingsUpdated = (event: Event) => {
        const customEvent = event as CustomEvent<{ groups?: StoredSettingGroup[] }>;
        seekStepSeconds.value = parseSeekStepSeconds(customEvent.detail?.groups);
    };

    onMounted(() => {
        void refreshSeekStepFromSettings();
        window.addEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    onUnmounted(() => {
        window.removeEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    const isNonUiTarget = (target: HTMLElement | null) => {
        if (!target) return false;
        if (target.closest(".player-controls")) return false;
        if (target.closest(".top-bar")) return false;
        if (target.closest(".main-panels")) return false;
        return true;
    };

    const onDoubleClick = async (event: MouseEvent) => {
        if (!isNonUiTarget(event.target as HTMLElement | null)) return;
        if (clickTimer !== null) {
            window.clearTimeout(clickTimer);
            clickTimer = null;
        }
        await onToggleFullscreen();
    };

    const onKeydown = async (event: KeyboardEvent) => {
        if (event.code === "Escape") {
            if (!player.state.window.isFullscreen) {
                return;
            }
            event.preventDefault();
            await onToggleFullscreen();
            return;
        }

        const target = event.target as HTMLElement | null;
        const tag = target?.tagName?.toLowerCase();
        if (
            tag === "input" ||
            tag === "textarea" ||
            (target && target.isContentEditable)
        ) {
            return;
        }

        if (
            event.code !== "Space" &&
            event.code !== "ArrowLeft" &&
            event.code !== "ArrowRight" &&
            event.code !== "ArrowUp" &&
            event.code !== "ArrowDown" &&
            event.code !== "KeyI"
        ) {
            return;
        }
        if (event.code === "KeyI") {
            if (!player.state.media.isFileLoaded) return;
            event.preventDefault();
            onToggleInfo();
            return;
        }
        if (event.code === "Space") {
            event.preventDefault();
            await player.togglePlayPause();
            return;
        }
        if (event.code === "ArrowUp" || event.code === "ArrowDown") {
            if (!player.state.media.isFileLoaded) return;
            event.preventDefault();
            const delta = event.code === "ArrowUp" ? VOLUME_STEP : -VOLUME_STEP;
            await player.setVolume(player.state.playback.volume + delta);
            onVolumeByArrow?.(player.state.playback.volume);
            return;
        }
        if (
            !player.state.media.isFileLoaded ||
            player.state.media.isLivePlayback ||
            player.state.playback.duration <= 0
        ) {
            return;
        }
        event.preventDefault();
        const delta =
            event.code === "ArrowLeft"
                ? -seekStepSeconds.value
                : seekStepSeconds.value;
        onSeekByArrow?.(delta);
        await player.seekRelative(delta);
    };

    return {
        onDoubleClick,
        onKeydown,
    };
};
