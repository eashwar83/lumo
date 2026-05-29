const LIVE_PLAYBACK_EXTENSIONS = [".m3u", ".m3u8"];

const getUrlPathname = (value: string) => {
    try {
        return decodeURIComponent(new URL(value).pathname).toLowerCase();
    } catch {
        return value.trim().toLowerCase();
    }
};

export const isLikelyLivePlaybackSource = (value: string): boolean => {
    const pathname = getUrlPathname(value);
    return LIVE_PLAYBACK_EXTENSIONS.some((extension) =>
        pathname.endsWith(extension),
    );
};
