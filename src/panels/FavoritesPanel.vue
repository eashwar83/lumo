<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
    DEFAULT_FAVORITE_FOLDER_ID,
    type FavoriteFolder,
    type PlaylistEntry,
} from "../types/playlist";
import { getPathDisplayName } from "../utils/getPathDisplayName";
import { readImageDataUrl } from "../utils/readImageDataUrl";

const props = defineProps<{
    favorites: PlaylistEntry[];
    folders: FavoriteFolder[];
    favoritesByFolder: Record<string, PlaylistEntry[]>;
    folderCounts: Record<string, number>;
    activeFolderId: string | null;
}>();

const emit = defineEmits<{
    (e: "play", entry: PlaylistEntry): void;
    (e: "remove", entry: PlaylistEntry): void;
    (e: "clear"): void;
    (e: "select-folder", id: string | null): void;
    (e: "create-folder", name: string): void;
    (e: "rename-folder", payload: { id: string; name: string }): void;
    (e: "delete-folder", id: string): void;
    (e: "move-to-folder", payload: { path: string; folderId: string }): void;
    (e: "move-many-to-folder", payload: { paths: string[]; folderId: string }): void;
    (e: "remove-many", paths: string[]): void;
    (e: "export"): void;
    (e: "import"): void;
}>();

// Entries shown in the grid: the selected folder, or all favourites for "All".
const displayedFavorites = computed<PlaylistEntry[]>(() =>
    props.activeFolderId
        ? props.favoritesByFolder[props.activeFolderId] ?? []
        : props.favorites,
);

// path -> the folder it currently belongs to (for highlighting in the move menu).
const pathFolder = computed<Record<string, string>>(() => {
    const map: Record<string, string> = {};
    for (const [fid, list] of Object.entries(props.favoritesByFolder)) {
        for (const entry of list) map[entry.path] = fid;
    }
    return map;
});
const currentFolderOf = (path: string) =>
    pathFolder.value[path] ?? DEFAULT_FAVORITE_FOLDER_ID;
const folderName = (id: string) =>
    props.folders.find((folder) => folder.id === id)?.name ?? "";

// --- folder create / rename UI state ---
const creatingFolder = ref(false);
const newFolderName = ref("");
const newFolderInput = ref<HTMLInputElement | null>(null);
const editingFolderId = ref<string | null>(null);
const editFolderName = ref("");

// Per-card "move to folder" menu — teleported to <body> and positioned with
// fixed coordinates so it is never clipped by the card / grid overflow.
const moveMenuPath = ref<string | null>(null);
const moveMenuStyle = ref<{ left: string; top: string }>({
    left: "0px",
    top: "0px",
});

// Multi-select for bulk move / remove.
const selectedPaths = ref<string[]>([]);
const isSelected = (path: string) => selectedPaths.value.includes(path);
const toggleSelect = (path: string) => {
    selectedPaths.value = isSelected(path)
        ? selectedPaths.value.filter((p) => p !== path)
        : [...selectedPaths.value, path];
};
const clearSelection = () => {
    selectedPaths.value = [];
};
const selectAllVisible = () => {
    selectedPaths.value = displayedFavorites.value.map((entry) => entry.path);
};
// Switching folders clears a stale selection.
watch(() => props.activeFolderId, clearSelection);

const bulkMoveTo = (folderId: string) => {
    if (!folderId || !selectedPaths.value.length) return;
    emit("move-many-to-folder", {
        paths: [...selectedPaths.value],
        folderId,
    });
    clearSelection();
};
const bulkRemove = () => {
    if (!selectedPaths.value.length) return;
    emit("remove-many", [...selectedPaths.value]);
    clearSelection();
};

const startCreateFolder = () => {
    creatingFolder.value = true;
    newFolderName.value = "";
    void nextTick(() => newFolderInput.value?.focus());
};
const confirmCreateFolder = () => {
    const name = newFolderName.value.trim();
    if (name) emit("create-folder", name);
    creatingFolder.value = false;
    newFolderName.value = "";
};
const cancelCreateFolder = () => {
    creatingFolder.value = false;
    newFolderName.value = "";
};

const startRenameFolder = (folder: FavoriteFolder) => {
    editingFolderId.value = folder.id;
    editFolderName.value = folder.name;
};
const confirmRenameFolder = () => {
    const id = editingFolderId.value;
    if (id) {
        const name = editFolderName.value.trim();
        if (name) emit("rename-folder", { id, name });
    }
    editingFolderId.value = null;
};

