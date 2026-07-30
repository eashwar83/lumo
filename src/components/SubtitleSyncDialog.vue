<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
    isGemini,
    isSarvam,
    isWhisper,
    SUBTITLE_LANGUAGES,
    TRANSCRIBE_PROVIDERS,
    useSubtitleAiConfig,
} from "../composables/useSubtitleAiConfig";

const props = defineProps<{ open: boolean; path: string }>();
const emit = defineEmits<{
    (e: "close"): void;
    (e: "notify", message: string): void;
    (e: "loaded", payload: { path: string; lineCount: number }): void;
}>();

const sub = useSubtitleAiConfig();

type Mode = "quick" | "smart";
const mode = ref<Mode>("quick");
const srtPath = ref("");
const subtitleLanguage = ref("en");
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
const progressLabel = computed(() => {
    const p = progress.value;
    if (!p) return "";
    if (p.stage === "transcribe") return `Analysing… ${p.done + 1}/${p.total}`;
    if (p.stage === "translate") return `Filling gaps… ${p.done + 1}/${p.total}`;
    if (p.stage === "done") return "Finishing…";
    return "Working…";
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
            if (current && /\.(srt|ass|ssa|vtt)$/i.test(current)) srtPath.value = current;
        } catch {
            /* no current external sub */
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
        statusText.value = "Choose a subtitle file to sync.";
        return;
    }
    generating.value = true;
    progress.value = null;
    statusText.value = mode.value === "quick" ? "Analysing audio…" : "Preparing…";
    try {
        let result: { srtPath: string; fileName: string; lineCount: number };
        if (mode.value === "quick") {
            result = await invoke("quick_sync_subtitle", {
                srtPath: srtPath.value,
                videoPath: props.path,
            });
        } else {
            const engine = isWhisper(sub.provider.value)
                ? "whisper"
                : isSarvam(sub.provider.value)
                  ? "sarvam"
                  : isGemini(sub.provider.value)
                    ? "gemini"
                    : "openai";
            if (engine === "whisper" && (!sub.whisperExe.value || !sub.whisperModel.value)) {
                statusText.value = "Set the whisper-cli program and model first (Generate dialog).";
                generating.value = false;
                return;
            }
            if (engine !== "whisper" && !sub.currentKey.value.trim()) {
                statusText.value = "Set the provider's API key first (Generate dialog).";
                generating.value = false;
                return;
            }
            // Resolve a chat translator for engines that don't translate inline.
            const plan = sub.resolveTranslation(subtitleLanguage.value);
            const chat = "mode" in plan && plan.mode === "chat" ? plan : null;
            result = await invoke("smart_sync_subtitle", {
                srtPath: srtPath.value,
                videoPath: props.path,
                engine,
                transcribeBase: sub.currentBaseUrl.value,
                transcribeKey: sub.currentKey.value,
                transcribeModel: sub.currentModel.value,
                sourceLanguage:
                    sub.sourceLanguage.value === "auto" ? null : sub.sourceLanguage.value,
                subLanguage: subtitleLanguage.value,
                chatBase: chat?.chatBase ?? null,
                chatKey: chat?.chatKey ?? null,
                chatModel: chat?.chatModel ?? null,
                whisperExe: sub.whisperExe.value || null,
                whisperModel: sub.whisperModel.value || null,
            });
        }
        emit("loaded", { path: result.srtPath, lineCount: result.lineCount });
        emit("notify", `Subtitles synced — ${result.lineCount} lines`);
        emit("close");
    } catch (error) {
        const message = String(error).replace(/^Error:\s*/, "");
        statusText.value = message === "Cancelled" ? "Cancelled." : message.slice(0, 260);
    } finally {
        generating.value = false;
        progress.value = null;
    }
};

const cancel = () => {
    if (generating.value) void invoke("cancel_ai_subtitles");
};
</script>

