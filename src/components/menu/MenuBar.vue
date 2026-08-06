<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { MenuNode, MenuTopLevel } from "../../types/menu";
import MenuList from "./MenuList.vue";
import WindowControls from "../WindowControls.vue";

// The VLC-style menu row. Click a title to open it; with one open, hovering the
// others switches between them, which is what every desktop menu bar does.

const props = defineProps<{
    /** Rebuilt by the caller whenever it opens, so state is never stale. */
    menus: MenuTopLevel[];
    /**
     * Host the app's own minimise/maximise/close buttons at the right end. Set
     * when the window is in compact mode, where the OS doesn't draw them.
     */
    showWindowControls?: boolean;
    /** Shown in the middle of the bar, the way a title bar would. */
    title?: string;
}>();

const emit = defineEmits<{ (e: "open"): void }>();

const openIndex = ref<number | null>(null);
const rootRef = ref<HTMLElement | null>(null);

// --- overflow ---------------------------------------------------------
// A narrow window used to clip the last menus and push the window buttons
// off the edge entirely. Titles that no longer fit move into a » popup,
// so every menu stays reachable at any width.
const OVERFLOW_INDEX = -2;
const titlesRef = ref<HTMLElement | null>(null);
const visibleCount = ref(props.menus.length);
/** Widths of the full title row, measured once while everything fits. */
let titleWidths: number[] = [];

const hiddenMenus = computed(() => props.menus.slice(visibleCount.value));
const shownMenus = computed(() => props.menus.slice(0, visibleCount.value));

/** The hidden menus as submenu nodes, for the » popup. */
const overflowItems = computed<MenuNode[]>(() =>
    hiddenMenus.value.map((menu) => ({
        kind: "submenu",
        label: menu.label,
        children: menu.children,
    })),
);

const measureTitles = () => {
    const row = titlesRef.value;
    if (!row) return;
    titleWidths = Array.from(row.children).map(
        (child) => (child as HTMLElement).offsetWidth,
    );
};

const fitTitles = () => {
    const root = rootRef.value;
    if (!root || !titleWidths.length) return;
    // What the row may occupy: everything except the window buttons and a
    // little breathing room for the title.
    const controls = root.querySelector("[data-window-controls]");
    const controlsWidth = (controls as HTMLElement | null)?.offsetWidth ?? 0;
    const available = root.clientWidth - controlsWidth - OVERFLOW_BUTTON_WIDTH - 16;

    let used = 0;
    let count = 0;
    for (const width of titleWidths) {
        if (used + width > available) break;
        used += width;
        count += 1;
    }
    // Showing one menu plus » is no better than showing none, and an empty
    // bar is confusing; keep at least one.
    visibleCount.value = Math.max(1, Math.min(count, props.menus.length));
};

const OVERFLOW_BUTTON_WIDTH = 34;

const close = () => {
    openIndex.value = null;
};

const onTitleClick = (index: number) => {
    if (openIndex.value === index) {
        close();
        return;
    }
    // Let the owner refresh track lists / toggle states before we render.
    emit("open");
    openIndex.value = index;
};

const onTitleEnter = (index: number) => {
    if (openIndex.value === null) return;
    emit("open");
    openIndex.value = index;
};

const onDocumentPointerDown = (event: PointerEvent) => {
    if (openIndex.value === null) return;
    const target = event.target as HTMLElement | null;
    if (rootRef.value?.contains(target)) return;
    // Submenus are teleported to <body>, so they aren't inside the bar — but a
    // click in one is still a click "inside" the menu.
    if (target?.closest?.("[data-menu-surface]")) return;
    close();
};

const onDocumentKeydown = (event: KeyboardEvent) => {
    if (openIndex.value === null) return;
    if (event.key !== "Escape") return;
    // Swallow it so the app-wide Escape chain doesn't also fire.
    event.preventDefault();
    event.stopPropagation();
    close();
};

// Measuring needs every title laid out, so show them all for one frame,
// record their widths, then decide how many survive.
const remeasure = async () => {
    visibleCount.value = props.menus.length;
    await nextTick();
    measureTitles();
    fitTitles();
};

let resizeObserver: ResizeObserver | null = null;

// Capture phase: the app-level Escape handler listens on window, so we have to
// get there first to keep Escape scoped to the open menu.
onMounted(() => {
    window.addEventListener("pointerdown", onDocumentPointerDown, true);
    window.addEventListener("keydown", onDocumentKeydown, true);
    void remeasure();
    if (rootRef.value && typeof ResizeObserver !== "undefined") {
        resizeObserver = new ResizeObserver(() => fitTitles());
        resizeObserver.observe(rootRef.value);
    }
});