const onDeleteFolder = (id: string) => emit("delete-folder", id);

const MOVE_MENU_WIDTH = 172;
const MOVE_MENU_MAX_HEIGHT = 240;
const openMoveMenu = (path: string, event: MouseEvent) => {
    if (moveMenuPath.value === path) {
        moveMenuPath.value = null;
        return;
    }
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const left = Math.max(
        8,
        Math.min(rect.left, window.innerWidth - MOVE_MENU_WIDTH - 8),
    );
    const top = Math.max(
        8,
        Math.min(rect.bottom + 4, window.innerHeight - MOVE_MENU_MAX_HEIGHT - 8),
    );
    moveMenuStyle.value = { left: `${left}px`, top: `${top}px` };
    moveMenuPath.value = path;
};
const closeMoveMenu = () => {
    moveMenuPath.value = null;
};
const moveTo = (path: string, folderId: string) => {
    emit("move-to-folder", { path, folderId });
    moveMenuPath.value = null;
};
// Close the teleported menu on any outside click (the menu itself stops
// propagation) and on scroll.
watch(moveMenuPath, (path) => {
    if (path) {
        window.addEventListener("click", closeMoveMenu);
        window.addEventListener("scroll", closeMoveMenu, true);
    } else {
        window.removeEventListener("click", closeMoveMenu);
        window.removeEventListener("scroll", closeMoveMenu, true);
    }
});

// path -> resolved <img> src (remote URL as-is, or data URL for local files)
const thumbs = ref<Record<string, string>>({});

const isRemote = (url?: string): boolean => !!url && /^https?:\/\//i.test(url);

const isRemotePath = (path: string): boolean =>
    /^(https?|rtsp|rtmp|smb|webdav):\/\//i.test(path);

// Entries favourited without ever playing (or added from the right-click menu)
// carry no artwork, so a poster is extracted on demand and cached on disk.
// Extraction spins up a headless mpv per file, so they run one at a time —
// firing forty at once would stall the machine.
const pending: string[] = [];
const requested = new Set<string>();
let draining = false;
let disposed = false;

const drain = async () => {
    if (draining) return;
    draining = true;
    while (pending.length && !disposed) {
        const path = pending.shift();
        if (!path || thumbs.value[path]) continue;
        try {
            const dataUrl = await invoke<string | null>("get_media_poster", { path });
            if (dataUrl && !disposed) {
                thumbs.value = { ...thumbs.value, [path]: dataUrl };
            }
        } catch {
            // Missing file or an unreadable codec — keep the placeholder.
        }
    }
    draining = false;
};

const queuePoster = (path: string) => {
    if (!path || requested.has(path) || thumbs.value[path]) return;
    if (isRemotePath(path)) return;
    requested.add(path);
    pending.push(path);
    void drain();
};

const resolveThumb = async (entry: PlaylistEntry) => {
    if (thumbs.value[entry.path]) return;
    const icon = entry.iconUrl?.trim();
    if (!icon) {
        queuePoster(entry.path);
        return;
    }
    if (isRemote(icon)) {
        thumbs.value = { ...thumbs.value, [entry.path]: icon };
        return;
    }
    const dataUrl = await readImageDataUrl(icon);
    if (dataUrl) {
        thumbs.value = { ...thumbs.value, [entry.path]: dataUrl };
        return;
    }
    // The stored artwork file has gone (cache cleared, or it never landed).
    queuePoster(entry.path);
};

watch(
    () => props.favorites,
    (entries) => {
        entries.forEach((entry) => void resolveThumb(entry));
    },
    { immediate: true, deep: true },
);

onBeforeUnmount(() => {
    disposed = true;
    pending.length = 0;
    window.removeEventListener("click", closeMoveMenu);
    window.removeEventListener("scroll", closeMoveMenu, true);
});

const displayName = (entry: PlaylistEntry): string =>
    entry.title?.trim() || getPathDisplayName(entry.path);

const thumbFor = (entry: PlaylistEntry): string | null =>
    thumbs.value[entry.path] ?? null;
</script>

