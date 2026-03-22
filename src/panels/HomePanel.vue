<script setup lang="ts">
const props = defineProps<{
    isFileLoaded: boolean;
    hover: boolean;
}>();

const emit = defineEmits<{
    (e: "open-file-picker"): void;
    (e: "update:hover", value: boolean): void;
}>();
</script>

<template>
    <div
        class="home-panel"
        @mouseenter="emit('update:hover', true)"
        @mouseleave="emit('update:hover', false)"
    >
        <button
            class="home-panel__button"
            :class="{
                'home-panel__button--hidden':
                    props.isFileLoaded && !props.hover,
            }"
            @click="emit('open-file-picker')"
        >
            <svg
                xmlns="http://www.w3.org/2000/svg"
                fill="none"
                viewBox="0 0 24 24"
                stroke-width="1.0"
                stroke="currentColor"
            >
                <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M12 4.5v15m7.5-7.5h-15"
                />
            </svg>
            <p>Open a File</p>
        </button>
    </div>
</template>

<style scoped>
.home-panel {
    width: 40%;
    max-width: 270px;
    aspect-ratio: 16 / 9;
    border-radius: 12px;
    overflow: hidden;
    pointer-events: auto;
}

.home-panel__button {
    width: 90%;
    height: 90%;
    background-color: transparent;
    border: 2px dashed gray;
    border-radius: 8px;
    color: gray;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    margin: auto;
    opacity: 1;
    transition: all 0.3s ease;
}

.home-panel__button--hidden {
    opacity: 0;
    pointer-events: none;
    border-color: transparent;
}

.home-panel__button:not(.home-panel__button--hidden):hover {
    background-color: rgba(0, 0, 0, 0.4);
    border-color: var(--text-color);
    color: var(--text-color);
}

.home-panel__button svg {
    width: 70px;
    margin-bottom: 1rem;
    filter: grayscale(100%) brightness(150%);
}

.home-panel__button p {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
}

:root[data-theme="light"] .home-panel__button:not(.home-panel__button--hidden):hover {
    background-color: rgba(57, 108, 216, 0.12);
    border-color: rgba(57, 108, 216, 0.72);
    color: rgba(25, 43, 74, 0.94);
    box-shadow: 0 10px 22px rgba(57, 108, 216, 0.16);
}

:root[data-theme="light"] .home-panel__button:not(.home-panel__button--hidden):hover svg {
    filter: grayscale(30%) brightness(100%);
}

:root[data-theme="graphite"] .home-panel__button:not(.home-panel__button--hidden):hover {
    background-color: rgba(132, 150, 176, 0.2);
    border-color: rgba(176, 194, 218, 0.72);
    color: #e8eef8;
    box-shadow: 0 12px 24px rgba(0, 0, 0, 0.24);
}

:root[data-theme="graphite"] .home-panel__button:not(.home-panel__button--hidden):hover svg {
    filter: grayscale(30%) brightness(120%);
}
</style>
