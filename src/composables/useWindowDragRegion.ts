import { onBeforeUnmount } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

const DRAG_THRESHOLD_PX = 4;
const INTERACTIVE_TARGET_SELECTOR = [
    "button",
    "input",
    "textarea",
    "select",
    "option",
    "a[href]",
    "summary",
    "[role='button']",
    "[role='link']",
    "[role='checkbox']",
    "[role='radio']",
    "[role='switch']",
    "[role='menuitem']",
    "[contenteditable='true']",
    "[tabindex]:not([tabindex='-1'])",
    "[data-window-no-drag]",
].join(", ");

export const useWindowDragRegion = () => {
    let dragStartX = 0;
    let dragStartY = 0;
    let isDragPending = false;

    function clearWindowDragCandidate() {
        isDragPending = false;
        window.removeEventListener("mousemove", onWindowDragMove);
        window.removeEventListener("mouseup", clearWindowDragCandidate);
    }

    function onWindowDragMove(event: MouseEvent) {
        if (!isDragPending) return;
        const movedX = Math.abs(event.clientX - dragStartX);
        const movedY = Math.abs(event.clientY - dragStartY);
        if (movedX < DRAG_THRESHOLD_PX && movedY < DRAG_THRESHOLD_PX) return;

        clearWindowDragCandidate();
        void getCurrentWindow().startDragging();
    }

    function onDragRegionMouseDown(event: MouseEvent) {
        if (event.button !== 0) return;
        if (event.detail > 1) return;

        dragStartX = event.clientX;
        dragStartY = event.clientY;
        isDragPending = true;
        window.addEventListener("mousemove", onWindowDragMove);
        window.addEventListener("mouseup", clearWindowDragCandidate, { once: true });
    }

    function isInteractiveTarget(target: EventTarget | null): boolean {
        if (!(target instanceof Element)) return false;
        return target.closest(INTERACTIVE_TARGET_SELECTOR) !== null;
    }

    function onAppMouseDownCapture(event: MouseEvent) {
        if (isInteractiveTarget(event.target)) return;
        onDragRegionMouseDown(event);
    }

    onBeforeUnmount(() => {
        clearWindowDragCandidate();
    });

    return {
        onAppMouseDownCapture,
        onDragRegionMouseDown,
        clearWindowDragCandidate,
    };
};
