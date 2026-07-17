import { onMounted, onUnmounted, ref, watch } from "vue";
import {
    AUTOLOAD_FOLDER_SETTING_LABEL,
    SETTINGS_UPDATED_EVENT,
} from "../mock/settings";
import { loadUiState } from "./useUiStateStore";
import {
    isMediaFilePath,
    listLocalSiblingFiles,
    normalizeLocalPathForCompare,
} from "../utils/localMediaSiblings";

// Port of the mpv `autoload.lua` script: when a local file starts playing,
// scan its folder for sibling media files and load them into the playlist so
// prev/next walk the folder sequence. Toggleable via the Auto-Load Folder
// setting.

type StoredSettingGroup = {
    title: string;
    items: Array<{ label: string; value: string }>;
};

type AutoloadPlaylistApi = {
    syncAutoloadFolder: (currentPath: string, mediaPaths: string[]) => void;
    clearAutoloadFolder: () => void;
};

type AutoloadFolderOptions = {
    playlist: AutoloadPlaylistApi;
};

const parseAutoloadEnabled = (groups?: StoredSettingGroup[]): boolean =>
    groups
        ?.flatMap((group) => group.items)
        .find((item) => item.label === AUTOLOAD_FOLDER_SETTING_LABEL)?.value ===
    "On";

// Remote/stream sources have no local folder to scan.
const isLocalMediaSource = (path: string): boolean => {
    const trimmed = path.trim();
    if (!trimmed) return false;
    if (/^(https?|rtmp|rtsp|mms|udp|smb|ftp|webdav|dlna|soia-):/i.test(trimmed)) {
        return false;
    }
    return true;
};

const compareKey = (path: string): string =>
    normalizeLocalPathForCompare(path).replace(/\\/g, "/").toLowerCase();

export const useAutoloadFolder = ({ playlist }: AutoloadFolderOptions) => {
    const enabled = ref(false);

    const applyStoredGroups = (groups?: StoredSettingGroup[]) => {
        enabled.value = parseAutoloadEnabled(groups);
    };

    const refreshFromSettings = async () => {
        const stored = await loadUiState<{
            settings?: { groups?: StoredSettingGroup[] };
        }>();
        applyStoredGroups(stored?.settings?.groups);
    };

    const onSettingsUpdated = (event: Event) => {
        const customEvent = event as CustomEvent<{
            groups?: StoredSettingGroup[];
        }>;
        applyStoredGroups(customEvent.detail?.groups);
    };

    const onFileLoaded = async (currentPath: string) => {
        if (!enabled.value) return;
        if (!isLocalMediaSource(currentPath)) return;

        const siblings = await listLocalSiblingFiles(currentPath);
        const mediaPaths = siblings.filter(isMediaFilePath);
        if (mediaPaths.length <= 1) return;

        // Ensure the current file appears in the list with the exact string used
        // for media.url, so playlist navigation (which string-matches) works even
        // when media.url differs in form (e.g. file:// URL) from the scan output.
        const currentKey = compareKey(currentPath);
        const index = mediaPaths.findIndex(
            (path) => compareKey(path) === currentKey,
        );
        if (index < 0) return;
        const paths = mediaPaths.slice();
        paths[index] = currentPath;

        playlist.syncAutoloadFolder(currentPath, paths);
    };

    watch(enabled, (isEnabled, wasEnabled) => {
        if (isEnabled === wasEnabled) return;
        if (!isEnabled) {
            playlist.clearAutoloadFolder();
        }
    });

    onMounted(() => {
        void refreshFromSettings();
        window.addEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    onUnmounted(() => {
        window.removeEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    return {
        enabled,
        onFileLoaded,
    };
};