<template>
    <div class="favorites panel panel--favorites">
        <div class="panel__header">
            <div class="panel__title">
                Favourites
                <span v-if="props.favorites.length" class="favorites__count">
                    {{ props.favorites.length }}
                </span>
            </div>
            <div class="favorites__header-actions">
                <button
                    class="favorites__hbtn"
                    type="button"
                    title="Import a favourites file"
                    @click.stop="emit('import')"
                >
                    Import
                </button>
                <button
                    v-if="props.favorites.length"
                    class="favorites__hbtn"
                    type="button"
                    title="Export favourites to a file"
                    @click.stop="emit('export')"
                >
                    Export
                </button>
                <button
                    v-if="props.favorites.length"
                    class="panel__reset"
                    type="button"
                    @click.stop="emit('clear')"
                >
                    Clear
                </button>
            </div>
        </div>

        <!-- Folder bar -->
        <div v-if="props.favorites.length || props.folders.length > 1" class="favorites__folders">
            <button
                class="favorites__folder-chip"
                :class="{ 'favorites__folder-chip--active': props.activeFolderId === null }"
                type="button"
                @click="emit('select-folder', null)"
            >
                All
                <span class="favorites__folder-count">{{ props.favorites.length }}</span>
            </button>

            <template v-for="folder in props.folders" :key="folder.id">
                <div
                    v-if="editingFolderId === folder.id"
                    class="favorites__folder-edit"
                >
                    <input
                        v-model="editFolderName"
                        class="favorites__folder-input"
                        type="text"
                        @keydown.enter.prevent="confirmRenameFolder"
                        @keydown.esc.prevent="editingFolderId = null"
                        @blur="confirmRenameFolder"
                    />
                </div>
                <div
                    v-else
                    class="favorites__folder-chip favorites__folder-chip--folder"
                    :class="{ 'favorites__folder-chip--active': props.activeFolderId === folder.id }"
                    role="button"
                    tabindex="0"
                    @click="emit('select-folder', folder.id)"
                    @keydown.enter="emit('select-folder', folder.id)"
                    @dblclick.stop="startRenameFolder(folder)"
                >
                    {{ folder.name }}
                    <span class="favorites__folder-count">
                        {{ props.folderCounts[folder.id] ?? 0 }}
                    </span>
                    <button
                        class="favorites__folder-edit-btn"
                        type="button"
                        title="Rename folder"
                        @click.stop="startRenameFolder(folder)"
                    >
                        ✎
                    </button>
                    <button
                        v-if="folder.id !== DEFAULT_FAVORITE_FOLDER_ID"
                        class="favorites__folder-edit-btn favorites__folder-edit-btn--danger"
                        type="button"
                        title="Delete folder (videos move back to General)"
                        @click.stop="onDeleteFolder(folder.id)"
                    >
                        ×
                    </button>
                </div>
            </template>

            <div v-if="creatingFolder" class="favorites__folder-edit">
                <input
                    ref="newFolderInput"
                    v-model="newFolderName"
                    class="favorites__folder-input"
                    type="text"
                    placeholder="Folder name"
                    @keydown.enter.prevent="confirmCreateFolder"
                    @keydown.esc.prevent="cancelCreateFolder"
                    @blur="confirmCreateFolder"
                />
            </div>
            <button
                v-else
                class="favorites__folder-chip favorites__folder-chip--new"
                type="button"
                title="New folder"
                @click="startCreateFolder"
            >
                + New
            </button>
        </div>

        <!-- Bulk selection bar -->
        <div v-if="selectedPaths.length" class="favorites__bulk">
            <span class="favorites__bulk-count">
                {{ selectedPaths.length }} selected
            </span>
            <button
                class="favorites__bulk-btn"
                type="button"
                @click="selectAllVisible"
            >
                Select all
            </button>
            <label class="favorites__bulk-move">
                <span>Move to</span>
                <select
                    class="favorites__bulk-select"
                    @change="
                        bulkMoveTo(($event.target as HTMLSelectElement).value);
                        ($event.target as HTMLSelectElement).value = '';
                    "
                >
                    <option value="" selected>Choose folder…</option>
                    <option
                        v-for="folder in props.folders"
                        :key="folder.id"
                        :value="folder.id"
                    >
                        {{ folder.name }}
                    </option>
                </select>
            </label>
            <button
                class="favorites__bulk-btn favorites__bulk-btn--danger"
                type="button"
                @click="bulkRemove"
            >
                Remove
            </button>
            <button
                class="favorites__bulk-btn"
                type="button"
                @click="clearSelection"
            >
                Clear
            </button>
        </div>

        <div class="favorites__content">
            <div v-if="!props.favorites.length" class="panel__empty">
                <svg
                    class="favorites__empty-icon"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.6"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    aria-hidden="true"
                >
                    <path
                        d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"
                    />
                </svg>
                <div class="panel__empty-title">No favourites yet</div>
                <div class="panel__empty-body">
                    Tap the heart while watching a video to add it here.
                </div>
            </div>

            <div v-else-if="!displayedFavorites.length" class="panel__empty">
                <div class="panel__empty-title">This folder is empty</div>
                <div class="panel__empty-body">
                    Move favourites here from another folder, or add new ones
                    while this folder is open.
                </div>
            </div>

            <div v-else class="favorites__grid">
                <div
                    v-for="entry in displayedFavorites"
                    :key="entry.path"
                    class="favorites__card"
                    :class="{
                        'favorites__card--selected': isSelected(entry.path),
                    }"
                    role="button"
                    tabindex="0"
                    :title="displayName(entry)"
                    @click="emit('play', entry)"
                    @keydown.enter="emit('play', entry)"
                    @keydown.space.prevent="emit('play', entry)"
                >
                    <div class="favorites__thumb">
                        <img
                            v-if="thumbFor(entry)"
                            class="favorites__thumb-img"
                            :src="thumbFor(entry) ?? ''"
                            :alt="displayName(entry)"
                            loading="lazy"
                            draggable="false"
                        />
                        <div v-else class="favorites__thumb-placeholder">
                            <svg
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1.5"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <path d="M8 5v14l11-7z" fill="currentColor" stroke="none" />
                            </svg>
                        </div>
                        <div class="favorites__play-overlay">
                            <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                                <path d="M8 5v14l11-7z" />
                            </svg>
                        </div>
                        <button
                            class="favorites__select"
                            :class="{
                                'favorites__select--on': isSelected(entry.path),
                            }"
                            type="button"
                            :aria-label="isSelected(entry.path) ? 'Deselect' : 'Select'"
                            title="Select"
                            @click.stop="toggleSelect(entry.path)"
                            @keydown.enter.stop
                            @keydown.space.prevent.stop
                        >
                            <svg
                                v-if="isSelected(entry.path)"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="3"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <path d="M20 6 9 17l-5-5" />
                            </svg>
                        </button>
                        <button
                            class="favorites__remove"
                            type="button"
                            aria-label="Remove from favourites"
                            title="Remove from favourites"
                            @click.stop="emit('remove', entry)"
                            @keydown.enter.stop
                            @keydown.space.prevent.stop
                        >
                            <svg
                                viewBox="0 0 24 24"
                                fill="currentColor"
                                aria-hidden="true"
                            >
                                <path
                                    d="M20.84 4.61a5.5 5.5 0 0 0-7.78 0L12 5.67l-1.06-1.06a5.5 5.5 0 0 0-7.78 7.78l1.06 1.06L12 21.23l7.78-7.78 1.06-1.06a5.5 5.5 0 0 0 0-7.78z"
                                />
                            </svg>
                        </button>
                        <button
                            class="favorites__move"
                            type="button"
                            aria-label="Move to folder"
                            :title="`In: ${folderName(currentFolderOf(entry.path))} — move to folder`"
                            @click.stop="openMoveMenu(entry.path, $event)"
                            @keydown.enter.stop
                            @keydown.space.prevent.stop
                        >
                            <svg
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1.8"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <path
                                    d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"
                                />
                            </svg>
                        </button>
                    </div>
                    <div class="favorites__name">{{ displayName(entry) }}</div>
                </div>
            </div>
        </div>

        <!-- Move-to-folder menu (teleported so it is never clipped) -->
        <Teleport to="body">
            <div
                v-if="moveMenuPath"
                class="favorites__move-menu"
                :style="moveMenuStyle"
                @click.stop
            >
                <div class="favorites__move-title">Move to</div>
                <button
                    v-for="folder in props.folders"
                    :key="folder.id"
                    class="favorites__move-item"
                    :class="{
                        'favorites__move-item--current':
                            currentFolderOf(moveMenuPath) === folder.id,
                    }"
                    type="button"
                    @click.stop="moveTo(moveMenuPath, folder.id)"
                >
                    {{ folder.name }}
                </button>
            </div>
        </Teleport>
    </div>
