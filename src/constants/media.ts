import mediaExtensions from "./mediaExtensions.json";

type MediaExtensionKind = "video" | "audio" | "image";
type MediaExtensionEntry = {
    ext: string;
    kind: MediaExtensionKind;
};

const MEDIA_EXTENSION_ENTRIES = mediaExtensions as readonly MediaExtensionEntry[];

export const MEDIA_FILE_EXTENSIONS = MEDIA_EXTENSION_ENTRIES.map((entry) => entry.ext);
