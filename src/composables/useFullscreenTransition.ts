import { ref } from "vue";

export const useFullscreenTransition = (
    toggleFullscreen: () => Promise<void>,
) => {
    const isFullscreenTransitioning = ref(false);

    const triggerFullscreenTransition = () => {
        isFullscreenTransitioning.value = true;
    };

    const resetFullscreenTransition = () => {
        isFullscreenTransitioning.value = false;
    };

    const onToggleFullscreen = async () => {
        triggerFullscreenTransition();
        await toggleFullscreen();
    };

    return {
        isFullscreenTransitioning,
        triggerFullscreenTransition,
        resetFullscreenTransition,
        onToggleFullscreen,
    };
};
