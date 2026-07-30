<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
    isGemini,
    isSarvam,
    isWhisper,
    sarvamCanTranslateTo,
    SUBTITLE_LANGUAGES,
    TRANSCRIBE_CHAT_MODEL,
    TRANSCRIBE_PROVIDERS,
    useSubtitleAiConfig,
} from "../composables/useSubtitleAiConfig";
import { useAiConfig } from "../composables/useAiConfig";

const props = defineProps<{
    open: boolean;
    path: string;
    abStart: number | null;
    abEnd: number | null;
    duration: number;
}>();
const emit = defineEmits<{
    (e: "close"): void;
    (e: "notify", message: string): void;
    (e: "loaded", payload: { path: string; lineCount: number }): void;
}>();

const sub = useSubtitleAiConfig();
const aiConfig = useAiConfig();

// --- range (whole video / A-B / manual From-To) for quick testing ---------
type RangeMode = "full" | "ab" | "manual";
const rangeMode = ref<RangeMode>("full");
const manualFrom = ref("");
const manualTo = ref("");

const sarvamSelected = computed(() => isSarvam(sub.provider.value));
const whisperSelected = computed(() => isWhisper(sub.provider.value));

const baseName = (p: string) => {
    const s = p.replace(/\\/g, "/");
    return s ? s.slice(s.lastIndexOf("/") + 1) : "";
};
const whisperExeName = computed(() => baseName(sub.whisperExe.value));
const whisperModelName = computed(() => baseName(sub.whisperModel.value));

const pickWhisperExe = async () => {
    const picked = await openDialog({ multiple: false });
    if (typeof picked === "string") sub.setWhisperExe(picked);
};
const pickWhisperModel = async () => {
    const picked = await openDialog({
        multiple: false,
        filters: [{ name: "Whisper model", extensions: ["bin", "gguf"] }],
    });
    if (typeof picked === "string") sub.setWhisperModel(picked);
};

// The accurate-timing choice only matters for Sarvam → English, the one path
// that would otherwise use Sarvam's timestamp-free direct translation.
const accurateRelevant = computed(
    () =>
        sarvamSelected.value &&
        sub.translate.value &&
        sub.targetLanguage.value === "en",
);

const hasAb = computed(
    () =>
        props.abStart != null &&
        props.abEnd != null &&
        props.abEnd > props.abStart,
);

const pad = (n: number) => String(n).padStart(2, "0");
const formatTime = (seconds: number | null | undefined): string => {
    if (seconds == null) return "";
    const s = Math.max(0, Math.floor(seconds));
    const hh = Math.floor(s / 3600);
    const mm = Math.floor((s % 3600) / 60);
    return hh > 0
        ? `${hh}:${pad(mm)}:${pad(s % 60)}`
        : `${mm}:${pad(s % 60)}`;
};
const parseTime = (raw: string): number | null => {
    const text = raw.trim();
    if (!text) return null;
    if (/^\d+(\.\d+)?$/.test(text)) return parseFloat(text);
    const parts = text.split(":").map((p) => p.trim());
    if (parts.some((p) => p === "" || Number.isNaN(Number(p)))) return null;
    let secs = 0;
    for (const part of parts) secs = secs * 60 + Number(part);
    return secs;
};

const effectiveRange = computed<{ start: number; end: number } | null>(() => {
    if (rangeMode.value === "ab" && hasAb.value) {
        return { start: props.abStart as number, end: props.abEnd as number };
    }
    if (rangeMode.value === "manual") {
        const start = parseTime(manualFrom.value);
        const end = parseTime(manualTo.value);
        if (start != null && end != null && end > start) return { start, end };
    }
    return null; // whole video
});

const generating = ref(false);
const statusText = ref("");
const progress = ref<{
    stage: string;
    done: number;
    total: number;
    wait?: number;
    quota?: string;
} | null>(null);
// The last successful run, so we can show where it saved and offer "Save a copy…".
const savedResult = ref<{ srtPath: string; fileName: string; lineCount: number } | null>(
    null,
);
const copyStatus = ref("");
let unlisten: UnlistenFn | null = null;

const progressPercent = computed(() => {
    const p = progress.value;
    if (!p || p.total <= 0) return 0;
    return Math.min(100, Math.round((p.done / p.total) * 100));
});
const progressLabel = computed(() => {
    const p = progress.value;
    if (!p) return "";
    if (p.stage === "transcribe") {
        return `Transcribing… chunk ${p.done + 1}/${p.total}`;
    }
    if (p.stage === "translate") {
        return `Translating… batch ${p.done + 1}/${p.total}`;
    }
    if (p.stage === "rate_wait") {
        const quota = p.quota ? ` — ${p.quota}` : "";
        return `Rate limit — resuming in ${p.wait ?? 0}s (${p.done}/${p.total} chunks done)${quota}`;
    }
    if (p.stage === "done") return "Finishing…";
    return "Working…";
});

