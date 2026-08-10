import { isPlaylistFilePath } from "./usePlaylistFileKinds";

export function filterDroppedMediaPaths(paths: string[]): string[] {
    const deduped = new Set<string>();
    paths.forEach((path) => {
        const trimmedPath = path.trim();
        if (!trimmedPath) return;
        if (!isPlaylistFilePath(trimmedPath)) return;
        deduped.add(trimmedPath);
    });
    return [...deduped];
}
