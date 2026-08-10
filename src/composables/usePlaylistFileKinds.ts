import { readonly, ref } from "vue";
import { AV_FILE_EXTENSIONS, MEDIA_FILE_EXTENSIONS } from "../constants/media";
import {
    PLAYLIST_INCLUDE_IMAGES_SETTING_LABEL,
    SETTINGS_UPDATED_EVENT,
} from "../mock/settings";
import { loadUiState } from "./useUiStateStore";

// One answer to "does this file belong in a playlist?", shared by every route a
// file can take into one: the open dialog, drag and drop, and the folder
// auto-load scan. mpv can display images, but a folder of films usually has
// cover art beside it, and those posters showing up as playlist entries is
// noise. Images are excluded unless the setting asks for them.

type StoredSettingGroup = {
    title: string;
    items: Array<{ label: string; value: string }>;
};

const includeImages = ref(false);

const toSet = (extensions: string[]) =>
    new Set(extensions.map((extension) => extension.toLowerCase()));

const WITH_IMAGES = toSet(MEDIA_FILE_EXTENSIONS);
const WITHOUT_IMAGES = toSet(AV_FILE_EXTENSIONS);

const parseIncludeImages = (groups?: StoredSettingGroup[]): boolean =>
    groups
        ?.flatMap((group) => group.items)
        .find((item) => item.label === PLAYLIST_INCLUDE_IMAGES_SETTING_LABEL)
        ?.value === "On";

/** Lower-cased extension without the dot, or "" when there isn't one. */
export const fileExtensionOf = (path: string): string => {
    const cleanPath = path.trim().split(/[?#]/, 1)[0] ?? "";
    const dotIndex = cleanPath.lastIndexOf(".");
    const separatorIndex = Math.max(
        cleanPath.lastIndexOf("/"),
        cleanPath.lastIndexOf("\\"),
    );
    if (dotIndex <= separatorIndex) return "";
    return cleanPath.slice(dotIndex + 1).toLowerCase();
};

/** The extensions a playlist currently accepts, for file-dialog filters. */
export const playlistFileExtensions = (): string[] =>
    includeImages.value ? [...MEDIA_FILE_EXTENSIONS] : [...AV_FILE_EXTENSIONS];

export const isPlaylistFilePath = (path: string): boolean => {
    const extension = fileExtensionOf(path);
    if (!extension) return false;
    return (includeImages.value ? WITH_IMAGES : WITHOUT_IMAGES).has(extension);
};

export const includeImagesInPlaylist = readonly(includeImages);

const applyStoredGroups = (groups?: StoredSettingGroup[]) => {
    includeImages.value = parseIncludeImages(groups);
};

const onSettingsUpdated = (event: Event) => {
    const customEvent = event as CustomEvent<{ groups?: StoredSettingGroup[] }>;
    applyStoredGroups(customEvent.detail?.groups);
};

// Module scope rather than a component hook: the consumers are plain functions
// called from drop handlers and scans, not from a setup(). Off is the safe
// default, so reading the stored value late only ever adds files.
if (typeof window !== "undefined") {
    window.addEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    void loadUiState<{ settings?: { groups?: StoredSettingGroup[] } }>().then(
        (stored) => applyStoredGroups(stored?.settings?.groups),
    );
}
