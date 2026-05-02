import { MEDIA_FILE_EXTENSIONS } from "../constants/media";

const MEDIA_EXTENSION_SET = new Set(
    MEDIA_FILE_EXTENSIONS.map((extension) => extension.toLowerCase()),
);

function extractPathExtension(path: string): string | null {
    const cleanPath = path.trim().split(/[?#]/, 1)[0];
    if (!cleanPath) return null;
    const lastDotIndex = cleanPath.lastIndexOf(".");
    const lastSeparatorIndex = Math.max(
        cleanPath.lastIndexOf("/"),
        cleanPath.lastIndexOf("\\"),
    );
    if (lastDotIndex <= lastSeparatorIndex) return null;
    return cleanPath.slice(lastDotIndex + 1).toLowerCase();
}

export function filterDroppedMediaPaths(paths: string[]): string[] {
    const deduped = new Set<string>();
    paths.forEach((path) => {
        const trimmedPath = path.trim();
        if (!trimmedPath) return;
        const extension = extractPathExtension(trimmedPath);
        if (!extension || !MEDIA_EXTENSION_SET.has(extension)) return;
        deduped.add(trimmedPath);
    });
    return [...deduped];
}
