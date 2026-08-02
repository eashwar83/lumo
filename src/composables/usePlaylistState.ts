import { computed, ref, watch } from "vue";
import {
    DEFAULT_FAVORITE_FOLDER_ID,
    DEFAULT_FAVORITE_FOLDER_NAME,
    FAVORITES_PLAYLIST_ID,
    FAVORITES_PLAYLIST_NAME,
    LEGACY_FAVOURITE_PLAYLIST_ID,
    type FavoriteFolder,
    type FavoritesMeta,
    type Playlist,
    type PlaylistEntry,
    type PlaylistLoopMode,
    type PlaylistSortMode,
} from "../types/playlist";
import { getPathDisplayName } from "../utils/getPathDisplayName";
import {
    createDebouncedUiStateSaver,
    loadUiState,
} from "./useUiStateStore";

type PersistedPlaylistState = {
    playlists?: Playlist[];
    playlistLoopMode?: PlaylistLoopMode;
    playlistSortMode?: PlaylistSortMode;
    activePlaylistId?: string | null;
};

type CreatePlaylistOptions = {
    name?: string;
    openInDrawer?: boolean;
    setAsPlayback?: boolean;
};

type CreatePlaylistEntryInput = {
    path: string;
    title?: string;
    iconUrl?: string;
};

// Reserved id for the transient "current folder" playlist populated by the
// Auto-Load Folder feature. It is never persisted (see toPersistedState).
export const AUTOLOAD_PLAYLIST_ID = "pl_autoload_folder";

// Reserved id for the transient YouTube "Up next" queue. Session-only,
// never persisted; Previous/Next and auto-advance walk it like any playlist.
export const YT_UPNEXT_PLAYLIST_ID = "pl_youtube_upnext";

const isValidSortMode = (value: unknown): value is PlaylistSortMode =>
    value === "name" || value === "added";

const createPlaylistId = () =>
    `pl_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;

const createFavoriteFolderId = () =>
    `ff_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;

const createDefaultFavoriteFolder = (): FavoriteFolder => ({
    id: DEFAULT_FAVORITE_FOLDER_ID,
    name: DEFAULT_FAVORITE_FOLDER_NAME,
    createdAt: 0,
});

// Coerce whatever was persisted into a sane FavoritesMeta: the default folder is
// always present and always first, names are trimmed, and stray fields dropped.
const normalizeFavoritesMeta = (raw: unknown): FavoritesMeta => {
    const value = (raw ?? {}) as Partial<FavoritesMeta>;
    const seen = new Set<string>();
    const folders: FavoriteFolder[] = [createDefaultFavoriteFolder()];
    seen.add(DEFAULT_FAVORITE_FOLDER_ID);
    if (Array.isArray(value.folders)) {
        for (const folder of value.folders) {
            const id = folder?.id?.trim();
            const name = folder?.name?.trim();
            if (!id || !name || seen.has(id)) continue;
            if (id === DEFAULT_FAVORITE_FOLDER_ID) {
                // Preserve a renamed default folder.
                folders[0] = { ...folders[0], name };
                continue;
            }
            seen.add(id);
            folders.push({
                id,
                name,
                createdAt:
                    typeof folder.createdAt === "number"
                        ? folder.createdAt
                        : Date.now(),
            });
        }
    }
    const assignments: Record<string, string> = {};
    if (value.assignments && typeof value.assignments === "object") {
        for (const [path, folderId] of Object.entries(value.assignments)) {
            const key = path.trim();
            if (key && typeof folderId === "string" && seen.has(folderId)) {
                assignments[key] = folderId;
            }
        }
    }
    return { folders, assignments };
};

const isFavoritesPlaylist = (playlistId: string | null) =>
    playlistId === FAVORITES_PLAYLIST_ID;

const normalizePlaylistId = (playlistId: string | null | undefined) =>
    playlistId === LEGACY_FAVOURITE_PLAYLIST_ID
        ? FAVORITES_PLAYLIST_ID
        : playlistId ?? null;

const createFavoritesPlaylist = (): Playlist => ({
    id: FAVORITES_PLAYLIST_ID,
    name: FAVORITES_PLAYLIST_NAME,
    entries: [],
    createdAt: 0,
});

const stripExtension = (fileName: string) => {
    const trimmed = fileName.trim();
    if (!trimmed) return "";
    const dotIndex = trimmed.lastIndexOf(".");
    if (dotIndex <= 0) return trimmed;
    return trimmed.slice(0, dotIndex);
};

const getParentDirectoryName = (path: string) => {
    const parts = path.split(/[/\\]+/).filter(Boolean);
    if (parts.length < 2) return "";
    return parts[parts.length - 2] ?? "";
};

const isNumericName = (value: string) => /^\d+$/.test(value.trim());

