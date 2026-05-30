<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import ConfirmDialog from "./ConfirmDialog.vue";

const props = defineProps<{
    open: boolean;
    message: string;
    nameDraft: string;
}>();

const emit = defineEmits<{
    (e: "update:nameDraft", value: string): void;
    (e: "cancel"): void;
    (e: "confirm"): void;
}>();

const inputRef = ref<HTMLInputElement | null>(null);
const playlistName = computed({
    get: () => props.nameDraft,
    set: (value: string) => emit("update:nameDraft", value),
});

const focusNameInput = async () => {
    await nextTick();
    window.requestAnimationFrame(() => {
        inputRef.value?.focus();
        inputRef.value?.select();
    });
};

watch(
    () => props.open,
    (open) => {
        if (!open) return;
        void focusNameInput();
    },
);
</script>

<template>
    <ConfirmDialog
        :open="props.open"
        title="Create Playlist"
        :message="props.message"
        confirm-text="Create"
        cancel-text="Cancel"
        confirm-variant="primary"
        @cancel="emit('cancel')"
        @confirm="emit('confirm')"
    >
        <div class="playlist-create-prompt">
            <p class="playlist-create-prompt__message">
                {{ props.message }}
            </p>
            <label class="playlist-create-prompt__field">
                <span class="playlist-create-prompt__label">Playlist Name</span>
                <input
                    ref="inputRef"
                    v-model="playlistName"
                    class="playlist-create-prompt__input"
                    type="text"
                    autocomplete="off"
                    @keydown.enter.prevent="emit('confirm')"
                />
            </label>
        </div>
    </ConfirmDialog>
</template>

<style scoped>
.playlist-create-prompt {
    display: grid;
    gap: 14px;
}

.playlist-create-prompt__message {
    margin: 0;
    white-space: pre-line;
}

.playlist-create-prompt__field {
    display: grid;
    gap: 7px;
}

.playlist-create-prompt__label {
    color: rgba(255, 255, 255, 0.62);
    font-size: 12px;
    font-weight: 600;
}

.playlist-create-prompt__input {
    width: 100%;
    box-sizing: border-box;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.94);
    font: inherit;
    outline: none;
    padding: 10px 12px;
}

.playlist-create-prompt__input:focus {
    border-color: rgba(112, 166, 255, 0.78);
    box-shadow: 0 0 0 3px rgba(112, 166, 255, 0.16);
}

:global(:root[data-theme="light"]) .playlist-create-prompt__label {
    color: rgba(33, 45, 60, 0.64);
}

:global(:root[data-theme="light"]) .playlist-create-prompt__input {
    border-color: rgba(33, 45, 60, 0.16);
    background: rgba(255, 255, 255, 0.78);
    color: rgba(33, 45, 60, 0.92);
}

</style>
