import { computed, ref } from "vue";

export type ContextMenuItem = {
    id: string;
    label: string;
    icon?:
        | "heart"
        | "settings"
        | "subtitle"
        | "subtitle-advanced-settings"
        | "subtitle-search"
        | "thumbnails"
        | "contact-sheet"
        | "menu-bar";
    disabled?: boolean;
    children?: ContextMenuItem[];
};

type UsePlaybackContextMenuOptions = {
    isFileLoaded: () => boolean;
    getCurrentPath: () => string;
    getCurrentTitle: () => string;
    addToFavorites: (item: { path: string; title?: string }) => void;
    isFavorite: () => boolean;
    searchOnlineSubtitles: (path: string, title?: string) => void | Promise<void>;
    openSubtitleAdvancedSettings: () => void;
    openSettings: () => void | Promise<void>;
    regenerateThumbnails: () => void | Promise<void>;
    exportContactSheet: () => void | Promise<void>;
    isMenuBarVisible: () => boolean;
    toggleMenuBar: () => void;
    hideAllMenus: () => void;
};

const ADD_TO_FAVORITES_ID = "add-to-favorites";
const SUBTITLE_ID = "subtitle";
const SUBTITLE_FIND_ONLINE_ID = "subtitle/find-online-subtitle";
const SUBTITLE_ADVANCED_SETTINGS_ID = "subtitle/advanced-settings";
const REGENERATE_THUMBS_ID = "regenerate-thumbnails";
const CONTACT_SHEET_ID = "export-contact-sheet";
const TOGGLE_MENU_BAR_ID = "toggle-menu-bar";
const OPEN_SETTINGS_ID = "open-settings";

const isLocalMediaPath = (path: string): boolean =>
    !!path && !/^(https?|rtsp|rtmp|smb|webdav):\/\//i.test(path);

const isInteractiveContextTarget = (target: HTMLElement | null) =>
    !!target?.closest(
        [
            "a",
            "button",
            "input",
            "textarea",
            "select",
            "[contenteditable='true']",
            "[role='menu']",
            "[role='menuitem']",
            ".top-bar",
            ".menu-bar",
            ".player-controls-content",
            ".playlist-drawer",
            ".side-actions-nav",
            ".track-menu-container",
        ].join(", "),
    );

export const usePlaybackContextMenu = ({
    isFileLoaded,
    getCurrentPath,
    getCurrentTitle,
    addToFavorites,
    isFavorite,
    searchOnlineSubtitles,
    openSubtitleAdvancedSettings,
    openSettings,
    regenerateThumbnails,
    exportContactSheet,
    isMenuBarVisible,
    toggleMenuBar,
    hideAllMenus,
}: UsePlaybackContextMenuOptions) => {
    const isOpen = ref(false);
    const position = ref({ x: 0, y: 0 });

    const items = computed<ContextMenuItem[]>(() => {
        const path = getCurrentPath().trim();
        const hasPath = !!path;
        const isLocal = isLocalMediaPath(path);
        return [
            {
                id: ADD_TO_FAVORITES_ID,
                label: isFavorite() ? "Remove from Favourites" : "Add to Favourites",
                icon: "heart",
                disabled: !hasPath,
            },
            {
                id: SUBTITLE_ID,
                label: "Subtitle",
                icon: "subtitle",
                disabled: !hasPath,
                children: [
                    {
                        id: SUBTITLE_ADVANCED_SETTINGS_ID,
                        label: "Advanced Subtitle Settings",
                        icon: "subtitle-advanced-settings",
                        disabled: !hasPath,
                    },
                    {
                        id: SUBTITLE_FIND_ONLINE_ID,
                        label: "Find Online Subtitles",
                        icon: "subtitle-search",
                        disabled: !hasPath,
                    },
                ],
            },
            {
                id: REGENERATE_THUMBS_ID,
                label: "Regenerate Thumbnails",
                icon: "thumbnails",
                disabled: !isLocal,
            },
            {
                id: CONTACT_SHEET_ID,
                label: "Save Contact Sheet",
                icon: "contact-sheet",
                disabled: !isLocal,
            },
            {
                id: TOGGLE_MENU_BAR_ID,
                label: isMenuBarVisible() ? "Hide Menu Bar" : "Show Menu Bar",
                icon: "menu-bar",
            },
            {
                id: OPEN_SETTINGS_ID,
                label: "Open Settings",
                icon: "settings",
            },
        ];
    });

    const close = () => {
        isOpen.value = false;
    };

    const openAt = (x: number, y: number) => {
        position.value = { x, y };
        isOpen.value = true;
    };

    const onContextMenu = (event: MouseEvent) => {
        if (!isFileLoaded()) return;
        if (isInteractiveContextTarget(event.target as HTMLElement | null)) return;

        const path = getCurrentPath().trim();
        if (!path) return;

        event.preventDefault();
        event.stopPropagation();
        hideAllMenus();
        openAt(event.clientX, event.clientY);
    };

    const onSelect = (id: string) => {
        if (id === ADD_TO_FAVORITES_ID) {
            const path = getCurrentPath().trim();
            if (path) {
                addToFavorites({
                    path,
                    title: getCurrentTitle().trim() || undefined,
                });
            }
        }
        if (id === SUBTITLE_FIND_ONLINE_ID) {
            const path = getCurrentPath().trim();
            if (path) {
                void searchOnlineSubtitles(path, getCurrentTitle().trim() || undefined);
            }
        }
        if (id === SUBTITLE_ADVANCED_SETTINGS_ID) {
            openSubtitleAdvancedSettings();
        }
        if (id === REGENERATE_THUMBS_ID) {
            void regenerateThumbnails();
        }
        if (id === CONTACT_SHEET_ID) {
            void exportContactSheet();
        }
        if (id === TOGGLE_MENU_BAR_ID) {
            toggleMenuBar();
        }
        close();
        if (id === OPEN_SETTINGS_ID) {
            void openSettings();
        }
    };

    return {
        isOpen,
        position,
        items,
        onContextMenu,
        onSelect,
        close,
    };
};