</template>

<style scoped src="../styles/panels.css"></style>

<style scoped>
.favorites {
    display: flex;
    flex-direction: column;
    gap: 12px;
    pointer-events: auto;
    cursor: default;
    user-select: none;
    -webkit-user-select: none;
    overflow: hidden;
}

.favorites__count {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    margin-left: 8px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.1);
    font-size: 11px;
    font-weight: 600;
    vertical-align: middle;
}

.favorites__header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
}

.favorites__hbtn {
    padding: 5px 12px;
    border: 1px solid var(--glass-border, rgba(0, 0, 0, 0.12));
    border-radius: 7px;
    background: rgba(0, 0, 0, 0.04);
    color: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}
.favorites__hbtn:hover {
    background: rgba(0, 0, 0, 0.08);
}

/* --- folder bar --- */
.favorites__folders {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 0 6px 2px;
}

.favorites__folder-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border: 1px solid var(--glass-border, rgba(0, 0, 0, 0.12));
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.04);
    color: inherit;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
}
.favorites__folder-chip:hover {
    background: rgba(0, 0, 0, 0.08);
}
.favorites__folder-chip--active {
    background: rgba(224, 86, 138, 0.16);
    border-color: rgba(224, 86, 138, 0.5);
    color: #e0568a;
}
.favorites__folder-chip--new {
    border-style: dashed;
    opacity: 0.85;
}