onMounted(async () => {
    unlisten = await listen<{
        stage: string;
        done: number;
        total: number;
        wait?: number;
        quota?: string;
    }>(
        "ai_subtitles_progress",
        (event) => {
            progress.value = event.payload;
        },
    );
});
onUnmounted(() => {
    if (unlisten) unlisten();
});

watch(
    () => props.open,
    (open) => {
        if (open) {
            statusText.value = "";
            progress.value = null;
            savedResult.value = null;
            copyStatus.value = "";
        }
    },
);

const run = async () => {
    if (generating.value) return;
    if (whisperSelected.value) {
        if (!sub.whisperExe.value.trim() || !sub.whisperModel.value.trim()) {
            statusText.value =
                "Set the whisper-cli program and a model file (.bin) first.";
            return;
        }
    } else if (!sub.currentKey.value.trim()) {
        statusText.value = "Enter your transcription API key first.";
        return;
    }
    const engine = isWhisper(sub.provider.value)
        ? "whisper"
        : isSarvam(sub.provider.value)
          ? "sarvam"
          : isGemini(sub.provider.value)
            ? "gemini"
            : "openai";
    const translateTo = sub.translate.value ? sub.targetLanguage.value : null;
    // Local whisper translates to English itself (offline `--translate`).
    const whisperEnglish = engine === "whisper" && translateTo === "en";
    // Sarvam can translate Indic audio → English itself (no chat step needed),
    // but that path carries no timestamps. With "Accurate timing" we instead
    // transcribe (real word timings) and translate via Sarvam's own /translate.
    const sarvamDirectEnglish =
        engine === "sarvam" && translateTo === "en" && !sub.accurateTiming.value;
    // Accurate-timing translation Sarvam can do itself (Indic↔English), so no
    // separate chat provider is required for those targets.
    const sarvamNativeTranslate =
        engine === "sarvam" &&
        translateTo != null &&
        sarvamCanTranslateTo(translateTo);

    // Resolve the chat config for translation: prefer AI Enhance; otherwise
    // reuse the transcription provider's own chat (Groq/OpenAI). Fail fast so we
    // never transcribe (spending quota) only to fail on translation.
    let chatBase: string | null = null;
    let chatKey: string | null = null;
    let chatModel: string | null = null;
    // Gemini transcribes and translates in one multimodal call — no chat needed.
    if (
        translateTo &&
        !sarvamDirectEnglish &&
        !sarvamNativeTranslate &&
        !whisperEnglish &&
        engine !== "gemini"
    ) {
        if (aiConfig.currentKey.value.trim() && aiConfig.currentBaseUrl.value) {
            chatBase = aiConfig.currentBaseUrl.value;
            chatKey = aiConfig.currentKey.value;
            chatModel = aiConfig.currentModel.value;
        } else {
            const fallbackModel = TRANSCRIBE_CHAT_MODEL[sub.provider.value];
            if (
                fallbackModel &&
                sub.currentKey.value.trim() &&
                sub.currentBaseUrl.value
            ) {
                chatBase = sub.currentBaseUrl.value;
                chatKey = sub.currentKey.value;
                chatModel = fallbackModel;
            } else {
                statusText.value =
                    "Translation needs a chat model. Use Groq or OpenAI (their chat is reused automatically), configure AI Enhance, or uncheck Translate.";
                return;
            }
        }
    }

    if (rangeMode.value === "manual" && !effectiveRange.value) {
        statusText.value = "Enter a valid From and To (e.g. 12:00 and 14:00).";
        return;
    }
    const range = effectiveRange.value;
    generating.value = true;
    progress.value = null;
    statusText.value = "Preparing…";
    try {
        const result = await invoke<{
            srtPath: string;
            fileName: string;
            lineCount: number;
        }>("generate_ai_subtitles", {
            path: props.path,
            transcribeBase: sub.currentBaseUrl.value,
            transcribeKey: sub.currentKey.value,
            transcribeModel: sub.currentModel.value,
            sourceLanguage:
                sub.sourceLanguage.value === "auto"
                    ? null
                    : sub.sourceLanguage.value,
            translateTo,
            chatBase,
            chatKey,
            chatModel,
            rangeStart: range ? range.start : null,
            rangeEnd: range ? range.end : null,
            engine,
            accurateTiming: sub.accurateTiming.value,
            whisperExe: sub.whisperExe.value || null,
            whisperModel: sub.whisperModel.value || null,
        });
        emit("loaded", { path: result.srtPath, lineCount: result.lineCount });
        emit("notify", `Subtitles ready — ${result.lineCount} lines`);
        // Keep the dialog open on success so the user sees where it saved and
        // can save a copy elsewhere or generate another range.
        savedResult.value = result;
        statusText.value = "";
    } catch (error) {
        const message = String(error).replace(/^Error:\s*/, "");
        statusText.value =
            message === "Cancelled" ? "Cancelled." : message.slice(0, 240);
    } finally {
        generating.value = false;
        progress.value = null;
    }
};

