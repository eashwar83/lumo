<script setup lang="ts">
const props = defineProps<{
    toast: { from: number; to: number } | null;
}>();

const emit = defineEmits<{
    (e: "undo"): void;
    (e: "dismiss"): void;
}>();

const formatTime = (seconds: number) => {
    const total = Math.max(0, Math.floor(seconds));
    const minutes = Math.floor(total / 60);
    const secs = total % 60;
    return `${minutes}:${String(secs).padStart(2, "0")}`;
};
</script>

<template>
    <transition name="yt-sponsor-toast">
        <div v-if="props.toast" class="yt-sponsor-toast" data-window-no-drag>
            <span class="yt-sponsor-toast__text">
                Skipped sponsor ({{ formatTime(props.toast.from) }} –
                {{ formatTime(props.toast.to) }})
            </span>
            <button
                class="yt-sponsor-toast__undo"
                type="button"
                @click="emit('undo')"
            >
                Undo
            </button>
            <button
                class="yt-sponsor-toast__dismiss"
                type="button"
                aria-label="Dismiss"
                @click="emit('dismiss')"
            >
                ✕
            </button>
        </div>
    </transition>
</template>

<style scoped>
.yt-sponsor-toast {
    position: fixed;
    left: 26px;
    bottom: 96px;
    z-index: 110;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border: 1px solid rgba(159, 216, 164, 0.45);
    border-radius: 9px;
    background: rgba(18, 20, 24, 0.92);
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
    color: #ececef;
    font-size: 12.5px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45);
    user-select: none;
    -webkit-user-select: none;
}

.yt-sponsor-toast__text {
    color: #9fd8a4;
    font-weight: 600;
}

.yt-sponsor-toast__undo {
    border: 1px solid rgba(255, 255, 255, 0.22);
    border-radius: 7px;
    background: transparent;
    color: #fff;
    font-size: 12px;
    font-weight: 700;
    padding: 4px 12px;
    cursor: pointer;
}

.yt-sponsor-toast__undo:hover {
    background: rgba(255, 255, 255, 0.12);
}

.yt-sponsor-toast__dismiss {
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
    font-size: 11px;
    width: 22px;
    height: 22px;
    cursor: pointer;
}

.yt-sponsor-toast__dismiss:hover {
    color: #fff;
}

.yt-sponsor-toast-enter-active,
.yt-sponsor-toast-leave-active {
    transition:
        opacity 0.2s ease,
        transform 0.2s ease;
}

.yt-sponsor-toast-enter-from,
.yt-sponsor-toast-leave-to {
    opacity: 0;
    transform: translateY(8px);
}
</style>
