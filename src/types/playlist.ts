export type PlaylistEntry = {
    path: string;
    title?: string;
    addedAt: number;
};

export type Playlist = {
    id: string;
    name: string;
    entries: PlaylistEntry[];
    createdAt: number;
};

export type PlaylistLoopMode = "list" | "shuffle";

export type PlaylistSortMode = "name" | "added";

export type PlaylistScrollState = {
    list: number;
    playlists: Record<string, number>;
};