.favorites__folder-count {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.1);
    font-size: 10.5px;
    font-weight: 700;
}

.favorites__folder-edit-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: transparent;
    color: inherit;
    font-size: 12px;
    line-height: 1;
    opacity: 0.55;
    cursor: pointer;
}
.favorites__folder-edit-btn:hover {
    opacity: 1;
    background: rgba(0, 0, 0, 0.1);
}
.favorites__folder-edit-btn--danger:hover {
    background: rgba(220, 60, 60, 0.85);
    color: #fff;
}

.favorites__folder-input {
    padding: 6px 10px;
    border: 1px solid rgba(224, 86, 138, 0.5);
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.04);
    color: inherit;
    font-size: 12.5px;
    width: 130px;
}

.favorites__content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
}

/* --- bulk selection bar --- */
.favorites__bulk {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    margin: 0 6px;
    border-radius: 10px;
    background: rgba(224, 86, 138, 0.12);
    border: 1px solid rgba(224, 86, 138, 0.35);
}
.favorites__bulk-count {
    font-size: 12.5px;
    font-weight: 700;
    color: #e0568a;
}
.favorites__bulk-btn {
    padding: 5px 12px;
    border: 1px solid var(--glass-border, rgba(0, 0, 0, 0.12));
    border-radius: 7px;
    background: rgba(0, 0, 0, 0.04);
    color: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}
.favorites__bulk-btn:hover {
    background: rgba(0, 0, 0, 0.08);
}
.favorites__bulk-btn--danger {
    border-color: rgba(220, 70, 70, 0.5);
    color: #d64646;
}
.favorites__bulk-btn--danger:hover {
    background: rgba(220, 70, 70, 0.14);
}
.favorites__bulk-move {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
}
.favorites__bulk-select {
    padding: 5px 8px;
    border: 1px solid var(--glass-border, rgba(0, 0, 0, 0.12));
    border-radius: 7px;
    background: rgba(0, 0, 0, 0.04);
    color: inherit;
    font-size: 12px;
    color-scheme: dark;
}
.favorites__bulk-select option {
    background-color: #1a1c21;
    color: #f2f2f2;
}

/* --- selection checkbox on cards --- */
.favorites__select {
    position: absolute;
    top: 6px;
    left: 6px;
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 2px solid rgba(255, 255, 255, 0.85);
    border-radius: 7px;
    background: rgba(0, 0, 0, 0.4);
    color: #fff;
    opacity: 0;
    cursor: pointer;
    transition: opacity 0.12s ease;
}
.favorites__select svg {
    width: 15px;
    height: 15px;
}
.favorites__card:hover .favorites__select {
    opacity: 1;
}
.favorites__select--on {
    opacity: 1;
    background: #e0568a;
    border-color: #e0568a;
}
.favorites__card--selected {
    outline: 2px solid #e0568a;
    outline-offset: 2px;
    border-radius: 12px;
}