const getCommonPrefix = (values: string[]) => {
    if (!values.length) return "";
    let prefix = values[0] ?? "";
    for (let index = 1; index < values.length; index += 1) {
        const current = values[index] ?? "";
        while (prefix && !current.startsWith(prefix)) {
            prefix = prefix.slice(0, -1);
        }
        if (!prefix) return "";
    }
    return prefix.replace(/[\s._-]+$/, "").trim();
};

const derivePlaylistNameFromPaths = (paths: string[], fallback: string) => {
    const fileNames = paths
        .map((path) => getPathDisplayName(path).trim())
        .filter(Boolean);
    const itemNames = fileNames.map(stripExtension).filter(Boolean);
    if (!itemNames.length) return fallback;

    if (itemNames.every(isNumericName)) {
        const folderNames = paths
            .map((path) => getParentDirectoryName(path).trim())
            .filter(Boolean);
        const uniqueFolderNames = Array.from(new Set(folderNames));
        if (uniqueFolderNames.length === 1) {
            return uniqueFolderNames[0] ?? fallback;
        }
        if (uniqueFolderNames.length > 1) {
            return uniqueFolderNames[0] ?? fallback;
        }
    }

    if (itemNames.length === 1) return itemNames[0] ?? fallback;

    const commonPrefix = getCommonPrefix(itemNames);
    if (commonPrefix.length >= 2) return commonPrefix;

    return itemNames[0] ?? fallback;
};

const normalizePlaylistEntries = (entries: PlaylistEntry[]): PlaylistEntry[] => {
    const unique = new Map<string, PlaylistEntry>();
    entries.forEach((entry) => {
        const path = entry?.path?.trim();
        if (!path) return;
        const title = entry?.title?.trim() || undefined;
        const iconUrl = entry?.iconUrl?.trim() || undefined;
        unique.set(path, {
            path,
            title,
            iconUrl,
            addedAt:
                typeof entry.addedAt === "number" ? entry.addedAt : Date.now(),
        });
    });
    return Array.from(unique.values());
};

const normalizePlaylistName = (name: string | undefined, fallback: string) => {
    const trimmed = name?.trim();
    return trimmed || fallback;
};

const normalizePlaylists = (items: Playlist[] | undefined): Playlist[] => {
    const source = items ?? [];
    let favoritesPlaylist: Playlist | null = null;
    const userPlaylists: Playlist[] = [];

    source.forEach((item, index) => {
        const normalizedPlaylist: Playlist = {
            id: item.id || createPlaylistId(),
            name: normalizePlaylistName(item.name, `Playlist ${index + 1}`),
            entries: normalizePlaylistEntries(item.entries ?? []),
            createdAt:
                typeof item.createdAt === "number" ? item.createdAt : Date.now(),
        };

        if (
            normalizedPlaylist.id === FAVORITES_PLAYLIST_ID ||
            normalizedPlaylist.id === LEGACY_FAVOURITE_PLAYLIST_ID
        ) {
            favoritesPlaylist = {
                ...normalizedPlaylist,
                id: FAVORITES_PLAYLIST_ID,
                name: FAVORITES_PLAYLIST_NAME,
                createdAt: normalizedPlaylist.createdAt || 0,
            };
            return;
        }

        userPlaylists.push(normalizedPlaylist);
    });

    return [favoritesPlaylist ?? createFavoritesPlaylist(), ...userPlaylists];
};

const sortEntries = (
    entries: PlaylistEntry[],
    mode: PlaylistSortMode,
): PlaylistEntry[] => {
    const list = [...entries];
    if (mode === "name") {
        list.sort((a, b) =>
            (a.title?.trim() || getPathDisplayName(a.path)).localeCompare(
                b.title?.trim() || getPathDisplayName(b.path),
                undefined,
                { numeric: true, sensitivity: "base" },
            ),
        );
    } else {
        list.sort((a, b) => b.addedAt - a.addedAt);
    }
    return list;
};