<template>
    <transition name="subsync-fade">
        <div
            v-if="open"
            class="subsync"
            @keydown.esc.stop.prevent="generating ? cancel() : emit('close')"
        >
            <div class="subsync__backdrop" @click="generating ? null : emit('close')" />
            <div class="subsync__box" role="dialog" aria-label="Sync subtitles">
                <div class="subsync__title">Sync Subtitles (AI)</div>
                <p class="subsync__hint">
                    Re-times a subtitle file made for a different release of this
                    video. Quick fixes a global offset / frame-rate mismatch. Smart
                    also handles cut or added scenes — dropping extra lines and
                    generating subtitles for gaps.
                </p>

                <div class="subsync__grid">
                    <span class="subsync__label">Subtitle file</span>
                    <div class="subsync__file">
                        <span class="subsync__file-name" :title="srtPath">
                            {{ fileName || "No file chosen" }}
                        </span>
                        <button
                            class="subsync__btn"
                            type="button"
                            :disabled="generating"
                            @click="pickFile"
                        >
                            Choose…
                        </button>
                    </div>
                </div>

                <div class="subsync__modes">
                    <label class="subsync__radio">
                        <input
                            type="radio"
                            :checked="mode === 'quick'"
                            :disabled="generating"
                            @change="mode = 'quick'"
                        />
                        <span>
                            <strong>Quick sync</strong>
                            <small>Global offset + frame-rate. Instant, offline, free.</small>
                        </span>
                    </label>
                    <label class="subsync__radio">
                        <input
                            type="radio"
                            :checked="mode === 'smart'"
                            :disabled="generating"
                            @change="mode = 'smart'"
                        />
                        <span>
                            <strong>Smart sync (AI)</strong>
                            <small>
                                Handles cut/added scenes — drops extras, fills gaps.
                                Uses your AI engine.
                            </small>
                        </span>
                    </label>
                </div>

                <div v-if="mode === 'smart'" class="subsync__grid">
                    <span class="subsync__label">AI engine</span>
                    <select
                        :value="sub.provider.value"
                        class="subsync__control"
                        :disabled="generating"
                        @change="sub.setProvider(($event.target as HTMLSelectElement).value)"
                    >
                        <option v-for="p in TRANSCRIBE_PROVIDERS" :key="p" :value="p">
                            {{ p }}
                        </option>
                    </select>

                    <span class="subsync__label">Audio language</span>
                    <select
                        :value="sub.sourceLanguage.value"
                        class="subsync__control"
                        :disabled="generating"
                        @change="sub.setSourceLanguage(($event.target as HTMLSelectElement).value)"
                    >
                        <option value="auto">Auto-detect</option>
                        <option
                            v-for="lang in SUBTITLE_LANGUAGES"
                            :key="lang.code"
                            :value="lang.code"
                        >
                            {{ lang.label }}
                        </option>
                    </select>

                    <span class="subsync__label">Subtitle language</span>
                    <select
                        v-model="subtitleLanguage"
                        class="subsync__control"
                        :disabled="generating"
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

                <div v-if="generating" class="subsync__progress">
                    <div class="subsync__bar">
                        <div class="subsync__bar-fill" :style="{ width: `${progressPercent}%` }" />
                    </div>
                    <div class="subsync__progress-label">
                        {{ progressLabel || "Working…" }}
                    </div>
                </div>

                <p v-if="statusText" class="subsync__status">{{ statusText }}</p>

                <div class="subsync__actions">
                    <button
                        v-if="generating"
                        class="subsync__btn"
                        type="button"
                        @click="cancel"
                    >
                        Cancel
                    </button>
                    <template v-else>
                        <button class="subsync__btn" type="button" @click="emit('close')">
                            Close
                        </button>
                        <button
                            class="subsync__btn subsync__btn--primary"
                            type="button"
                            :disabled="!srtPath"
                            @click="run"
                        >
                            {{ mode === "quick" ? "Sync" : "Smart sync" }}
                        </button>
                    </template>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.subsync {
    position: fixed;
    inset: 0;
    z-index: 210;
    display: flex;
    align-items: center;
    justify-content: center;
}
.subsync__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}
.subsync__box {
    position: relative;
    width: min(520px, 92vw);
    max-height: 90vh;
    overflow-y: auto;
    padding: 20px;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    color: #fff;
}
.subsync__title {
    font-size: 15px;
    font-weight: 700;
}
.subsync__hint {
    margin: 6px 0 14px;
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.55);
}
.subsync__grid {
    display: grid;
    grid-template-columns: 120px 1fr;
    align-items: center;
    gap: 10px 12px;
    margin-bottom: 4px;
}
.subsync__label {
    font-size: 12.5px;
    font-weight: 600;
}
.subsync__file {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
}
.subsync__file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    color: rgba(255, 255, 255, 0.75);
}
.subsync__control {
    padding: 8px 10px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.22);
    color: inherit;
    font-size: 13px;
    color-scheme: dark;
    min-width: 0;
}
.subsync__control option {
    background-color: #1a1c21;
    color: #f2f2f2;
}
.subsync__modes {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 12px 0;
}
.subsync__radio {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    font-size: 13px;
    cursor: pointer;
}
.subsync__radio input {
    margin-top: 3px;
}
.subsync__radio small {
    display: block;
    margin-top: 2px;
    font-weight: 400;
    font-size: 11.5px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.5);
}
.subsync__progress {
    margin-top: 14px;
}
.subsync__bar {
    height: 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
    overflow: hidden;
}
.subsync__bar-fill {
    height: 100%;
    background: linear-gradient(90deg, #7c5cff, #c4a0ff);
    transition: width 0.3s ease;
}
.subsync__progress-label {
    margin-top: 6px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.7);
}
.subsync__status {
    margin: 10px 0 0;
    font-size: 12px;
    color: #ffb454;
}
.subsync__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
}
.subsync__btn {
    padding: 8px 16px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}
.subsync__btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
}
.subsync__btn:disabled {
    opacity: 0.5;
    cursor: default;
}
.subsync__btn--primary {
    border-color: rgba(196, 160, 255, 0.5);
    background: rgba(170, 130, 255, 0.28);
}
.subsync__btn--primary:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.42);
}
.subsync-fade-enter-active,
.subsync-fade-leave-active {
    transition: opacity 0.15s ease;
}
.subsync-fade-enter-from,
.subsync-fade-leave-to {
    opacity: 0;
}
</style>
