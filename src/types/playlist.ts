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
};

export type PlaylistLoopMode = "list" | "shuffle";

export type PlaylistSortMode = "name" | "added";

export type PlaylistScrollState = {
    list: number;
    playlists: Record<string, number>;
};