/* --- move-to-folder button + menu --- */
.favorites__move {
    position: absolute;
    bottom: 6px;
    left: 6px;
    width: 26px;
    height: 26px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: none;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.55);
    color: #fff;
    opacity: 0;
    cursor: pointer;
    transition: opacity 0.12s ease;
}
.favorites__move svg {
    width: 15px;
    height: 15px;
}
.favorites__card:hover .favorites__move {
    opacity: 1;
}
.favorites__move:hover {
    background: rgba(224, 86, 138, 0.9);
}

.favorites__move-menu {
    position: fixed;
    z-index: 400;
    min-width: 172px;
    max-height: 240px;
    overflow-y: auto;
    padding: 6px;
    border-radius: 10px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.5);
}
.favorites__move-title {
    font-size: 10.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.45);
    padding: 2px 8px 6px;
}
.favorites__move-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 7px 8px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: #fff;
    font-size: 12.5px;
    cursor: pointer;
}
.favorites__move-item:hover {
    background: rgba(255, 255, 255, 0.1);
}
.favorites__move-item--current {
    color: #e0568a;
    font-weight: 700;
}

.favorites__empty-icon {
    width: 40px;
    height: 40px;
    margin: 0 auto 10px;
    color: #e0568a;
    opacity: 0.85;
}

.favorites__grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 14px;
    padding: 6px 6px 12px;
    scrollbar-width: thin;
}

.favorites__card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    border-radius: 12px;
    cursor: pointer;
    transition: transform 0.12s ease;
}

.favorites__card:hover {
    transform: translateY(-2px);
}

.favorites__card:focus-visible {
    outline: 2px solid rgba(57, 108, 216, 0.6);
    outline-offset: 3px;
    border-radius: 12px;
}

.favorites__thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    border-radius: 10px;
    overflow: hidden;
    background: rgba(0, 0, 0, 0.14);
    border: 1px solid rgba(0, 0, 0, 0.1);
}

.favorites__thumb-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
}

.favorites__thumb-placeholder {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: rgba(0, 0, 0, 0.3);
}

.favorites__thumb-placeholder svg {
    width: 34px;
    height: 34px;
}

.favorites__play-overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.34);
    opacity: 0;
    transition: opacity 0.15s ease;
    color: #fff;
}

.favorites__play-overlay svg {
    width: 40px;
    height: 40px;
    filter: drop-shadow(0 2px 6px rgba(0, 0, 0, 0.5));
}

.favorites__card:hover .favorites__play-overlay,
.favorites__card:focus-visible .favorites__play-overlay {
    opacity: 1;
}

.favorites__remove {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 30px;
    height: 30px;
    border: none;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.5);
    color: #ff5b8a;
    display: grid;
    place-items: center;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.15s ease, background-color 0.15s ease,
        transform 0.12s ease;
}

.favorites__remove svg {
    width: 17px;
    height: 17px;
}

.favorites__card:hover .favorites__remove,
.favorites__remove:focus-visible {
    opacity: 1;
}

.favorites__remove:hover {
    background: rgba(0, 0, 0, 0.72);
    transform: scale(1.05);
}

.favorites__name {
    font-size: 12.5px;
    line-height: 1.35;
    color: var(--text-color);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    padding: 0 2px;
}

:root[data-theme="light"] .favorites__count {
    background: rgba(0, 0, 0, 0.08);
}

@media (prefers-color-scheme: dark) {
    :root:not([data-theme]) .favorites__count {
        background: rgba(255, 255, 255, 0.14);
    }
    :root:not([data-theme]) .favorites__thumb {
        background: rgba(255, 255, 255, 0.06);
        border-color: rgba(255, 255, 255, 0.1);
    }
    :root:not([data-theme]) .favorites__thumb-placeholder {
        color: rgba(255, 255, 255, 0.32);
    }
}

:root:is([data-theme="dark"], [data-theme="graphite"]) .favorites__count {
    background: rgba(255, 255, 255, 0.14);
}

:root:is([data-theme="dark"], [data-theme="graphite"]) .favorites__thumb {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.1);
}

:root:is([data-theme="dark"], [data-theme="graphite"])
    .favorites__thumb-placeholder {
    color: rgba(255, 255, 255, 0.32);
}
</style>
