export type PlaylistEntry = {
    path: string;
    title?: string;
    iconUrl?: string;
    addedAt: number;
};

export type Playlist = {
    id: string;
    name: string;
    entries: PlaylistEntry[];
    createdAt: number;
};

export const FAVORITES_PLAYLIST_ID = "favorites";
export const LEGACY_FAVOURITE_PLAYLIST_ID = "favourite";
export const FAVORITES_PLAYLIST_NAME = "Favorites";

// A folder groups favourite entries. The default folder always exists and holds
// any favourite not explicitly moved elsewhere (including legacy favourites).
export type FavoriteFolder = {
    id: string;
    name: string;
    createdAt: number;
};

export const DEFAULT_FAVORITE_FOLDER_ID = "fav_default";
export const DEFAULT_FAVORITE_FOLDER_NAME = "General";

// Persisted shape of the favourites-folder metadata (opaque `favoritesMeta`
// slice of ui-state): the folder list plus a path -> folderId assignment map.
export type FavoritesMeta = {
    folders: FavoriteFolder[];
    assignments: Record<string, string>;
    sort?: FavoriteSortMode;
    /** path -> length in seconds, where known. Feeds the length sort. */
    durations?: Record<string, number>;
};

export const FAVORITE_SORT_MODES = [
    "date-desc",
    "date-asc",
    "name-asc",
    "name-desc",
    "length-asc",
    "length-desc",
] as const;
export type FavoriteSortMode = (typeof FAVORITE_SORT_MODES)[number];
// Newest first: the order the tab had before sorting was configurable.
export const DEFAULT_FAVORITE_SORT_MODE: FavoriteSortMode = "date-desc";

export type PlaylistLoopMode = "list" | "shuffle";

export type PlaylistSortMode = "name" | "added";

export type PlaylistScrollState = {
    list: number;
    playlists: Record<string, number>;
};
