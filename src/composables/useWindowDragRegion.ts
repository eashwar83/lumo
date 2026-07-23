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

type WindowDragRegionOptions = {
    /**
     * Return true to hand the drag to something else. Used while the video is
     * zoomed in, where a left-drag pans the image instead of moving the window.
     */
    shouldSuppress?: () => boolean;
};

export const useWindowDragRegion = (options: WindowDragRegionOptions = {}) => {
    let dragStartX = 0;
    let dragStartY = 0;
    let isDragPending = false;
    let touchDragIdentifier: number | null = null;

    function clearWindowDragCandidate() {
        isDragPending = false;
        touchDragIdentifier = null;
        window.removeEventListener("mousemove", onWindowDragMove);
        window.removeEventListener("mouseup", clearWindowDragCandidate);
        window.removeEventListener("touchmove", onWindowTouchMove);
        window.removeEventListener("touchend", clearWindowDragCandidate);
        window.removeEventListener("touchcancel", clearWindowDragCandidate);
    }

    function tryStartWindowDragging(clientX: number, clientY: number) {
        if (!isDragPending) return;
        const movedX = Math.abs(clientX - dragStartX);
        const movedY = Math.abs(clientY - dragStartY);
        if (movedX < DRAG_THRESHOLD_PX && movedY < DRAG_THRESHOLD_PX) return;

        clearWindowDragCandidate();
        void getCurrentWindow().startDragging();
    }

    function onWindowDragMove(event: MouseEvent) {
        tryStartWindowDragging(event.clientX, event.clientY);
    }

    function onWindowTouchMove(event: TouchEvent) {
        if (touchDragIdentifier === null) return;
        const touch = Array.from(event.changedTouches).find(
            (item) => item.identifier === touchDragIdentifier,
        );
        if (!touch) return;
        event.preventDefault();
        tryStartWindowDragging(touch.clientX, touch.clientY);
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

    function onDragRegionTouchStart(event: TouchEvent) {
        if (event.touches.length !== 1) return;
        const touch = event.touches[0];
        dragStartX = touch.clientX;
        dragStartY = touch.clientY;
        touchDragIdentifier = touch.identifier;
        isDragPending = true;
        window.addEventListener("touchmove", onWindowTouchMove, { passive: false });
        window.addEventListener("touchend", clearWindowDragCandidate, { once: true });
        window.addEventListener("touchcancel", clearWindowDragCandidate, {
            once: true,
        });
    }

    function isInteractiveTarget(target: EventTarget | null): boolean {
        if (!(target instanceof Element)) return false;
        return target.closest(INTERACTIVE_TARGET_SELECTOR) !== null;
    }

    function onAppMouseDownCapture(event: MouseEvent) {
        if (isInteractiveTarget(event.target)) return;
        if (options.shouldSuppress?.()) return;
        onDragRegionMouseDown(event);
    }

    function onAppTouchStartCapture(event: TouchEvent) {
        if (isInteractiveTarget(event.target)) return;
        if (options.shouldSuppress?.()) return;
        onDragRegionTouchStart(event);
    }

    onBeforeUnmount(() => {
        clearWindowDragCandidate();
    });

    return {
        onAppMouseDownCapture,
        onAppTouchStartCapture,
        onDragRegionMouseDown,
        onDragRegionTouchStart,
        clearWindowDragCandidate,
    };
};
