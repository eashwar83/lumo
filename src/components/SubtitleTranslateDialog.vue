<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
    isSarvam,
    SUBTITLE_LANGUAGES,
    TRANSCRIBE_PROVIDERS,
    useSubtitleAiConfig,
} from "../composables/useSubtitleAiConfig";

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
    (e: "close"): void;
    (e: "notify", message: string): void;
    (e: "loaded", payload: { path: string; lineCount: number }): void;
}>();

const sub = useSubtitleAiConfig();

const srtPath = ref("");
const generating = ref(false);
const statusText = ref("");
const progress = ref<{ stage: string; done: number; total: number } | null>(null);
let unlisten: UnlistenFn | null = null;

const fileName = computed(() => {
    const p = srtPath.value.replace(/\\/g, "/");
    return p ? p.slice(p.lastIndexOf("/") + 1) : "";
});

const progressPercent = computed(() => {
    const p = progress.value;
    if (!p || p.total <= 0) return 0;
    return Math.min(100, Math.round((p.done / p.total) * 100));
});

onMounted(async () => {
    unlisten = await listen<{ stage: string; done: number; total: number }>(
        "ai_subtitles_progress",
        (event) => {
            if (generating.value) progress.value = event.payload;
        },
    );
});
onUnmounted(() => {
    if (unlisten) unlisten();
});

// On open, try to prefill with the currently-loaded external subtitle file.
watch(
    () => props.open,
    async (open) => {
        if (!open) return;
        statusText.value = "";
        progress.value = null;
        try {
            const current = await invoke<string | null>("mpv_get_property_string", {
                name: "current-tracks/sub/external-filename",
            });
            if (current && /\.(srt|ass|ssa|vtt)$/i.test(current)) {
                srtPath.value = current;
            }
        } catch {
            /* no current external sub — leave empty */
        }
    },
);

const pickFile = async () => {
    const picked = await openDialog({
        multiple: false,
        filters: [{ name: "Subtitles", extensions: ["srt", "vtt", "ass", "ssa"] }],
    });
    if (typeof picked === "string") srtPath.value = picked;
};

const run = async () => {
    if (generating.value) return;
    if (!srtPath.value.trim()) {
        statusText.value = "Choose a subtitle file to translate.";
        return;
    }
    const target = sub.targetLanguage.value;
    const plan = sub.resolveTranslation(target);
    if ("error" in plan) {
        statusText.value = plan.error;
        return;
    }
    const engine = isSarvam(sub.provider.value) ? "sarvam" : "openai";

    generating.value = true;
    progress.value = null;
    statusText.value = "Translating…";
    try {
        const result = await invoke<{
            srtPath: string;
            fileName: string;
            lineCount: number;
        }>("translate_subtitle_file", {
            srtPath: srtPath.value,
            targetLanguage: target,
            engine,
            transcribeBase: sub.currentBaseUrl.value,
            transcribeKey: sub.currentKey.value,
            sourceLanguage:
                sub.sourceLanguage.value === "auto" ? null : sub.sourceLanguage.value,
            chatBase: plan.mode === "chat" ? plan.chatBase : null,
            chatKey: plan.mode === "chat" ? plan.chatKey : null,
            chatModel: plan.mode === "chat" ? plan.chatModel : null,
        });
        emit("loaded", { path: result.srtPath, lineCount: result.lineCount });
        emit("notify", `Translated subtitles ready — ${result.lineCount} lines`);
        emit("close");
    } catch (error) {
        const message = String(error).replace(/^Error:\s*/, "");
        statusText.value = message.slice(0, 240);
    } finally {
        generating.value = false;
        progress.value = null;
    }
};
</script>

