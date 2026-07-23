<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { MenuTopLevel } from "../../types/menu";
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
}>();

const emit = defineEmits<{ (e: "open"): void }>();

const openIndex = ref<number | null>(null);
const rootRef = ref<HTMLElement | null>(null);

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

// Capture phase: the app-level Escape handler listens on window, so we have to
// get there first to keep Escape scoped to the open menu.
onMounted(() => {
    window.addEventListener("pointerdown", onDocumentPointerDown, true);
    window.addEventListener("keydown", onDocumentKeydown, true);
});

onBeforeUnmount(() => {
    window.removeEventListener("pointerdown", onDocumentPointerDown, true);
    window.removeEventListener("keydown", onDocumentKeydown, true);
});

// A menu left open while the window changes shape would float detached.
watch(
    () => props.menus.length,
    () => close(),
);

defineExpose({ close });
</script>

<template>
    <div
        ref="rootRef"
        class="menu-bar"
        role="menubar"
    >
        <div
            v-for="(menu, index) in props.menus"
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

        <!-- Pushes the window buttons to the far right of the bar. -->
        <div class="menu-bar__spacer"></div>
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
