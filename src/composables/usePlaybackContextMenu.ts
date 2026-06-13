import { computed, ref } from "vue";

type ContextMenuItem = {
    id: string;
    label: string;
    icon?: "heart" | "settings";
    disabled?: boolean;
};

type UsePlaybackContextMenuOptions = {
    isFileLoaded: () => boolean;
    getCurrentPath: () => string;
    getCurrentTitle: () => string;
    addToFavorites: (item: { path: string; title?: string }) => void;
    openSettings: () => void | Promise<void>;
    hideAllMenus: () => void;
};

const ADD_TO_FAVORITES_ID = "add-to-favorites";
const OPEN_SETTINGS_ID = "open-settings";

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
    openSettings,
    hideAllMenus,
}: UsePlaybackContextMenuOptions) => {
    const isOpen = ref(false);
    const position = ref({ x: 0, y: 0 });

    const items = computed<ContextMenuItem[]>(() => [
        {
            id: ADD_TO_FAVORITES_ID,
            label: "Add to Favorites",
            icon: "heart",
            disabled: !getCurrentPath().trim(),
        },
        {
            id: OPEN_SETTINGS_ID,
            label: "Open Settings",
            icon: "settings",
        },
    ]);

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
