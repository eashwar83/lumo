import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { MEDIA_FILE_EXTENSIONS } from "../constants/media";

type PlayerEffectState = {
  media: {
    url: string;
  };
  window: {
    isFullscreen: boolean;
  };
};

type CurrentWindow = {
  isFullscreen: () => Promise<boolean>;
  setFullscreen: (value: boolean) => Promise<void>;
};

export type ParsedPlaylistEntry = {
  path: string;
  title?: string | null;
};

export const usePlaybackCommands = (
  state: PlayerEffectState,
  currentWindow: CurrentWindow,
) => {
  const MEDIA_FILES_FILTER = [
    {
      name: "Media Files",
      extensions: [...MEDIA_FILE_EXTENSIONS],
    },
  ];

  const normalizeSelectedPaths = (selected: string | string[] | null): string[] => {
    if (!selected) return [];
    return Array.isArray(selected) ? selected : [selected];
  };

  const openVideoPicker = async (): Promise<string[]> => {
    const selected = await open({
      multiple: true,
      directory: false,
      filters: MEDIA_FILES_FILTER,
    });
    return normalizeSelectedPaths(selected);
  };

  const loadFile = async (
    resumePosition?: number,
    autoPlay = true,
  ): Promise<void> => {
    if (state.media.url) {
      await invoke("load_file", {
        payload: { url: state.media.url, resumePosition, autoPlay },
      });
    }
  };

  const loadFileAtUrl = async (
    url: string,
    resumePosition?: number,
    autoPlay = true,
  ): Promise<void> => {
    if (!url) return;
    await invoke("load_file", {
      payload: { url, resumePosition, autoPlay },
    });
  };

  const pickMediaPathsAuto = async (): Promise<string[]> => {
    const selected = await invoke<string[]>("pick_media_paths_native");
    return Array.isArray(selected) ? selected : [];
  };

  const pickFiles = async (): Promise<string[]> => {
    return openVideoPicker();
  };

  const loadNetworkFile = async (
    protocol: string,
    connectionId: string,
    filePath: string,
    resumePosition?: number,
    autoPlay = true,
  ): Promise<void> => {
    await invoke("load_network_file", {
      payload: {
        protocol,
        connectionId,
        filePath,
        resumePosition,
        autoPlay,
      },
    });
  };

  const parsePlaylistFile = async (path: string): Promise<ParsedPlaylistEntry[]> => {
    const response = await invoke<{ entries?: ParsedPlaylistEntry[] }>(
      "parse_playlist_file",
      { payload: { path } },
    );
    return Array.isArray(response?.entries) ? response.entries : [];
  };

  const parsePlaylistSource = async (
    source: string,
  ): Promise<ParsedPlaylistEntry[]> => {
    const response = await invoke<{ entries?: ParsedPlaylistEntry[] }>(
      "parse_playlist_source",
      { payload: { source } },
    );
    return Array.isArray(response?.entries) ? response.entries : [];
  };

  const togglePlayPause = async (): Promise<void> => {
    await invoke("cycle_pause");
  };

  const toggleFullscreen = async (): Promise<void> => {
    const isFull = await currentWindow.isFullscreen();
    await currentWindow.setFullscreen(!isFull);
    state.window.isFullscreen = !isFull;
  };

  type MpvArg = string | number | boolean;
  const runMpvCommand = async (args: MpvArg[]): Promise<void> => {
    await invoke("mpv_run_command", { args });
  };

  const stopPlayback = async (): Promise<void> => {
    await runMpvCommand(["stop"]);
  };

  const syncFullscreen = async (): Promise<void> => {
    state.window.isFullscreen = await currentWindow.isFullscreen();
  };

  const seek = async (position: number): Promise<void> => {
    await invoke("seek_video", { position });
  };

  const seekRelative = async (position: number): Promise<void> => {
    await runMpvCommand(["seek", position, "relative"]);
  };

  const setLoopFile = async (enabled: boolean): Promise<void> => {
    await runMpvCommand(["set", "loop-file", enabled ? "inf" : "no"]);
  };

  const setPlaybackSpeed = async (rate: number): Promise<void> => {
    await invoke("mpv_set_option_string", { name: "speed", value: rate });
  };

  return {
    loadFile,
    loadFileAtUrl,
    loadNetworkFile,
    parsePlaylistFile,
    parsePlaylistSource,
    pickMediaPathsAuto,
    pickFiles,
    togglePlayPause,
    toggleFullscreen,
    stopPlayback,
    syncFullscreen,
    seek,
    seekRelative,
    setLoopFile,
    setPlaybackSpeed,
  };
};
