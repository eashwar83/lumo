import { ref } from "vue";

export const useHoverState = () => {
    const isControlsHovered = ref(false);
    const isTopBarHovered = ref(false);
    const isFilePickerHovered = ref(false);

    const shouldKeepControlsVisible = () =>
        isControlsHovered.value ||
        isTopBarHovered.value ||
        isFilePickerHovered.value;

    return {
        isControlsHovered,
        isTopBarHovered,
        isFilePickerHovered,
        shouldKeepControlsVisible,
    };
};
