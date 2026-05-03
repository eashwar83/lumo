export type PlaybackDirection = 1 | -1;

export type ResolvePlaybackNavigationPathOptions = {
    currentPath: string;
    direction: PlaybackDirection;
    resolvePlaylistPath: (
        currentPath: string,
        direction: PlaybackDirection,
    ) => string | null;
    resolveDirectoryPath: (
        currentPath: string,
        direction: PlaybackDirection,
    ) => Promise<string | null>;
};

export const resolvePlaybackNavigationPath = async ({
    currentPath,
    direction,
    resolvePlaylistPath,
    resolveDirectoryPath,
}: ResolvePlaybackNavigationPathOptions): Promise<string | null> => {
    if (!currentPath.trim()) return null;
    return (
        resolvePlaylistPath(currentPath, direction) ??
        (await resolveDirectoryPath(currentPath, direction))
    );
};