// Save a copy of the generated .srt to a location the user picks.
const saveCopy = async () => {
    const src = savedResult.value;
    if (!src) return;
    try {
        const dest = await saveDialog({
            defaultPath: src.fileName,
            filters: [{ name: "Subtitles", extensions: ["srt"] }],
        });
        if (!dest) return;
        const contents = await invoke<string>("read_text_file", {
            path: src.srtPath,
        });
        await invoke("write_text_file", { path: dest, contents });
        copyStatus.value = "Saved.";
        emit("notify", "Subtitle file saved");
    } catch (error) {
        copyStatus.value = String(error)
            .replace(/^Error:\s*/, "")
            .slice(0, 160);
    }
};

const cancel = () => {
    if (generating.value) {
        void invoke("cancel_ai_subtitles");
        statusText.value = "Cancelling…";
    }
};
</script>

<template>
    <transition name="subai-fade">
        <div
            v-if="open"
            class="subai"
            @keydown.esc.stop.prevent="generating ? cancel() : emit('close')"
        >
            <div class="subai__backdrop" @click="generating ? null : emit('close')" />
            <div class="subai__box" role="dialog" aria-label="Generate subtitles">
                <div class="subai__title">Generate Subtitles (AI)</div>
                <p class="subai__hint">
                    Transcribes the audio into a timed subtitle track. With Groq
                    or OpenAI, the same key also powers translation — no separate
                    setup needed.
                </p>
                <p v-if="sarvamSelected" class="subai__note">
                    Sarvam is tuned for Indian languages (best for Telugu, Tamil,
                    etc.). For English subtitles, use “Accurate timing” below for
                    real sync (recommended for movies). Get a key at
                    dashboard.sarvam.ai.
                </p>
                <p v-if="whisperSelected" class="subai__note">
                    Runs fully offline on your machine — no API key or quota. Point
                    it at a whisper.cpp program (whisper-cli) and a ggml model file.
                    Get models from huggingface.co/ggerganov/whisper.cpp — bigger
                    models are more accurate but slower. “Translate to English”
                    works offline; other languages still need a chat translator.
                </p>

                <div class="subai__grid">
                    <span class="subai__label">Provider</span>
                    <select
                        :value="sub.provider.value"
                        class="subai__control"
                        :disabled="generating"
                        @change="
                            sub.setProvider(
                                ($event.target as HTMLSelectElement).value,
                            )
                        "
                    >
                        <option v-for="p in TRANSCRIBE_PROVIDERS" :key="p" :value="p">
                            {{ p }}
                        </option>
                    </select>

                    <template v-if="!whisperSelected">
                        <span class="subai__label">API Key</span>
                        <input
                            :value="sub.currentKey.value"
                            class="subai__control"
                            type="password"
                            autocomplete="off"
                            spellcheck="false"
                            :disabled="generating"
                            placeholder="Paste this provider's API key"
                            @input="sub.setKey(($event.target as HTMLInputElement).value)"
                        />

                        <span class="subai__label">Base URL</span>
                        <input
                            :value="sub.currentBaseUrl.value"
                            class="subai__control"
                            type="text"
                            spellcheck="false"
                            :disabled="generating"
                            :placeholder="sub.baseUrlPlaceholder.value"
                            @input="
                                sub.setBaseUrl(($event.target as HTMLInputElement).value)
                            "
                        />

                        <span class="subai__label">Model</span>
                        <input
                            :value="sub.currentModel.value"
                            class="subai__control"
                            type="text"
                            spellcheck="false"
                            :disabled="generating"
                            @input="
                                sub.setModel(($event.target as HTMLInputElement).value)
                            "
                        />
                    </template>
                    <template v-else>
                        <span class="subai__label">Whisper program</span>
                        <div class="subai__file">
                            <span
                                class="subai__file-name"
                                :title="sub.whisperExe.value"
                            >
                                {{ whisperExeName || "whisper-cli(.exe)" }}
                            </span>
                            <button
                                class="subai__btn"
                                type="button"
                                :disabled="generating"
                                @click="pickWhisperExe"
                            >
                                Browse…
                            </button>
                        </div>

                        <span class="subai__label">Model file</span>
                        <div class="subai__file">
                            <span
                                class="subai__file-name"
                                :title="sub.whisperModel.value"
                            >
                                {{ whisperModelName || "ggml-*.bin" }}
                            </span>
                            <button
                                class="subai__btn"
                                type="button"
                                :disabled="generating"
                                @click="pickWhisperModel"
                            >
                                Browse…
                            </button>
                        </div>
                    </template>

                    <span class="subai__label">Spoken language</span>
                    <select
                        :value="sub.sourceLanguage.value"
                        class="subai__control"
                        :disabled="generating"
                        @change="
                            sub.setSourceLanguage(
                                ($event.target as HTMLSelectElement).value,
                            )
                        "
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
                </div>

                <label class="subai__translate">
                    <input
                        :checked="sub.translate.value"
                        type="checkbox"
                        :disabled="generating"
                        @change="
                            sub.setTranslate(
                                ($event.target as HTMLInputElement).checked,
                            )
                        "
                    />
                    Translate to
                    <select
                        :value="sub.targetLanguage.value"
                        class="subai__control subai__control--inline"
                        :disabled="generating || !sub.translate.value"
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
                </label>

                <label v-if="accurateRelevant" class="subai__accurate">
                    <input
                        :checked="sub.accurateTiming.value"
                        type="checkbox"
                        :disabled="generating"
                        @change="
                            sub.setAccurateTiming(
                                ($event.target as HTMLInputElement).checked,
                            )
                        "
                    />
                    <span>
                        Accurate timing
                        <small>
                            Transcribes with real word-level timestamps, then
                            translates with Sarvam's own translator (same key — no
                            extra setup). Best sync — recommended for movies.
                            Uncheck for Sarvam's direct English (nicer phrasing,
                            looser timing).
                        </small>
                    </span>
                </label>

                <div class="subai__range">
                    <div class="subai__range-title">Range</div>
                    <label class="subai__radio">
                        <input
                            type="radio"
                            :checked="rangeMode === 'full'"
                            :disabled="generating"
                            @change="rangeMode = 'full'"
                        />
                        Whole video
                    </label>
                    <label v-if="hasAb" class="subai__radio">
                        <input
                            type="radio"
                            :checked="rangeMode === 'ab'"
                            :disabled="generating"
                            @change="rangeMode = 'ab'"
                        />
                        A-B range ({{ formatTime(abStart) }}–{{ formatTime(abEnd) }})
                    </label>
                    <label class="subai__radio">
                        <input
                            type="radio"
                            :checked="rangeMode === 'manual'"
                            :disabled="generating"
                            @change="rangeMode = 'manual'"
                        />
                        From / To
                    </label>
                    <div v-if="rangeMode === 'manual'" class="subai__manual">
                        <input
                            v-model="manualFrom"
                            class="subai__control subai__control--time"
                            type="text"
                            :disabled="generating"
                            placeholder="From (12:00)"
                        />
                        <span class="subai__arrow">→</span>
                        <input
                            v-model="manualTo"
                            class="subai__control subai__control--time"
                            type="text"
                            :disabled="generating"
                            placeholder="To (14:00)"
                        />
                    </div>
                    <p class="subai__range-hint">
                        A short range is great for testing — it uses little of your
                        provider's quota.
                    </p>
                </div>

                <div v-if="generating" class="subai__progress">
                    <div class="subai__bar">
                        <div
                            class="subai__bar-fill"
                            :style="{ width: `${progressPercent}%` }"
                        />
                    </div>
                    <div class="subai__progress-label">
                        {{ progressLabel || "Preparing…" }}
                    </div>
                </div>

                <p v-if="statusText" class="subai__status">{{ statusText }}</p>

                <div v-if="savedResult && !generating" class="subai__done">
                    <div class="subai__done-title">
                        ✓ Saved {{ savedResult.lineCount }} subtitle lines
                    </div>
                    <div class="subai__done-path" :title="savedResult.srtPath">
                        {{ savedResult.fileName }}
                    </div>
                    <div class="subai__done-hint">
                        Loaded into the player and saved next to the video.
                        Generating another time-range adds to this file.
                    </div>
                    <div v-if="copyStatus" class="subai__done-copy">{{ copyStatus }}</div>
                </div>

                <div class="subai__actions">
                    <button
                        v-if="generating"
                        class="subai__btn"
                        type="button"
                        @click="cancel"
                    >
                        Cancel
                    </button>
                    <template v-else>
                        <button class="subai__btn" type="button" @click="emit('close')">
                            Close
                        </button>
                        <button
                            v-if="savedResult"
                            class="subai__btn"
                            type="button"
                            @click="saveCopy"
                        >
                            Save a copy…
                        </button>
                        <button
                            class="subai__btn subai__btn--primary"
                            type="button"
                            :disabled="
                                whisperSelected
                                    ? !sub.whisperExe.value || !sub.whisperModel.value
                                    : !sub.currentKey.value
                            "
                            @click="run"
                        >
                            {{ savedResult ? "Generate more" : "Generate" }}
                        </button>
                    </template>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.subai {
    position: fixed;
    inset: 0;
    z-index: 210;
    display: flex;
    align-items: center;
    justify-content: center;
}
.subai__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}
.subai__box {
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
.subai__title {
    font-size: 15px;
    font-weight: 700;
}
.subai__hint {
    margin: 6px 0 14px;
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.55);
}
.subai__note {
    margin: -8px 0 14px;
    padding: 8px 10px;
    border-radius: 8px;
    background: rgba(120, 200, 140, 0.12);
    border: 1px solid rgba(120, 200, 140, 0.3);
    font-size: 12px;
    line-height: 1.5;
    color: rgba(200, 240, 210, 0.9);
}
.subai__grid {
    display: grid;
    grid-template-columns: 116px 1fr;
    align-items: center;
    gap: 8px 12px;
}
.subai__label {
    font-size: 12.5px;
    font-weight: 600;
}
.subai__control {
    padding: 8px 10px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.22);
    color: inherit;
    font-size: 13px;
    color-scheme: dark;
    min-width: 0;
}
.subai__control option {
    background-color: #1a1c21;
    color: #f2f2f2;
}
.subai__control--inline {
    padding: 5px 8px;
}
.subai__file {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
}
.subai__file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    font-family: ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.72);
}
.subai__translate {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 14px 0 4px;
    font-size: 12.5px;
    font-weight: 600;
}
.subai__accurate {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 10px 0 2px;
    font-size: 12.5px;
    font-weight: 600;
}
.subai__accurate input {
    margin-top: 2px;
}
.subai__accurate small {
    display: block;
    margin-top: 3px;
    font-weight: 400;
    font-size: 11.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.5);
}
.subai__range {
    margin-top: 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
}
.subai__range-title {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.45);
}
.subai__radio {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
}
.subai__manual {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 2px 0 2px 24px;
}
.subai__control--time {
    width: 130px;
}
.subai__arrow {
    color: rgba(255, 255, 255, 0.5);
}
.subai__range-hint {
    margin: 2px 0 0;
    font-size: 11.5px;
    color: rgba(255, 255, 255, 0.45);
}

