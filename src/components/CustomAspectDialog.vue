<script setup lang="ts">
import { ref, computed, watch, nextTick } from "vue";
import {
    ASPECT_PRESETS,
    parseCustomAspectRatio,
} from "../composables/useVideoGeometry";

const props = defineProps<{
    open: boolean;
    currentValue: string;
    currentLabel: string;
}>();

const emit = defineEmits<{
    (e: "close"): void;
    (e: "apply", value: string): void;
}>();

const QUICK_RATIOS: { label: string; value: string }[] = [
    ...ASPECT_PRESETS,
    { label: "16:10", value: "16:10" },
    { label: "18:9", value: "18:9" },
    { label: "1.85:1", value: "1.85" },
    { label: "2.39:1", value: "2.39" },
    { label: "3:2", value: "3:2" },
    { label: "1:1", value: "1:1" },
    { label: "4:5", value: "4:5" },
    { label: "9:16", value: "9:16" },
];

const draft = ref("");
const inputRef = ref<HTMLInputElement | null>(null);

watch(
    () => props.open,
    (open) => {
        if (!open) return;
        draft.value =
            props.currentValue && props.currentValue !== "-1"
                ? props.currentLabel
                : "";
        void nextTick(() => {
            inputRef.value?.focus();
            inputRef.value?.select();
        });
    },
);

const parsed = computed(() => {
    const raw = draft.value.trim();
    if (!raw) {
        return { value: "-1", label: "Default", numericRatio: 0 };
    }
    return parseCustomAspectRatio(raw);
});

const pickQuick = (value: string, label: string) => {
    draft.value = value === "-1" ? "" : label;
};

const applyAspect = () => {
    if (!parsed.value) return;
    emit("apply", parsed.value.value);
    emit("close");
};
</script>

<template>
    <transition name="custom-aspect-fade">
        <div
            v-if="open"
            class="custom-aspect"
            @keydown.esc.stop.prevent="emit('close')"
        >
            <div class="custom-aspect__backdrop" @click="emit('close')" />
            <div
                class="custom-aspect__box"
                role="dialog"
                aria-label="Custom Aspect Ratio"
            >
                <div class="custom-aspect__title">Aspect Ratio</div>
                <p class="custom-aspect__hint">
                    Pick a preset or enter any custom display aspect ratio
                    (e.g. <code>16:10</code>, <code>1.85:1</code>,
                    <code>2.39</code>, <code>9:16</code>). Leave empty for the
                    file’s original aspect ratio.
                </p>

                <div class="custom-aspect__section">
                    <div class="custom-aspect__label">Presets</div>
                    <div class="custom-aspect__pills">
                        <button
                            v-for="item in QUICK_RATIOS"
                            :key="item.value"
                            class="custom-aspect__pill"
                            :class="{
                                'custom-aspect__pill--active':
                                    parsed?.value === item.value,
                            }"
                            type="button"
                            @click="pickQuick(item.value, item.label)"
                        >
                            {{ item.label }}
                        </button>
                    </div>
                </div>

                <div class="custom-aspect__section">
                    <label
                        class="custom-aspect__label"
                        for="custom-aspect-input"
                    >
                        Custom Ratio
                    </label>
                    <input
                        id="custom-aspect-input"
                        ref="inputRef"
                        v-model="draft"
                        class="custom-aspect__input"
                        type="text"
                        placeholder="e.g. 16:10, 1.85:1, 2.39, 9:16 (empty = Default)"
                        @keydown.enter.prevent="applyAspect"
                        @keydown.stop
                    />
                    <p
                        v-if="draft.trim() && !parsed"
                        class="custom-aspect__note custom-aspect__note--warn"
                    >
                        Enter a valid positive ratio such as <code>16:10</code>,
                        <code>4/3</code>, <code>1.85:1</code>, or
                        <code>2.39</code>.
                    </p>
                    <p v-else-if="parsed" class="custom-aspect__note">
                        Will apply: <strong>{{ parsed.label }}</strong>
                        <span v-if="parsed.numericRatio > 0">
                            ({{ parsed.numericRatio.toFixed(2) }}:1)
                        </span>
                    </p>
                </div>

                <div class="custom-aspect__actions">
                    <button
                        class="custom-aspect__btn"
                        type="button"
                        @click="emit('close')"
                    >
                        Cancel
                    </button>
                    <button
                        class="custom-aspect__btn custom-aspect__btn--primary"
                        type="button"
                        :disabled="!parsed"
                        @click="applyAspect"
                    >
                        Apply
                    </button>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.custom-aspect {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    align-items: center;
    justify-content: center;
}
.custom-aspect__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}
.custom-aspect__box {
    position: relative;
    width: min(460px, 92vw);
    max-height: 88vh;
    overflow-y: auto;
    padding: 20px;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    color: #fff;
}
.custom-aspect__title {
    font-size: 15px;
    font-weight: 700;
}
.custom-aspect__hint {
    margin: 6px 0 12px;
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.55);
}
.custom-aspect__hint code,
.custom-aspect__note code {
    padding: 1px 5px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.85);
    font-size: 11.5px;
}
.custom-aspect__section {
    margin: 12px 0;
}
.custom-aspect__label {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.72);
    margin-bottom: 7px;
}
.custom-aspect__pills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
}
.custom-aspect__pill {
    padding: 6px 11px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.8);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}
.custom-aspect__pill:hover {
    background: rgba(255, 255, 255, 0.11);
}
.custom-aspect__pill--active {
    border-color: rgba(196, 160, 255, 0.6);
    background: rgba(170, 130, 255, 0.32);
    color: #fff;
}
.custom-aspect__input {
    width: 100%;
    box-sizing: border-box;
    padding: 8px 11px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.28);
    color: #fff;
    font-size: 13px;
    font-family: inherit;
    outline: none;
}
.custom-aspect__input:focus {
    border-color: rgba(196, 160, 255, 0.65);
}
.custom-aspect__note {
    margin: 6px 0 0;
    font-size: 12px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.58);
}
.custom-aspect__note--warn {
    color: #ffb454;
}
.custom-aspect__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
}
.custom-aspect__btn {
    padding: 8px 14px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}
.custom-aspect__btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
}
.custom-aspect__btn:disabled {
    opacity: 0.5;
    cursor: default;
}
.custom-aspect__btn--primary {
    border-color: rgba(196, 160, 255, 0.5);
    background: rgba(170, 130, 255, 0.28);
    color: #fff;
}
.custom-aspect__btn--primary:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.42);
}
.custom-aspect-fade-enter-active,
.custom-aspect-fade-leave-active {
    transition: opacity 0.15s ease;
}
.custom-aspect-fade-enter-from,
.custom-aspect-fade-leave-to {
    opacity: 0;
}
</style>
