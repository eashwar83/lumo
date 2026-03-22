<script setup lang="ts">
const props = defineProps<{
    isPlaying: boolean;
    currentTime: number;
    duration: number;
    formatTime: (seconds: number) => string;
    badges: string[];
}>();

const emit = defineEmits<{
    (e: "prev-track"): void;
    (e: "toggle-play-pause"): void;
    (e: "stop-playback"): void;
    (e: "next-track"): void;
}>();
</script>

<template>
    <div class="controls-left">
        <button
            class="icon-button icon-button--player icon-button--lg"
            @click="emit('prev-track')"
            title="Previous"
        >
            <svg viewBox="0 0 24 24" fill="currentColor">
                <path d="M6 18V6h2v12H6zm3.5-6 8.5 6V6l-8.5 6z" />
            </svg>
        </button>
        <button
            class="icon-button icon-button--player icon-button--lg"
            @click="emit('toggle-play-pause')"
        >
            <svg viewBox="0 0 24 24" fill="currentColor">
                <path
                    v-if="!isPlaying"
                    d="M8,5.14V19.14L19,12.14L8,5.14Z"
                />
                <path v-else d="M14,19H18V5H14M6,19H10V5H6V19Z" />
            </svg>
        </button>
        <button
            class="icon-button icon-button--player icon-button--lg"
            @click="emit('next-track')"
            title="Next"
        >
            <svg viewBox="0 0 24 24" fill="currentColor">
                <path d="M16 6v12h2V6h-2zm-1.5 6L6 18V6l8.5 6z" />
            </svg>
        </button>
        <div class="time-display">
            <span>{{ formatTime(currentTime) }}</span>
            <span class="separator">/</span>
            <span>{{ formatTime(duration) }}</span>
        </div>
        <div v-if="props.badges.length" class="status-badges">
            <span
                v-for="badge in props.badges"
                :key="badge"
                class="status-badge"
            >
                {{ badge }}
            </span>
        </div>
    </div>
</template>

<style scoped>
.status-badges {
    display: inline-flex;
    gap: 6px;
    margin-left: 8px;
}

.status-badge {
    font-size: 9px;
    font-weight: 400;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 999px;
    border: none;
    color: rgba(248, 220, 140, 0.95);
    background: none;
}
</style>