.subai__progress {
    margin-top: 14px;
}
.subai__bar {
    height: 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.12);
    overflow: hidden;
}
.subai__bar-fill {
    height: 100%;
    background: linear-gradient(90deg, #7c5cff, #c4a0ff);
    transition: width 0.3s ease;
}
.subai__progress-label {
    margin-top: 6px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.7);
}
.subai__status {
    margin: 10px 0 0;
    font-size: 12px;
    color: #ffb454;
}
.subai__done {
    margin: 12px 0 0;
    padding: 10px 12px;
    border-radius: 8px;
    background: rgba(120, 200, 140, 0.12);
    border: 1px solid rgba(120, 200, 140, 0.3);
}
.subai__done-title {
    font-size: 13px;
    font-weight: 700;
    color: rgba(200, 240, 210, 0.95);
}
.subai__done-path {
    margin-top: 4px;
    font-size: 12px;
    font-family: ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.75);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.subai__done-hint {
    margin-top: 6px;
    font-size: 11.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.55);
}
.subai__done-copy {
    margin-top: 6px;
    font-size: 12px;
    color: rgba(200, 240, 210, 0.9);
}
.subai__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
}
.subai__btn {
    padding: 8px 16px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}
.subai__btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
}
.subai__btn:disabled {
    opacity: 0.5;
    cursor: default;
}
.subai__btn--primary {
    border-color: rgba(196, 160, 255, 0.5);
    background: rgba(170, 130, 255, 0.28);
}
.subai__btn--primary:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.42);
}
.subai-fade-enter-active,
.subai-fade-leave-active {
    transition: opacity 0.15s ease;
}
.subai-fade-enter-from,
.subai-fade-leave-to {
    opacity: 0;
}
</style>
