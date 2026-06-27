import { ref } from "vue";

export const useUiControls = (
    isFileLoaded: () => boolean,
    onHideMenus: () => void,
    shouldKeepVisible: () => boolean,
    onShowControls?: () => void,
) => {
    const showControls = ref(true);
    const hoverFilePicker = ref(false);
    let hideTimeout: ReturnType<typeof setTimeout> | null = null;

    const hideBar = () => {
        if (shouldKeepVisible()) {
            resetInactivityTimer();
            return;
        }
        showControls.value = false;
        onHideMenus();
    };
    const showBar = () => {
        const wasHidden = !showControls.value;
        showControls.value = true;
        if (wasHidden) onShowControls?.();
    };

    const resetInactivityTimer = () => {
        if (hideTimeout) clearTimeout(hideTimeout);
        hideTimeout = setTimeout(hideBar, 2000);
    };

    const stopInactivityTimer = () => {
        if (hideTimeout) clearTimeout(hideTimeout);
        hideTimeout = null;
    };

    const onUserInteraction = () => {
        if (!isFileLoaded()) return;
        showBar();
        resetInactivityTimer();
    };

    const cleanup = () => {
        if (hideTimeout) clearTimeout(hideTimeout);
    };

    return {
        showControls,
        hoverFilePicker,
        onUserInteraction,
        resetInactivityTimer,
        stopInactivityTimer,
        cleanup,
    };
};
