import { invoke } from "@tauri-apps/api/core";

type ResolvedPlaybackSourceBase = {
    playbackKey: string;
    displayPath: string;
    parentPath?: string | null;
};

export type ResolvedPlaybackSource =
    | (ResolvedPlaybackSourceBase & {
          kind: "local";
          filePath: string;
      })
    | (ResolvedPlaybackSourceBase & {
          kind: "webdav";
          connectionId: string;
          filePath: string;
      })
    | (ResolvedPlaybackSourceBase & {
          kind: "dlna";
          connectionId: string;
          resourceUrl: string;
      })
    | (ResolvedPlaybackSourceBase & {
          kind: "smb";
          connectionId: string;
          filePath: string;
      })
    | (ResolvedPlaybackSourceBase & {
          kind: "directSmb";
          resourceUrl: string;
      });

export const resolvePlaybackSource = (keyOrUrl: string) =>
    invoke<ResolvedPlaybackSource>("resolve_playback_source", { keyOrUrl });