<template>
    <transition name="subtr-fade">
        <div
            v-if="open"
            class="subtr"
            @keydown.esc.stop.prevent="generating ? null : emit('close')"
        >
            <div class="subtr__backdrop" @click="generating ? null : emit('close')" />
            <div class="subtr__box" role="dialog" aria-label="Translate subtitles">
                <div class="subtr__title">Translate Subtitles (AI)</div>
                <p class="subtr__hint">
                    Translates an existing subtitle file into another language,
                    keeping the original timings. Uses Sarvam for Indic ↔ English,
                    otherwise your chat AI (Gemini / Groq / OpenAI / AI Enhance).
                </p>

                <div class="subtr__grid">
                    <span class="subtr__label">Subtitle file</span>
                    <div class="subtr__file">
                        <span class="subtr__file-name" :title="srtPath">
                            {{ fileName || "No file chosen" }}
                        </span>
                        <button
                            class="subtr__btn"
                            type="button"
                            :disabled="generating"
                            @click="pickFile"
                        >
                            Choose…
                        </button>
                    </div>

                    <span class="subtr__label">Translate with</span>
                    <select
                        :value="sub.provider.value"
                        class="subtr__control"
                        :disabled="generating"
                        @change="
                            sub.setProvider(($event.target as HTMLSelectElement).value)
                        "
                    >
                        <option v-for="p in TRANSCRIBE_PROVIDERS" :key="p" :value="p">
                            {{ p }}
                        </option>
                    </select>

                    <span class="subtr__label">Translate to</span>
                    <select
                        :value="sub.targetLanguage.value"
                        class="subtr__control"
                        :disabled="generating"
                        @change="
                            sub.setTargetLanguage(
                                ($event.target as HTMLSelectElement).value,
                            )
                        "
                    >
                        <option
                            v-for="lang in SUBTITLE_LANGUAGES"
                            :key="lang.code"
                            :value="lang.code"
                        >
                            {{ lang.label }}
                        </option>
                    </select>
                </div>

                <div v-if="generating" class="subtr__progress">
                    <div class="subtr__bar">
                        <div
                            class="subtr__bar-fill"
                            :style="{ width: `${progressPercent}%` }"
                        />
                    </div>
                    <div class="subtr__progress-label">Translating…</div>
                </div>

                <p v-if="statusText" class="subtr__status">{{ statusText }}</p>

                <div class="subtr__actions">
                    <button class="subtr__btn" type="button" @click="emit('close')">
                        {{ generating ? "Hide" : "Close" }}
                    </button>
                    <button
                        class="subtr__btn subtr__btn--primary"
                        type="button"
                        :disabled="generating || !srtPath || !sub.currentKey.value"
                        @click="run"
                    >
                        Translate
                    </button>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.subtr {
    position: fixed;
    inset: 0;
    z-index: 210;
    display: flex;
    align-items: center;
    justify-content: center;
}
.subtr__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}
.subtr__box {
    position: relative;
    width: min(500px, 92vw);
    max-height: 90vh;
    overflow-y: auto;
    padding: 20px;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    color: #fff;
}
.subtr__title {
    font-size: 15px;
    font-weight: 700;
}
.subtr__hint {
    margin: 6px 0 14px;
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.55);
}
.subtr__grid {
    display: grid;
    grid-template-columns: 110px 1fr;
    align-items: center;
    gap: 10px 12px;
}
.subtr__label {
    font-size: 12.5px;
    font-weight: 600;
}
.subtr__file {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
}
.subtr__file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    color: rgba(255, 255, 255, 0.75);
}
.subtr__control {
    padding: 8px 10px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.22);
    color: inherit;
    font-size: 13px;
    color-scheme: dark;
    min-width: 0;
}
.subtr__control option {
    background-color: #1a1c21;
    color: #f2f2f2;
}
.subtr__progress {
    margin-top: 14px;
}
.subtr__bar {
    height: 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
    overflow: hidden;
}
.subtr__bar-fill {
    height: 100%;
    background: linear-gradient(90deg, #7c5cff, #c4a0ff);
    transition: width 0.3s ease;
}
.subtr__progress-label {
    margin-top: 6px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.7);
}
.subtr__status {
    margin: 10px 0 0;
    font-size: 12px;
    color: #ffb454;
}
.subtr__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
}
.subtr__btn {
    padding: 8px 16px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}
.subtr__btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
}
.subtr__btn:disabled {
    opacity: 0.5;
    cursor: default;
}
.subtr__btn--primary {
    border-color: rgba(196, 160, 255, 0.5);
    background: rgba(170, 130, 255, 0.28);
}
.subtr__btn--primary:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.42);
}
.subtr-fade-enter-active,
.subtr-fade-leave-active {
    transition: opacity 0.15s ease;
}
.subtr-fade-enter-from,
.subtr-fade-leave-to {
    opacity: 0;
}
</style>