onBeforeUnmount(() => {
    window.removeEventListener("pointerdown", onDocumentPointerDown, true);
    window.removeEventListener("keydown", onDocumentKeydown, true);
    resizeObserver?.disconnect();
});

// A menu left open while the window changes shape would float detached.
watch(
    () => props.menus.length,
    () => {
        close();
        void remeasure();
    },
);

defineExpose({ close });
</script>

<template>
    <div
        ref="rootRef"
        class="menu-bar"
        role="menubar"
    >
        <div ref="titlesRef" class="menu-bar__titles">
            <div
                v-for="(menu, index) in shownMenus"
                :key="menu.label"
                class="menu-bar__item"
            >
                <button
                    class="menu-bar__title"
                    :class="{ 'menu-bar__title--open': openIndex === index }"
                    type="button"
                    role="menuitem"
                    :aria-expanded="openIndex === index"
                    aria-haspopup="true"
                    @click.stop="onTitleClick(index)"
                    @mouseenter="onTitleEnter(index)"
                >
                    {{ menu.label }}
                </button>
                <MenuList
                    v-if="openIndex === index"
                    :items="menu.children"
                    @close="close"
                />
            </div>
        </div>

        <div v-if="hiddenMenus.length" class="menu-bar__item">
            <button
                class="menu-bar__title menu-bar__overflow"
                :class="{
                    'menu-bar__title--open': openIndex === OVERFLOW_INDEX,
                }"
                type="button"
                role="menuitem"
                :title="`More menus: ${hiddenMenus.map((m) => m.label).join(', ')}`"
                :aria-expanded="openIndex === OVERFLOW_INDEX"
                aria-haspopup="true"
                @click.stop="onTitleClick(OVERFLOW_INDEX)"
                @mouseenter="onTitleEnter(OVERFLOW_INDEX)"
            >
                »
            </button>
            <MenuList
                v-if="openIndex === OVERFLOW_INDEX"
                :items="overflowItems"
                @close="close"
            />
        </div>

        <!-- Pushes the window buttons to the far right of the bar, and
             carries the title the way a native title bar would. -->
        <div class="menu-bar__spacer">
            <span v-if="props.title" class="menu-bar__media-title">{{
                props.title
            }}</span>
        </div>
        <WindowControls v-if="props.showWindowControls" />
    </div>
</template>

<style scoped>
.menu-bar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: var(--menu-bar-height, 30px);
    display: flex;
    align-items: center;
    gap: 1px;
    /* No right padding: the window buttons run flush to the corner, as they do
       in every native title bar. */
    padding: 0 0 0 8px;
    background: rgba(16, 18, 22, 0.92);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    user-select: none;
    -webkit-user-select: none;
    /* Above the side nav and window controls (120) and the playlist peek
       button (125), so an open dropdown is never clipped or overlapped. */
    z-index: 126;
}

.menu-bar__item {
    position: relative;
}

/* The bar root carries no `data-window-no-drag`, so this empty stretch behaves
   like a title bar and can be dragged. The titles are <button>s and the window
   controls opt out on their own, so neither starts a drag. */
.menu-bar__spacer {
    flex: 1;
    align-self: stretch;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
}

.menu-bar__titles {
    display: flex;
    align-items: center;
    gap: 1px;
    flex: none;
}

/* The title is decoration, not a control: it must never win space from
   the menus, and it disappears rather than squeezing them. */
.menu-bar__media-title {
    max-width: 100%;
    padding: 0 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: rgba(255, 255, 255, 0.62);
    font-size: 12px;
    pointer-events: none;
}

.menu-bar__overflow {
    font-size: 15px;
    line-height: 1;
    padding: 3px 9px;
}

.menu-bar__title {
    padding: 4px 10px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.82);
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
    transition: background-color 0.12s ease, color 0.12s ease;
}

.menu-bar__title:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
}

.menu-bar__title--open {
    background: rgba(255, 255, 255, 0.16);
    color: #fff;
}

/* Fades in/out with the rest of the on-screen chrome. */
.fade-enter-active,
.fade-leave-active {
    transition: opacity 0.18s ease;
}

.fade-enter-from,
.fade-leave-to {
    opacity: 0;
}
</style>
