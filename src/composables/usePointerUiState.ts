import { nextTick, onMounted, onUnmounted, ref } from "vue";

type UsePointerUiStateOptions = {
    revealWidthCssVar?: string;
    revealWidthFallback?: number;
};

const DEFAULT_REVEAL_WIDTH_CSS_VAR = "--playlist-peek-reveal-width";
const DEFAULT_REVEAL_WIDTH_FALLBACK = 80;

type LastPointerPosition = {
    x: number | null;
    y: number | null;
};

export const usePointerUiState = (options: UsePointerUiStateOptions = {}) => {
    const revealWidthCssVar =
        options.revealWidthCssVar ?? DEFAULT_REVEAL_WIDTH_CSS_VAR;
    const revealWidthFallback =
        options.revealWidthFallback ?? DEFAULT_REVEAL_WIDTH_FALLBACK;
    const revealWidth = ref(revealWidthFallback);
    const isPointerOverUi = ref(false);
    const isPointerNearLeft = ref(false);
    const lastPointerPosition: LastPointerPosition = {
        x: null,
        y: null,
    };

    const setPointerFlags = (x: number, target: Element | null) => {
        const isOverControls = !!target?.closest(".player-controls");
        const isOverTrackMenu = !!target?.closest(".track-menu");
        const isOverTopBar = !!target?.closest(".top-bar");
        isPointerOverUi.value =
            !!target?.closest(".ui-surface") ||
            isOverControls ||
            isOverTrackMenu ||
            isOverTopBar;
        isPointerNearLeft.value =
            x <= revealWidth.value && !isOverControls && !isOverTopBar;
    };

    const resetPointerState = () => {
        isPointerOverUi.value = false;
        isPointerNearLeft.value = false;
        lastPointerPosition.x = null;
        lastPointerPosition.y = null;
    };

    const updateFromMouseEvent = (event: MouseEvent) => {
        if (!(event.target instanceof Element)) {
            return;
        }
        lastPointerPosition.x = event.clientX;
        lastPointerPosition.y = event.clientY;
        setPointerFlags(event.clientX, event.target);
    };

    const refreshPointerState = () => {
        if (lastPointerPosition.x === null || lastPointerPosition.y === null) {
            resetPointerState();
            return;
        }
        const element = document.elementFromPoint(
            lastPointerPosition.x,
            lastPointerPosition.y,
        );
        setPointerFlags(lastPointerPosition.x, element);
    };

    const schedulePointerRefresh = () => {
        void nextTick(() => {
            requestAnimationFrame(() => {
                refreshPointerState();
            });
        });
    };

    const readRevealWidth = () => {
        const raw = getComputedStyle(document.documentElement)
            .getPropertyValue(revealWidthCssVar)
            .trim();
        const parsed = Number.parseFloat(raw);
        return Number.isFinite(parsed) ? parsed : revealWidthFallback;
    };

    const updateRevealWidth = () => {
        revealWidth.value = readRevealWidth();
    };

    const onWindowMouseOut = (event: MouseEvent) => {
        const leftWindowBounds =
            event.clientX <= 0 ||
            event.clientX >= window.innerWidth ||
            event.clientY <= 0 ||
            event.clientY >= window.innerHeight;
        if (event.relatedTarget === null && leftWindowBounds) {
            resetPointerState();
        }
    };

    const onVisibilityChange = () => {
        if (document.hidden) {
            resetPointerState();
        }
    };

    onMounted(() => {
        updateRevealWidth();
        window.addEventListener("resize", updateRevealWidth);
        window.addEventListener("mousemove", updateFromMouseEvent, {
            passive: true,
        });
        window.addEventListener("mousedown", updateFromMouseEvent, {
            passive: true,
        });
        window.addEventListener("pointercancel", resetPointerState);
        window.addEventListener("pointerleave", resetPointerState);
        window.addEventListener("mouseleave", resetPointerState);
        window.addEventListener("mouseout", onWindowMouseOut);
        window.addEventListener("blur", resetPointerState);
        document.addEventListener("visibilitychange", onVisibilityChange);
    });

    onUnmounted(() => {
        window.removeEventListener("resize", updateRevealWidth);
        window.removeEventListener("mousemove", updateFromMouseEvent);
        window.removeEventListener("mousedown", updateFromMouseEvent);
        window.removeEventListener("pointercancel", resetPointerState);
        window.removeEventListener("pointerleave", resetPointerState);
        window.removeEventListener("mouseleave", resetPointerState);
        window.removeEventListener("mouseout", onWindowMouseOut);
        window.removeEventListener("blur", resetPointerState);
        document.removeEventListener("visibilitychange", onVisibilityChange);
    });

    return {
        revealWidth,
        isPointerOverUi,
        isPointerNearLeft,
        schedulePointerRefresh,
    };
};
