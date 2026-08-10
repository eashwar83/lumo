import mediaExtensions from "./mediaExtensions.json";

export type MediaExtensionKind = "video" | "audio" | "image";
type MediaExtensionEntry = {
    ext: string;
    kind: MediaExtensionKind;
};

const MEDIA_EXTENSION_ENTRIES = mediaExtensions as readonly MediaExtensionEntry[];

/** Everything mpv will open, images included. */
export const MEDIA_FILE_EXTENSIONS = MEDIA_EXTENSION_ENTRIES.map((entry) => entry.ext);

/**
 * Video and audio only. Lumo is a video player, so a folder of films with
 * cover art beside them should give a playlist of films — the posters are
 * part of the folder, not part of what you sat down to watch.
 */
export const AV_FILE_EXTENSIONS = MEDIA_EXTENSION_ENTRIES.filter(
    (entry) => entry.kind !== "image",
).map((entry) => entry.ext);

export const IMAGE_FILE_EXTENSIONS = MEDIA_EXTENSION_ENTRIES.filter(
    (entry) => entry.kind === "image",
).map((entry) => entry.ext);
