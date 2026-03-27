import { computed, reactive } from "vue";
import { Window } from "@tauri-apps/api/window";
import { usePlaybackCommands } from "./usePlaybackCommands";
import { formatTime } from "../utils/formatTime";

type PlayerState = {
  media: {
    url: string;
    lastLoadedUrl: string;
    isFileLoaded: boolean;
    title: string;
  };
  playback: {
    isPlaying: boolean;
    currentTime: number;
    duration: number;
    bufferedTime: number;
    videoBitrate: number;
  };
  window: {
    isFullscreen: boolean;
  };
};

export type PlayerApi = {
  state: PlayerState;
  progressPercent: { value: number };
  bufferedPercent: { value: number };
  isUrlModified: { value: boolean };
  formatTime: (seconds: number) => string;
  loadFile: (resumePosition?: number, autoPlay?: boolean) => Promise<void>;
  loadWebdavFile: (
    connectionId: string,
    filePath: string,
    resumePosition?: number,
    autoPlay?: boolean,
  ) => Promise<void>;
  pickMediaPathsAuto: () => Promise<string[]>;
  pickFiles: () => Promise<string[]>;
  togglePlayPause: () => Promise<void>;
  toggleFullscreen: () => Promise<void>;
  stopPlayback: () => Promise<void>;
  syncFullscreen: () => Promise<void>;
  seek: (position: number) => Promise<void>;
  seekRelative: (position: number) => Promise<void>;
  setLoopFile: (enabled: boolean) => Promise<void>;
  setPlaybackSpeed: (rate: number) => Promise<void>;
};

export const usePlaybackController = (): PlayerApi => {
  const currentWindow = Window.getCurrent();

  const state = reactive<PlayerState>({
    media: {
      // url: "/Users/feng/video/test1080x1080.mp4",
      // url: "/Users/feng/video/DolbyVision/Shogun.S01E01.2024.2160p.DSNP.WEB-DL.H265.DV.DDP5.1.mkv",
      url: "",
      lastLoadedUrl: "",
      isFileLoaded: false,
      title: "",
    },
    playback: {
      isPlaying: false,
      currentTime: 0,
      duration: 0,
      bufferedTime: 0,
      videoBitrate: 0,
    },
    window: {
      isFullscreen: false,
    },
  });

  const progressPercent = computed(() => {
    if (state.playback.duration <= 0) return 0;
    return (state.playback.currentTime / state.playback.duration) * 100;
  });

  const bufferedPercent = computed(() => {
    if (state.playback.duration <= 0) return 0;
    return (state.playback.bufferedTime / state.playback.duration) * 100;
  });

  const isUrlModified = computed(
    () => state.media.url !== state.media.lastLoadedUrl,
  );

  const commands = usePlaybackCommands(state, currentWindow);

  return {
    state,
    progressPercent,
    bufferedPercent,
    isUrlModified,
    formatTime,
    ...commands,
  };
};
