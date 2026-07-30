<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useAiConfig } from "../composables/useAiConfig";
import { AI_PROVIDERS } from "../constants/aiModels";

const props = defineProps<{
    open: boolean;
    path: string;
    start: number;
    end: number;
}>();
const emit = defineEmits<{
    (e: "close"): void;
    (e: "notify", message: string): void;
}>();

const aiConfig = useAiConfig();

const loading = ref(false);
const result = ref("");
const errorText = ref("");

const pad = (n: number) => String(n).padStart(2, "0");
const fmt = (s: number) => {
    const t = Math.max(0, Math.floor(s));
    const h = Math.floor(t / 3600);
    const m = Math.floor((t % 3600) / 60);
    return h > 0 ? `${h}:${pad(m)}:${pad(t % 60)}` : `${m}:${pad(t % 60)}`;
};
const rangeLabel = computed(() => `${fmt(props.start)} – ${fmt(props.end)}`);

const run = async () => {
    if (loading.value) return;
    if (!aiConfig.currentKey.value.trim()) {
        errorText.value = "Set your AI key in Settings → AI Enhance first.";
        return;
    }
    loading.value = true;
    result.value = "";
    errorText.value = "";
    try {
        const text = await invoke<string>("describe_clip", {
            path: props.path,
            start: props.start,
            end: props.end,
            provider: aiConfig.provider.value,
            apiKey: aiConfig.currentKey.value,
            model: aiConfig.currentModel.value || null,
            baseUrl: aiConfig.currentBaseUrl.value || null,
            dialogue: null,
        });
        result.value = text;
    } catch (error) {
        errorText.value = String(error).replace(/^Error:\s*/, "").slice(0, 300);
    } finally {
        loading.value = false;
    }
};

watch(
    () => props.open,
    (open) => {
        if (open) {
            result.value = "";
            errorText.value = "";
            void run();
        }
    },
);

const copy = async () => {
    if (!result.value) return;
    try {
        await navigator.clipboard.writeText(result.value);
        emit("notify", "Description copied");
    } catch {
        /* clipboard unavailable */
    }
};
</script>

<template>
    <transition name="cd-fade">
        <div v-if="open" class="cd" @keydown.esc.stop.prevent="emit('close')">
            <div class="cd__backdrop" @click="emit('close')" />
            <div class="cd__box" role="dialog" aria-label="Describe clip">
                <div class="cd__head">
                    <div class="cd__title">Describe Clip (AI)</div>
                    <span class="cd__range">{{ rangeLabel }}</span>
                </div>

                <div class="cd__engine">
                    <select
                        class="cd__select"
                        :value="aiConfig.provider.value"
                        :disabled="loading"
                        aria-label="AI provider"
                        @change="
                            aiConfig.setProvider(
                                ($event.target as HTMLSelectElement).value,
                            )
                        "
                    >
                        <option v-for="p in AI_PROVIDERS" :key="p" :value="p">
                            {{ p }}
                        </option>
                    </select>
                    <select
                        class="cd__select"
                        :value="aiConfig.currentModel.value"
                        :disabled="loading"
                        aria-label="Model"
                        @change="
                            aiConfig.setModel(($event.target as HTMLSelectElement).value)
                        "
                    >
                        <option v-for="m in aiConfig.modelOptions.value" :key="m" :value="m">
                            {{ m }}
                        </option>
                    </select>
                </div>

                <div v-if="loading" class="cd__loading">
                    <span class="cd__spinner" />
                    Watching the clip…
                </div>
                <p v-else-if="errorText" class="cd__error">{{ errorText }}</p>
                <p v-else-if="result" class="cd__result">{{ result }}</p>

                <div class="cd__actions">
                    <button class="cd__btn" type="button" @click="emit('close')">
                        Close
                    </button>
                    <button
                        v-if="result && !loading"
                        class="cd__btn"
                        type="button"
                        @click="copy"
                    >
                        Copy
                    </button>
                    <button
                        class="cd__btn cd__btn--primary"
                        type="button"
                        :disabled="loading"
                        @click="run"
                    >
                        {{ result || errorText ? "Regenerate" : "Describe" }}
                    </button>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.cd {
    position: fixed;
    inset: 0;
    z-index: 210;
    display: flex;
    align-items: center;
    justify-content: center;
}
.cd__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}
.cd__box {
    position: relative;
    width: min(500px, 92vw);
    max-height: 80vh;
    overflow-y: auto;
    padding: 20px;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    color: #fff;
}
.cd__head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
}
.cd__title {
    font-size: 15px;
    font-weight: 700;
}
.cd__range {
    font-size: 12px;
    font-family: ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.55);
}
.cd__engine {
    display: flex;
    gap: 8px;
    margin-bottom: 12px;
}
.cd__select {
    flex: 1;
    min-width: 0;
    padding: 7px 9px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.22);
    color: inherit;
    font-size: 12.5px;
    color-scheme: dark;
}
.cd__select option {
    background-color: #1a1c21;
    color: #f2f2f2;
}
.cd__loading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 16px 0;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.7);
}
.cd__spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.25);
    border-top-color: #c4a0ff;
    border-radius: 50%;
    animation: cd-spin 0.8s linear infinite;
}
@keyframes cd-spin {
    to {
        transform: rotate(360deg);
    }
}
.cd__result {
    margin: 4px 0 0;
    font-size: 13.5px;
    line-height: 1.6;
    color: rgba(255, 255, 255, 0.9);
    white-space: pre-wrap;
}
.cd__error {
    margin: 4px 0 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: #ffb454;
}
.cd__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
}
.cd__btn {
    padding: 8px 16px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}
.cd__btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
}
.cd__btn:disabled {
    opacity: 0.5;
    cursor: default;
}
.cd__btn--primary {
    border-color: rgba(196, 160, 255, 0.5);
    background: rgba(170, 130, 255, 0.28);
}
.cd__btn--primary:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.42);
}
.cd-fade-enter-active,
.cd-fade-leave-active {
    transition: opacity 0.15s ease;
}
.cd-fade-enter-from,
.cd-fade-leave-to {
    opacity: 0;
}
</style>
