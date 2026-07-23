<script setup lang="ts">
import { computed, ref } from "vue";
import type { MenuNode } from "../../types/menu";

// One dropdown level, rendered recursively.
//
// Submenus are teleported to <body> and positioned with fixed coordinates
// rather than nested inside the parent list. Two things make that necessary:
// the list scrolls (`overflow-y: auto`), and per the CSS spec an `overflow-x`
// of `visible` computes to `auto` when the other axis isn't visible — so a
// nested child at `left: 100%` gets clipped. `backdrop-filter` also makes the
// list a containing block for fixed descendants, so escaping it needs a
// teleport, not just `position: fixed`.

const props = defineProps<{
    items: MenuNode[];
    /** Viewport coordinates for a teleported submenu. */
    fixedAt?: { top: number; left: number } | null;
}>();

const emit = defineEmits<{ (e: "close"): void }>();

const openIndex = ref<number | null>(null);
const submenuAt = ref<{ top: number; left: number } | null>(null);

/** Enough to decide which side a submenu opens on before it paints. */
const ESTIMATED_SUBMENU_WIDTH = 240;
const ESTIMATED_SUBMENU_HEIGHT = 260;
const EDGE_GAP = 8;

const onEnterItem = (index: number, node: MenuNode, event: MouseEvent) => {
    if (node.kind !== "submenu" || node.disabled) {
        openIndex.value = null;
        submenuAt.value = null;
        return;
    }
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();

    // Prefer the right; flip left when it would run off the edge.
    const overflowsRight =
        rect.right + ESTIMATED_SUBMENU_WIDTH > window.innerWidth - EDGE_GAP;
    const left = overflowsRight
        ? Math.max(EDGE_GAP, rect.left - ESTIMATED_SUBMENU_WIDTH - 2)
        : rect.right + 2;

    // Align with the row, then lift it if it would hang below the window.
    const maxTop = window.innerHeight - EDGE_GAP - ESTIMATED_SUBMENU_HEIGHT;
    const top = Math.max(EDGE_GAP, Math.min(rect.top - 5, Math.max(EDGE_GAP, maxTop)));

    submenuAt.value = { top, left };
    openIndex.value = index;
};

const onActivate = (node: MenuNode) => {
    if (node.kind !== "action" || node.disabled) return;
    void node.run();
    emit("close");
};

const submenuItems = computed<MenuNode[] | null>(() => {
    if (openIndex.value === null) return null;
    const node = props.items[openIndex.value];
    return node && node.kind === "submenu" ? node.children : null;
});
</script>

<template>
    <div
        class="menu-list"
        :class="{ 'menu-list--floating': !!props.fixedAt }"
        :style="
            props.fixedAt
                ? { top: `${props.fixedAt.top}px`, left: `${props.fixedAt.left}px` }
                : undefined
        "
        role="menu"
        data-window-no-drag
        data-menu-surface
    >
        <template v-for="(node, index) in props.items" :key="index">
            <div v-if="node.kind === 'separator'" class="menu-list__sep"></div>

            <div
                v-else
                class="menu-list__row"
                :class="{
                    'menu-list__row--disabled': node.disabled,
                    'menu-list__row--open':
                        node.kind === 'submenu' && openIndex === index,
                }"
                role="menuitem"
                :aria-disabled="node.disabled ? 'true' : undefined"
                :aria-haspopup="node.kind === 'submenu' ? 'true' : undefined"
                tabindex="-1"
                @mouseenter="onEnterItem(index, node, $event)"
                @click.stop="onActivate(node)"
            >
                <span class="menu-list__check">
                    <svg
                        v-if="node.kind === 'action' && node.checked"
                        viewBox="0 0 24 24"
                        fill="currentColor"
                        aria-hidden="true"
                    >
                        <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" />
                    </svg>
                </span>

                <span class="menu-list__label">{{ node.label }}</span>

                <span
                    v-if="node.kind === 'action' && node.shortcut"
                    class="menu-list__key"
                >
                    {{ node.shortcut }}
                </span>
                <span v-else-if="node.kind === 'submenu'" class="menu-list__arrow">
                    ›
                </span>
            </div>
        </template>

        <!-- Outside the scrolling list so it can't be clipped by it. -->
        <Teleport to="body">
            <MenuList
                v-if="submenuItems && submenuAt"
                :items="submenuItems"
                :fixed-at="submenuAt"
                @close="emit('close')"
            />
        </Teleport>
    </div>
</template>

<style scoped>
.menu-list {
    position: absolute;
    top: 100%;
    left: 0;
    min-width: 210px;
    max-width: 340px;
    padding: 5px;
    border-radius: 10px;
    background: rgba(26, 28, 33, 0.98);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: 0 12px 34px rgba(0, 0, 0, 0.5);
    z-index: 200;
    /* Very long lists (track lists, presets) scroll instead of overflowing the
       window. Submenus are teleported out, so nothing needs to escape this. */
    max-height: min(70vh, 620px);
    overflow-y: auto;
    overflow-x: hidden;
}

/* Teleported to <body>, so it positions against the viewport. */
.menu-list--floating {
    position: fixed;
    /* Above the menu bar itself (126) and every panel. */
    z-index: 240;
}

.menu-list__sep {
    height: 1px;
    margin: 5px 8px;
    background: rgba(255, 255, 255, 0.1);
}

.menu-list__row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 9px;
    border-radius: 6px;
    color: rgba(255, 255, 255, 0.9);
    font-size: 12.5px;
    line-height: 1.3;
    cursor: pointer;
    white-space: nowrap;
}

.menu-list__row:hover:not(.menu-list__row--disabled),
.menu-list__row--open {
    background: rgba(255, 255, 255, 0.13);
}

.menu-list__row--disabled {
    color: rgba(255, 255, 255, 0.32);
    cursor: default;
}

.menu-list__check {
    flex: none;
    width: 14px;
    height: 14px;
    color: var(--progress-color, #4a9eff);
}

.menu-list__check svg {
    width: 14px;
    height: 14px;
    display: block;
}

.menu-list__label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
}

.menu-list__key {
    flex: none;
    margin-left: 18px;
    color: rgba(255, 255, 255, 0.42);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
}

.menu-list__arrow {
    flex: none;
    margin-left: 12px;
    color: rgba(255, 255, 255, 0.5);
    font-size: 15px;
    line-height: 1;
}
</style>