export const usePlaylistState = () => {
    const playlists = ref<Playlist[]>([createFavoritesPlaylist()]);
    const activePlaylistId = ref<string | null>(null);
    const playbackPlaylistId = ref<string | null>(null);
    const loopMode = ref<PlaylistLoopMode>("list");
    const sortMode = ref<PlaylistSortMode>("added");
    const isLoopOne = ref(false);

    // --- favourite folders --------------------------------------------------
    const favoriteFolders = ref<FavoriteFolder[]>([createDefaultFavoriteFolder()]);
    const favoriteAssignments = ref<Record<string, string>>({});
    // The folder the Favourites view is currently showing (null = "All"). New
    // quick-favourites land here when set, otherwise in the default folder.
    const activeFavoriteFolderId = ref<string | null>(null);

    // Self-contained persistence of the opaque `favoritesMeta` ui-state slice,
    // independent of the playlist persistence path.
    let favoritesMetaReady = false;
    const favoritesMetaSaver = createDebouncedUiStateSaver(300);
    const persistFavoritesMeta = () => {
        if (!favoritesMetaReady) return;
        favoritesMetaSaver.saveDebounced({
            favoritesMeta: {
                folders: favoriteFolders.value,
                assignments: favoriteAssignments.value,
            } satisfies FavoritesMeta,
        });
    };
    void (async () => {
        const stored = await loadUiState<{ favoritesMeta?: unknown }>();
        const meta = normalizeFavoritesMeta(stored?.favoritesMeta);
        favoriteFolders.value = meta.folders;
        favoriteAssignments.value = meta.assignments;
        favoritesMetaReady = true;
    })();
    watch([favoriteFolders, favoriteAssignments], persistFavoritesMeta, {
        deep: true,
    });

    const activePlaylist = computed<Playlist | null>(
        () =>
            playlists.value.find((item) => item.id === activePlaylistId.value) ??
            null,
    );
    const playlist = computed<PlaylistEntry[]>(() => activePlaylist.value?.entries ?? []);
    const orderedPlaylist = computed(() =>
        sortEntries(playlist.value, sortMode.value),
    );

    const hasPlaylist = (playlistId: string | null) =>
        !!playlistId && playlists.value.some((item) => item.id === playlistId);

    const findPlaylistById = (playlistId: string | null): Playlist | null => {
        if (!playlistId) return null;
        return playlists.value.find((item) => item.id === playlistId) ?? null;
    };

    const getOrderedEntriesByPlaylistId = (playlistId: string | null) => {
        const target = findPlaylistById(playlistId);
        if (!target) return [];
        return sortEntries(target.entries, sortMode.value);
    };

    const syncSelectionAfterMutation = () => {
        if (!hasPlaylist(activePlaylistId.value)) {
            activePlaylistId.value = null;
        }
        if (!hasPlaylist(playbackPlaylistId.value)) {
            playbackPlaylistId.value = null;
        }
    };

    const addManyToPlaylist = (playlistId: string, paths: string[]) => {
        const playlistIndex = playlists.value.findIndex((item) => item.id === playlistId);
        if (playlistIndex < 0) return;

        const target = playlists.value[playlistIndex];
        const existing = new Set(target.entries.map((item) => item.path));
        const dedupedPaths = Array.from(
            new Set(paths.map((item) => item.trim()).filter(Boolean)),
        );
        const timestamp = Date.now();
        const additions = dedupedPaths
            .filter((path) => !existing.has(path))
            .map((path, index) => ({ path, addedAt: timestamp + index }));
        if (!additions.length) return;

        const nextPlaylists = [...playlists.value];
        nextPlaylists[playlistIndex] = {
            ...target,
            entries: [...target.entries, ...additions],
        };
        playlists.value = nextPlaylists;
    };

    const addEntryToPlaylist = (
        playlistId: string,
        item: CreatePlaylistEntryInput,
    ) => {
        const playlistIndex = playlists.value.findIndex((entry) => entry.id === playlistId);
        if (playlistIndex < 0) return;

        const path = item.path?.trim() ?? "";
        if (!path) return;

        const target = playlists.value[playlistIndex];
        const title = item.title?.trim() || undefined;
        const iconUrl = item.iconUrl?.trim() || undefined;
        const existingIndex = target.entries.findIndex((entry) => entry.path === path);
        const nextEntries = [...target.entries];

        if (existingIndex >= 0) {
            nextEntries[existingIndex] = {
                ...nextEntries[existingIndex],
                title: title ?? nextEntries[existingIndex]?.title,
                iconUrl: iconUrl ?? nextEntries[existingIndex]?.iconUrl,
            };
        } else {
            nextEntries.push({
                path,
                title,
                iconUrl,
                addedAt: Date.now(),
            });
        }

        const nextPlaylists = [...playlists.value];
        nextPlaylists[playlistIndex] = {
            ...target,
            entries: nextEntries,
        };
        playlists.value = nextPlaylists;
    };

    const folderExists = (id: string) =>
        favoriteFolders.value.some((folder) => folder.id === id);

    // Set/clear a favourite's folder. The default folder means "no assignment".
    const assignFavoriteFolder = (path: string, folderId: string) => {
        const key = path.trim();
        if (!key) return;
        const next = { ...favoriteAssignments.value };
        if (folderId === DEFAULT_FAVORITE_FOLDER_ID || !folderExists(folderId)) {
            delete next[key];
        } else {
            next[key] = folderId;
        }
        favoriteAssignments.value = next;
    };

    const addToFavorites = (
        item: CreatePlaylistEntryInput,
        folderId?: string,
    ) => {
        addEntryToPlaylist(FAVORITES_PLAYLIST_ID, item);
        const path = item.path?.trim();
        if (!path) return;
        // Quick-favourites land in the folder currently open in the Favourites
        // view (activeFavoriteFolderId), else the default folder.
        const target =
            folderId ??
            activeFavoriteFolderId.value ??
            DEFAULT_FAVORITE_FOLDER_ID;
        assignFavoriteFolder(path, target);
    };

    const favoritesPlaylist = computed<Playlist | null>(() =>
        findPlaylistById(FAVORITES_PLAYLIST_ID),
    );

    // Favourited videos, newest first.
    const favorites = computed<PlaylistEntry[]>(() =>
        [...(favoritesPlaylist.value?.entries ?? [])].sort(
            (a, b) => b.addedAt - a.addedAt,
        ),
    );

    const isFavorite = (path: string): boolean => {
        const normalized = path.trim();
        if (!normalized) return false;
        return (favoritesPlaylist.value?.entries ?? []).some(
            (entry) => entry.path === normalized,
        );
    };

    const removeFromFavorites = (path: string) => {
        const normalized = path.trim();
        if (!normalized) return;
        const index = playlists.value.findIndex(
            (item) => item.id === FAVORITES_PLAYLIST_ID,
        );
        if (index < 0) return;
        const target = playlists.value[index];
        const nextEntries = target.entries.filter(
            (entry) => entry.path !== normalized,
        );
        if (nextEntries.length === target.entries.length) return;
        const nextPlaylists = [...playlists.value];
        nextPlaylists[index] = { ...target, entries: nextEntries };
        playlists.value = nextPlaylists;
        // Drop the folder assignment for a favourite that no longer exists.
        if (favoriteAssignments.value[normalized]) {
            const nextAssignments = { ...favoriteAssignments.value };
            delete nextAssignments[normalized];
            favoriteAssignments.value = nextAssignments;
        }
    };

    // Make the drawer show the Favourites list, creating the playlist if it is
    // somehow missing. Robust entry point used by the header/nav toggle.
    const openFavoritesView = () => {
        if (
            !playlists.value.some((item) => item.id === FAVORITES_PLAYLIST_ID)
        ) {
            playlists.value = [createFavoritesPlaylist(), ...playlists.value];
        }
        activePlaylistId.value = FAVORITES_PLAYLIST_ID;
    };

    const clearFavorites = () => {
        const index = playlists.value.findIndex(
            (item) => item.id === FAVORITES_PLAYLIST_ID,
        );
        if (index < 0 || !playlists.value[index].entries.length) return;
        const nextPlaylists = [...playlists.value];
        nextPlaylists[index] = { ...nextPlaylists[index], entries: [] };
        playlists.value = nextPlaylists;
    };

    // Returns the new state: true if now favourited, false if removed.
    const toggleFavorite = (item: CreatePlaylistEntryInput): boolean => {
        const normalized = item.path?.trim() ?? "";
        if (!normalized) return false;
        if (isFavorite(normalized)) {
            removeFromFavorites(normalized);
            return false;
        }
        addToFavorites(item);
        return true;
    };

    // --- favourite folders --------------------------------------------------

    // Default folder first, then user folders in creation order.
    const favoriteFolderList = computed<FavoriteFolder[]>(() => {
        const def =
            favoriteFolders.value.find(
                (folder) => folder.id === DEFAULT_FAVORITE_FOLDER_ID,
            ) ?? createDefaultFavoriteFolder();
        const others = favoriteFolders.value.filter(
            (folder) => folder.id !== DEFAULT_FAVORITE_FOLDER_ID,
        );
        return [def, ...others];
    });

    const folderIdForPath = (path: string): string => {
        const assigned = favoriteAssignments.value[path.trim()];
        return assigned && folderExists(assigned)
            ? assigned
            : DEFAULT_FAVORITE_FOLDER_ID;
    };

    // folderId -> favourite entries (in the same newest-first order as favorites).
    const favoritesByFolder = computed<Record<string, PlaylistEntry[]>>(() => {
        const groups: Record<string, PlaylistEntry[]> = {};
        for (const folder of favoriteFolders.value) groups[folder.id] = [];
        for (const entry of favorites.value) {
            const fid = folderIdForPath(entry.path);
            (groups[fid] ??= []).push(entry);
        }
        return groups;
    });

    const favoriteFolderCounts = computed<Record<string, number>>(() => {
        const counts: Record<string, number> = {};
        for (const [fid, list] of Object.entries(favoritesByFolder.value)) {
            counts[fid] = list.length;
        }
        return counts;
    });

    const createFavoriteFolder = (name: string): string | null => {
        const trimmed = name.trim();
        if (!trimmed) return null;
        const id = createFavoriteFolderId();
        favoriteFolders.value = [
            ...favoriteFolders.value,
            { id, name: trimmed, createdAt: Date.now() },
        ];
        return id;
    };

    const renameFavoriteFolder = (id: string, name: string) => {
        const trimmed = name.trim();
        if (!trimmed) return;
        favoriteFolders.value = favoriteFolders.value.map((folder) =>
            folder.id === id ? { ...folder, name: trimmed } : folder,
        );
    };

    const deleteFavoriteFolder = (id: string) => {
        if (id === DEFAULT_FAVORITE_FOLDER_ID) return; // default is permanent
        if (!folderExists(id)) return;
        favoriteFolders.value = favoriteFolders.value.filter(
            (folder) => folder.id !== id,
        );
        // Its videos fall back to the default folder (drop their assignments).
        const next = { ...favoriteAssignments.value };
        for (const [path, fid] of Object.entries(next)) {
            if (fid === id) delete next[path];
        }
        favoriteAssignments.value = next;
        if (activeFavoriteFolderId.value === id) {
            activeFavoriteFolderId.value = null;
        }
    };

    const moveFavoriteToFolder = (path: string, folderId: string) => {
        if (!isFavorite(path)) return;
        assignFavoriteFolder(path, folderId);
    };

    const setActiveFavoriteFolder = (id: string | null) => {
        activeFavoriteFolderId.value = id && folderExists(id) ? id : null;
    };

    // --- favourites export / import ----------------------------------------

    const exportFavoritesData = () => ({
        kind: "lumo-favorites",
        version: 1,
        exportedAt: Date.now(),
        folders: favoriteFolders.value,
        favorites: favorites.value.map((entry) => ({
            path: entry.path,
            title: entry.title,
            addedAt: entry.addedAt,
            folderId: folderIdForPath(entry.path),
        })),
    });

    type ImportedFavorite = {
        path?: string;
        title?: string;
        addedAt?: number;
        folderId?: string;
    };

    const importFavoritesData = (
        data: unknown,
        mode: "merge" | "replace",
    ): number => {
        const payload = (data ?? {}) as {
            folders?: unknown;
            favorites?: unknown;
        };
        const incomingFolders = normalizeFavoritesMeta({
            folders: payload.folders,
            assignments: {},
        }).folders;
        const incomingEntries: ImportedFavorite[] = Array.isArray(payload.favorites)
            ? (payload.favorites as ImportedFavorite[])
            : [];

        if (mode === "replace") {
            clearFavorites();
            favoriteFolders.value = incomingFolders;
            favoriteAssignments.value = {};
        } else {
            const existing = new Set(
                favoriteFolders.value.map((folder) => folder.id),
            );
            const merged = [...favoriteFolders.value];
            for (const folder of incomingFolders) {
                if (!existing.has(folder.id)) {
                    merged.push(folder);
                    existing.add(folder.id);
                }
            }
            favoriteFolders.value = merged;
        }

        const nextAssignments =
            mode === "replace" ? {} : { ...favoriteAssignments.value };
        let added = 0;
        for (const entry of incomingEntries) {
            const path = entry?.path?.trim();
            if (!path) continue;
            addEntryToPlaylist(FAVORITES_PLAYLIST_ID, {
                path,
                title: entry.title,
            });
            const fid =
                entry.folderId && folderExists(entry.folderId)
                    ? entry.folderId
                    : DEFAULT_FAVORITE_FOLDER_ID;
            if (fid === DEFAULT_FAVORITE_FOLDER_ID) {
                delete nextAssignments[path];
            } else {
                nextAssignments[path] = fid;
            }
            added += 1;
        }
        favoriteAssignments.value = nextAssignments;
        return added;
    };

    const createPlaylistWithEntries = (
        items: CreatePlaylistEntryInput[],
        options: CreatePlaylistOptions = {},
    ): string | null => {
        const timestamp = Date.now();
        const normalizedItems = items
            .map((item) => ({
                path: item.path?.trim() ?? "",
                title: item.title?.trim() || undefined,
                iconUrl: item.iconUrl?.trim() || undefined,
            }))
            .filter((item) => !!item.path);
        const entries = normalizedItems.map((item, index) => ({
            path: item.path,
            title: item.title,
            iconUrl: item.iconUrl,
            addedAt: timestamp + index,
        }));
        const normalizedEntries = normalizePlaylistEntries(entries);
        if (!normalizedEntries.length) return null;

        const playlistId = createPlaylistId();
        const fallbackName = `Playlist ${
            playlists.value.filter((item) => !isFavoritesPlaylist(item.id)).length + 1
        }`;
        const derivedName = derivePlaylistNameFromPaths(
            normalizedEntries.map((item) => item.path),
            fallbackName,
        );
        const newPlaylist: Playlist = {
            id: playlistId,
            name: normalizePlaylistName(options.name, derivedName),
            entries: normalizedEntries,
            createdAt: timestamp,
        };

        playlists.value = [...playlists.value, newPlaylist];
        if (options.openInDrawer) {
            activePlaylistId.value = playlistId;
        }
        if (options.setAsPlayback) {
            playbackPlaylistId.value = playlistId;
        }
        return playlistId;
    };

    const getDefaultPlaylistNameForEntries = (
        items: CreatePlaylistEntryInput[],
        fallback?: string,
    ) => {
        const normalizedPaths = items
            .map((item) => item.path?.trim() ?? "")
            .filter(Boolean);
        const fallbackName =
            fallback?.trim() ||
            `Playlist ${
                playlists.value.filter((item) => !isFavoritesPlaylist(item.id)).length +
                1
            }`;
        return derivePlaylistNameFromPaths(normalizedPaths, fallbackName);
    };

    const getDefaultPlaylistNameForPaths = (paths: string[], fallback?: string) =>
        getDefaultPlaylistNameForEntries(
            paths.map((path) => ({ path })),
            fallback,
        );

    const createPlaylistWithPaths = (
        paths: string[],
        options: CreatePlaylistOptions = {},
    ): string | null =>
        createPlaylistWithEntries(
            paths.map((path) => ({ path })),
            options,
        );

    const applyPersistedState = (stored: PersistedPlaylistState) => {
        if (stored.playlists) {
            playlists.value = normalizePlaylists(stored.playlists);
        }
        if (stored.playlistLoopMode) {
            loopMode.value =
                stored.playlistLoopMode === "shuffle" ? "shuffle" : "list";
        }
        if (stored.playlistSortMode && isValidSortMode(stored.playlistSortMode)) {
            sortMode.value = stored.playlistSortMode;
        }

        const storedActivePlaylistId = normalizePlaylistId(stored.activePlaylistId);
        activePlaylistId.value = hasPlaylist(storedActivePlaylistId)
            ? storedActivePlaylistId
            : null;
        syncSelectionAfterMutation();
    };

    const toPersistedState = () => ({
        // Transient playlists (auto-loaded folder, YouTube Up next) are
        // session-only; never persist them.
        playlists: playlists.value.filter(
            (item) =>
                item.id !== AUTOLOAD_PLAYLIST_ID &&
                item.id !== YT_UPNEXT_PLAYLIST_ID,
        ),
        playlistLoopMode: loopMode.value,
        playlistSortMode: sortMode.value,
        activePlaylistId:
            activePlaylistId.value === AUTOLOAD_PLAYLIST_ID ||
            activePlaylistId.value === YT_UPNEXT_PLAYLIST_ID
                ? null
                : activePlaylistId.value,
    });

    /** Replaces the transient YouTube "Up next" queue (empty list removes it). */
    const setYoutubeUpNext = (items: CreatePlaylistEntryInput[]) => {
        const timestamp = Date.now();
        const entries = normalizePlaylistEntries(
            items
                .map((item, index) => ({
                    path: item.path?.trim() ?? "",
                    title: item.title?.trim() || undefined,
                    iconUrl: item.iconUrl?.trim() || undefined,
                    addedAt: timestamp + index,
                }))
                .filter((item) => !!item.path),
        );
        const others = playlists.value.filter(
            (item) => item.id !== YT_UPNEXT_PLAYLIST_ID,
        );
        if (!entries.length) {
            playlists.value = others;
            if (playbackPlaylistId.value === YT_UPNEXT_PLAYLIST_ID) {
                playbackPlaylistId.value = null;
            }
            if (activePlaylistId.value === YT_UPNEXT_PLAYLIST_ID) {
                activePlaylistId.value = null;
            }
            return;
        }
        playlists.value = [
            ...others,
            {
                id: YT_UPNEXT_PLAYLIST_ID,
                name: "Up next",
                entries,
                createdAt: timestamp,
            },
        ];
        playbackPlaylistId.value = YT_UPNEXT_PLAYLIST_ID;
    };

    const addFromDrawerSelection = (paths: string[]) => {
        if (activePlaylist.value) {
            addManyToPlaylist(activePlaylist.value.id, paths);
            return;
        }
        createPlaylistWithPaths(paths, { openInDrawer: true });
    };

    const clearActivePlaylist = () => {
        if (!activePlaylist.value) return;
        playlists.value = playlists.value.map((item) =>
            item.id === activePlaylist.value?.id ? { ...item, entries: [] } : item,
        );
    };

    const removeFromActivePlaylist = (entry: PlaylistEntry) => {
        if (!activePlaylist.value) return;
        playlists.value = playlists.value.map((item) =>
            item.id === activePlaylist.value?.id
                ? {
                      ...item,
                      entries: item.entries.filter(
                          (candidate) => candidate.path !== entry.path,
                      ),
                  }
                : item,
        );
    };

    const renamePlaylist = (playlistId: string, name: string) => {
        if (isFavoritesPlaylist(playlistId)) return;
        const normalizedName = name.trim();
        if (!normalizedName) return;
        playlists.value = playlists.value.map((item) =>
            item.id === playlistId ? { ...item, name: normalizedName } : item,
        );
    };

    const deletePlaylist = (playlistId: string) => {
        if (isFavoritesPlaylist(playlistId)) return;
        if (!hasPlaylist(playlistId)) return;
        playlists.value = playlists.value.filter((item) => item.id !== playlistId);
        syncSelectionAfterMutation();
    };

    const movePlaylist = (fromPlaylistId: string, toPlaylistId: string) => {
        if (isFavoritesPlaylist(fromPlaylistId) || isFavoritesPlaylist(toPlaylistId)) {
            return;
        }
        if (fromPlaylistId === toPlaylistId) return;
        const fromIndex = playlists.value.findIndex(
            (item) => item.id === fromPlaylistId,
        );
        const toIndex = playlists.value.findIndex((item) => item.id === toPlaylistId);
        if (fromIndex < 0 || toIndex < 0) return;
        if (Math.abs(fromIndex - toIndex) !== 1) return;

        const nextPlaylists = [...playlists.value];
        const temp = nextPlaylists[fromIndex];
        nextPlaylists[fromIndex] = nextPlaylists[toIndex];
        nextPlaylists[toIndex] = temp;
        playlists.value = nextPlaylists;
    };

    const enterPlaylist = (playlistId: string) => {
        if (!hasPlaylist(playlistId)) return;
        activePlaylistId.value = playlistId;
    };

    const backToPlaylistList = () => {
        activePlaylistId.value = null;
    };

    const cycleSortMode = () => {
        sortMode.value = sortMode.value === "name" ? "added" : "name";
    };

    const cycleLoopMode = () => {
        loopMode.value = loopMode.value === "list" ? "shuffle" : "list";
    };

    const pickRandomIndex = (length: number, currentIndex: number): number => {
        if (length <= 1) return 0;
        let nextIndex = currentIndex;
        do {
            nextIndex = Math.floor(Math.random() * length);
        } while (nextIndex === currentIndex);
        return nextIndex;
    };

    const resolvePlaybackPlaylistId = (currentPath: string): string | null => {
        const current = currentPath.trim();
        if (!current) return null;

        const playbackPlaylist = findPlaylistById(playbackPlaylistId.value);
        if (playbackPlaylist?.entries.some((entry) => entry.path === current)) {
            return playbackPlaylist.id;
        }

        const active = activePlaylist.value;
        if (active?.entries.some((entry) => entry.path === current)) {
            return active.id;
        }

        const matched = [...playlists.value]
            .reverse()
            .find((item) => item.entries.some((entry) => entry.path === current));
        return matched?.id ?? null;
    };

    const getAdjacentPath = (
        currentPath: string,
        direction: 1 | -1,
    ): string | null => {
        const playlistId = resolvePlaybackPlaylistId(currentPath);
        if (!playlistId) return null;
        playbackPlaylistId.value = playlistId;

        const list = getOrderedEntriesByPlaylistId(playlistId);
        if (!list.length) return null;
        const currentIndex = list.findIndex((item) => item.path === currentPath);

        if (loopMode.value === "shuffle") {
            return list[pickRandomIndex(list.length, currentIndex)]?.path ?? null;
        }

        if (currentIndex < 0) {
            return direction === 1
                ? list[0]?.path ?? null
                : list[list.length - 1]?.path ?? null;
        }

        let nextIndex = currentIndex + direction;
        if (nextIndex < 0) nextIndex = list.length - 1;
        if (nextIndex >= list.length) nextIndex = 0;
        return list[nextIndex]?.path ?? null;
    };

    const getPathForEnd = (currentPath: string): string | null => {
        if (isLoopOne.value) return null;

        const playlistId = resolvePlaybackPlaylistId(currentPath);
        if (!playlistId) return null;
        playbackPlaylistId.value = playlistId;

        const list = getOrderedEntriesByPlaylistId(playlistId);
        if (!list.length) return null;
        const currentIndex = list.findIndex((item) => item.path === currentPath);
        if (currentIndex < 0) return null;

        if (loopMode.value === "shuffle") {
            return list[pickRandomIndex(list.length, currentIndex)]?.path ?? null;
        }

        return list[(currentIndex + 1) % list.length]?.path ?? null;
    };

    const getTitleForPath = (path: string): string | undefined => {
        const normalizedPath = path.trim();
        if (!normalizedPath) return undefined;
        const playlistId = resolvePlaybackPlaylistId(normalizedPath);
        if (!playlistId) return undefined;
        const entry = getOrderedEntriesByPlaylistId(playlistId).find(
            (item) => item.path === normalizedPath,
        );
        return entry?.title?.trim() || undefined;
    };

    const markActivePlaylistAsPlayback = () => {
        if (!activePlaylist.value) return;
        playbackPlaylistId.value = activePlaylist.value.id;
    };

    const removeAutoloadPlaylist = () => {
        if (!playlists.value.some((item) => item.id === AUTOLOAD_PLAYLIST_ID)) {
            return;
        }
        playlists.value = playlists.value.filter(
            (item) => item.id !== AUTOLOAD_PLAYLIST_ID,
        );
        if (activePlaylistId.value === AUTOLOAD_PLAYLIST_ID) {
            activePlaylistId.value = null;
        }
        if (playbackPlaylistId.value === AUTOLOAD_PLAYLIST_ID) {
            playbackPlaylistId.value = null;
        }
    };

    const clearAutoloadFolder = () => {
        removeAutoloadPlaylist();
    };

    // Point playback at the folder (so prev/next walk it) and show it in the
    // drawer by default — but NEVER override a playlist the user has opened in
    // the drawer (e.g. Favourites), which would otherwise be undone every time
    // the next file auto-loads.
    const activateAutoloadPlaylist = () => {
        if (
            activePlaylistId.value === null ||
            activePlaylistId.value === AUTOLOAD_PLAYLIST_ID
        ) {
            activePlaylistId.value = AUTOLOAD_PLAYLIST_ID;
        }
        playbackPlaylistId.value = AUTOLOAD_PLAYLIST_ID;
    };

    // Populate the transient folder playlist with the media files that live
    // alongside the currently playing file, so prev/next walk the folder and
    // the files are visible in the drawer. `mediaPaths` must be in folder order
    // and include `currentPath` exactly (so navigation string-matches media.url).
    const syncAutoloadFolder = (currentPath: string, mediaPaths: string[]) => {
        const current = currentPath.trim();
        if (!current) return;

        // Don't hijack a user playlist the file is actively being played from.
        const playbackId = playbackPlaylistId.value;
        if (playbackId && playbackId !== AUTOLOAD_PLAYLIST_ID) {
            const activePlayback = findPlaylistById(playbackId);
            if (activePlayback?.entries.some((entry) => entry.path === current)) {
                return;
            }
        }

        const paths = mediaPaths.map((path) => path.trim()).filter(Boolean);
        if (paths.length <= 1 || !paths.includes(current)) {
            removeAutoloadPlaylist();
            return;
        }

        // Idempotent: same folder already loaded — just ensure it's active.
        const existing = findPlaylistById(AUTOLOAD_PLAYLIST_ID);
        if (
            existing &&
            existing.entries.length === paths.length &&
            paths.every((path) =>
                existing.entries.some((entry) => entry.path === path),
            )
        ) {
            activateAutoloadPlaylist();
            return;
        }

        // addedAt decreases with folder index so the default "added" (desc)
        // sort still shows the folder in natural order.
        const base = Date.now();
        const entries: PlaylistEntry[] = paths.map((path, index) => ({
            path,
            addedAt: base - index,
        }));
        const autoloadPlaylist: Playlist = {
            id: AUTOLOAD_PLAYLIST_ID,
            name: derivePlaylistNameFromPaths(paths, "Current Folder"),
            entries,
            createdAt: base,
        };
        playlists.value = [
            ...playlists.value.filter((item) => item.id !== AUTOLOAD_PLAYLIST_ID),
            autoloadPlaylist,
        ];
        activateAutoloadPlaylist();
    };

    const toggleLoopOne = async (
        setLoopFile: (enabled: boolean) => Promise<void>,
    ) => {
        isLoopOne.value = !isLoopOne.value;
        await setLoopFile(isLoopOne.value);
    };

    const togglePlaylistLoop = async (
        setLoopFile: (enabled: boolean) => Promise<void>,
    ) => {
        if (isLoopOne.value) {
            isLoopOne.value = false;
            await setLoopFile(false);
        }
        cycleLoopMode();
    };

    return {
        playlists,
        activePlaylistId,
        activePlaylist,
        playlist,
        loopMode,
        sortMode,
        isLoopOne,
        orderedPlaylist,
        applyPersistedState,
        toPersistedState,
        createPlaylistWithPaths,
        createPlaylistWithEntries,
        setYoutubeUpNext,
        getDefaultPlaylistNameForPaths,
        getDefaultPlaylistNameForEntries,
        addFromDrawerSelection,
        addToFavorites,
        clearActivePlaylist,
        removeFromActivePlaylist,
        renamePlaylist,
        deletePlaylist,
        movePlaylist,
        enterPlaylist,
        backToPlaylistList,
        markActivePlaylistAsPlayback,
        favorites,
        isFavorite,
        toggleFavorite,
        removeFromFavorites,
        clearFavorites,
        openFavoritesView,
        // Favourite folders
        favoriteFolderList,
        favoritesByFolder,
        favoriteFolderCounts,
        activeFavoriteFolderId,
        folderIdForPath,
        createFavoriteFolder,
        renameFavoriteFolder,
        deleteFavoriteFolder,
        moveFavoriteToFolder,
        setActiveFavoriteFolder,
        exportFavoritesData,
        importFavoritesData,
        cycleSortMode,
        getAdjacentPath,
        getPathForEnd,
        getTitleForPath,
        toggleLoopOne,
        togglePlaylistLoop,
        syncAutoloadFolder,
        clearAutoloadFolder,
    };
};
