<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from "vue";

const props = defineProps<{
    open: boolean;
    x: number;
    y: number;
    items: {
        id: string;
        label: string;
        disabled?: boolean;
    }[];
}>();

const emit = defineEmits<{
    (e: "select", id: string): void;
    (e: "close"): void;
}>();

const menuRef = ref<HTMLElement | null>(null);
const menuStyle = ref({ left: "0px", top: "0px" });
const isMacOsPlatform =
    typeof navigator !== "undefined" && /mac|darwin/i.test(navigator.userAgent);

const clampPosition = (value: number, size: number, viewportSize: number) =>
    Math.min(Math.max(8, viewportSize - size - 8), Math.max(8, value));

const updateMenuPosition = async () => {
    if (!props.open) return;
    menuStyle.value = {
        left: `${Math.max(8, props.x)}px`,
        top: `${Math.max(8, props.y)}px`,
    };

    await nextTick();

    const menu = menuRef.value;
    const width = menu?.offsetWidth ?? 150;
    const height = menu?.offsetHeight ?? Math.max(40, props.items.length * 34 + 10);

    menuStyle.value = {
        left: `${clampPosition(props.x, width, window.innerWidth)}px`,
        top: `${clampPosition(props.y, height, window.innerHeight)}px`,
    };
};

const close = () => {
    if (props.open) {
        emit("close");
    }
};

const onKeydown = (event: KeyboardEvent) => {
    if (event.key === "Escape") {
        close();
    }
};

const onItemClick = (id: string, disabled?: boolean) => {
    if (disabled) return;
    emit("select", id);
};

watch(
    () => [props.open, props.x, props.y, props.items.length],
    () => {
        void updateMenuPosition();
    },
);

onMounted(() => {
    window.addEventListener("click", close);
    window.addEventListener("keydown", onKeydown);
    window.addEventListener("resize", close);
    window.addEventListener("scroll", close, true);
});

onUnmounted(() => {
    window.removeEventListener("click", close);
    window.removeEventListener("keydown", onKeydown);
    window.removeEventListener("resize", close);
    window.removeEventListener("scroll", close, true);
});
</script>

<template>
    <Teleport to="body">
        <div
            v-if="props.open"
            ref="menuRef"
            class="context-menu"
            :class="{ 'context-menu--macos': isMacOsPlatform }"
            :style="menuStyle"
            role="menu"
            @mousedown.prevent.stop
            @click.stop
            @contextmenu.prevent.stop
        >
            <button
                v-for="item in props.items"
                :key="item.id"
                class="context-menu__item"
                type="button"
                role="menuitem"
                :disabled="item.disabled"
                @click="onItemClick(item.id, item.disabled)"
            >
                {{ item.label }}
            </button>
        </div>
    </Teleport>
</template>

<style scoped>
.context-menu {
    position: fixed;
    z-index: 2600;
    display: grid;
    min-width: 156px;
    overflow: hidden;
    padding: 6px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 11px;
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.06), rgba(255, 255, 255, 0)),
        rgba(20, 22, 29, 0.94);
    box-shadow:
        0 18px 42px rgba(0, 0, 0, 0.42),
        0 2px 8px rgba(0, 0, 0, 0.22),
        inset 0 1px 0 rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(22px) saturate(1.18);
    -webkit-backdrop-filter: blur(22px) saturate(1.18);
    transform-origin: top left;
    animation: context-menu-pop 110ms ease-out;
}

.context-menu--macos {
    border-color: rgba(255, 255, 255, 0.2);
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.11), rgba(255, 255, 255, 0.03)),
        rgba(30, 32, 40, 0.62);
    box-shadow:
        0 22px 56px rgba(0, 0, 0, 0.38),
        0 7px 18px rgba(0, 0, 0, 0.22),
        inset 0 1px 0 rgba(255, 255, 255, 0.18),
        inset 0 0 0 1px rgba(255, 255, 255, 0.04);
    backdrop-filter: blur(34px) saturate(1.85) brightness(1.04);
    -webkit-backdrop-filter: blur(34px) saturate(1.85) brightness(1.04);
}

.context-menu__item {
    width: 100%;
    border: none;
    border-radius: 7px;
    background: transparent;
    color: rgba(255, 255, 255, 0.9);
    cursor: default;
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    line-height: 1.25;
    min-height: 30px;
    padding: 6px 12px;
    text-align: left;
    transition:
        background-color 80ms ease,
        color 80ms ease;
}

.context-menu__item:not(:disabled):hover,
.context-menu__item:not(:disabled):focus-visible {
    background: rgba(86, 137, 232, 0.86);
    color: #fff;
    outline: none;
}

.context-menu__item:not(:disabled):active {
    background: rgba(73, 122, 211, 0.9);
}

.context-menu__item:disabled {
    color: rgba(255, 255, 255, 0.34);
}

:global(:root[data-theme="light"]) .context-menu {
    border-color: rgba(33, 45, 60, 0.12);
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.78), rgba(255, 255, 255, 0.5)),
        rgba(247, 249, 252, 0.9);
    box-shadow:
        0 18px 42px rgba(33, 45, 60, 0.18),
        0 2px 8px rgba(33, 45, 60, 0.1),
        inset 0 1px 0 rgba(255, 255, 255, 0.9);
}

:global(:root[data-theme="light"]) .context-menu--macos {
    border-color: rgba(33, 45, 60, 0.11);
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.74), rgba(255, 255, 255, 0.36)),
        rgba(246, 248, 252, 0.58);
    box-shadow:
        0 22px 56px rgba(33, 45, 60, 0.2),
        0 7px 18px rgba(33, 45, 60, 0.12),
        inset 0 1px 0 rgba(255, 255, 255, 0.95);
}

:global(:root[data-theme="light"]) .context-menu__item {
    color: rgba(33, 45, 60, 0.9);
}

:global(:root[data-theme="light"]) .context-menu__item:not(:disabled):hover,
:global(:root[data-theme="light"]) .context-menu__item:not(:disabled):focus-visible {
    background: rgba(57, 108, 216, 0.9);
    color: #fff;
}

:global(:root[data-theme="light"]) .context-menu__item:disabled {
    color: rgba(33, 45, 60, 0.36);
}

@keyframes context-menu-pop {
    from {
        opacity: 0;
        transform: scale(0.985) translateY(-2px);
    }
    to {
        opacity: 1;
        transform: scale(1) translateY(0);
    }
}

@media (prefers-reduced-motion: reduce) {
    .context-menu {
        animation: none;
    }
}
</style>
