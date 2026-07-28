import { computed, ref } from "vue";
import { loadUiState, saveUiState } from "./useUiStateStore";
import { useAiConfig } from "./useAiConfig";

// Config for AI subtitle generation, stored in the `subtitleAi` ui-state slice.
// Transcription needs an OpenAI-shaped /audio/transcriptions endpoint (OpenAI or
// Groq); translation reuses the chat AI (useAiConfig).

export const TRANSCRIBE_PROVIDERS = [
    "Sarvam",
    "Groq",
    "OpenAI",
    "Fireworks",
    "Lemonfox",
    "Custom",
] as const;
export type TranscribeProvider = (typeof TRANSCRIBE_PROVIDERS)[number];

// Sarvam is India-first (best for Indic) with its own API; the rest speak the
// OpenAI /audio/transcriptions shape.
export const isSarvam = (provider: string) => provider === "Sarvam";

// Languages Sarvam's own /translate endpoint can produce (mirrors the backend's
// sarvam_language_code mapping). When the target is one of these, accurate-timing
// translation is done by Sarvam itself — no separate chat provider needed.
const SARVAM_TRANSLATE_CODES = new Set([
    "hi",
    "bn",
    "kn",
    "ml",
    "mr",
    "or",
    "od",
    "pa",
    "ta",
    "te",
    "gu",
    "en",
]);
export const sarvamCanTranslateTo = (code: string) =>
    SARVAM_TRANSLATE_CODES.has(code.trim().toLowerCase());

const DEFAULT_BASE: Record<TranscribeProvider, string> = {
    Sarvam: "https://api.sarvam.ai",
    Groq: "https://api.groq.com/openai/v1",
    OpenAI: "https://api.openai.com/v1",
    Fireworks: "https://audio-prod.us-virginia-1.direct.fireworks.ai/v1",
    Lemonfox: "https://api.lemonfox.ai/v1",
    Custom: "",
};
const DEFAULT_MODEL: Record<TranscribeProvider, string> = {
    Sarvam: "saaras:v3",
    Groq: "whisper-large-v3",
    OpenAI: "whisper-1",
    Fireworks: "whisper-v3",
    Lemonfox: "whisper-1",
    Custom: "whisper-1",
};

// Providers whose SAME base+key can also do the chat translation step, so a
// user who set up Groq/OpenAI doesn't need a separate AI Enhance provider.
// (Value = a good default chat model at the same base URL.)
export const TRANSCRIBE_CHAT_MODEL: Partial<Record<TranscribeProvider, string>> = {
    Groq: "llama-3.3-70b-versatile",
    OpenAI: "gpt-4o-mini",
};

// Languages for the source + translation dropdowns (Whisper-supported), sorted
// alphabetically by label. Includes the major Indic languages.
export const SUBTITLE_LANGUAGES: { code: string; label: string }[] = (
    [
        { code: "ar", label: "Arabic" },
        { code: "as", label: "Assamese" },
        { code: "bn", label: "Bengali" },
        { code: "bg", label: "Bulgarian" },
        { code: "zh", label: "Chinese" },
        { code: "hr", label: "Croatian" },
        { code: "cs", label: "Czech" },
        { code: "da", label: "Danish" },
        { code: "nl", label: "Dutch" },
        { code: "en", label: "English" },
        { code: "fi", label: "Finnish" },
        { code: "fr", label: "French" },
        { code: "de", label: "German" },
        { code: "el", label: "Greek" },
        { code: "gu", label: "Gujarati" },
        { code: "he", label: "Hebrew" },
        { code: "hi", label: "Hindi" },
        { code: "hu", label: "Hungarian" },
        { code: "id", label: "Indonesian" },
        { code: "it", label: "Italian" },
        { code: "ja", label: "Japanese" },
        { code: "kn", label: "Kannada" },
        { code: "ko", label: "Korean" },
        { code: "ml", label: "Malayalam" },
        { code: "ms", label: "Malay" },
        { code: "mr", label: "Marathi" },
        { code: "ne", label: "Nepali" },
        { code: "no", label: "Norwegian" },
        { code: "fa", label: "Persian" },
        { code: "pl", label: "Polish" },
        { code: "pt", label: "Portuguese" },
        { code: "pa", label: "Punjabi" },
        { code: "ro", label: "Romanian" },
        { code: "ru", label: "Russian" },
        { code: "sr", label: "Serbian" },
        { code: "si", label: "Sinhala" },
        { code: "sk", label: "Slovak" },
        { code: "es", label: "Spanish" },
        { code: "sw", label: "Swahili" },
        { code: "sv", label: "Swedish" },
        { code: "ta", label: "Tamil" },
        { code: "te", label: "Telugu" },
        { code: "th", label: "Thai" },
        { code: "tl", label: "Tagalog" },
        { code: "tr", label: "Turkish" },
        { code: "uk", label: "Ukrainian" },
        { code: "ur", label: "Urdu" },
        { code: "vi", label: "Vietnamese" },
    ] as { code: string; label: string }[]
).sort((a, b) => a.label.localeCompare(b.label));

// Map a transcription provider to the equivalent AI-Enhance provider (so a key
// set there is reused). Groq has no AI-Enhance equivalent.
const AI_ENHANCE_EQUIVALENT: Record<string, string> = {
    OpenAI: "OpenAI",
    Custom: "Custom (OpenAI-compatible)",
};

type SubtitleAiShape = {
    provider: string;
    baseUrls: Record<string, string>;
    keys: Record<string, string>;
    models: Record<string, string>;
    sourceLanguage: string; // "auto" or a code
    translate: boolean;
    targetLanguage: string;
    // Sarvam→English only: transcribe (real word timestamps) + chat-translate,
    // instead of Sarvam's direct translate which carries no timing.
    accurateTiming: boolean;
};

const provider = ref<TranscribeProvider>("Groq");
const baseUrls = ref<Record<string, string>>({});
const keys = ref<Record<string, string>>({});
const models = ref<Record<string, string>>({});
const sourceLanguage = ref<string>("auto");
const translate = ref<boolean>(false);
const targetLanguage = ref<string>("en");
const accurateTiming = ref<boolean>(true);

let initialized = false;
let saveTimer: number | null = null;

const persist = () => {
    if (saveTimer) window.clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => {
        saveTimer = null;
        void saveUiState({
            subtitleAi: {
                provider: provider.value,
                baseUrls: { ...baseUrls.value },
                keys: { ...keys.value },
                models: { ...models.value },
                sourceLanguage: sourceLanguage.value,
                translate: translate.value,
                targetLanguage: targetLanguage.value,
                accurateTiming: accurateTiming.value,
            } satisfies SubtitleAiShape,
        });
    }, 300);
};

const load = async () => {
    if (initialized) return;
    initialized = true;
    const stored = await loadUiState<{ subtitleAi?: Partial<SubtitleAiShape> }>();
    const cfg = stored?.subtitleAi;
    if (!cfg) return;
    if (
        cfg.provider &&
        (TRANSCRIBE_PROVIDERS as readonly string[]).includes(cfg.provider)
    ) {
        provider.value = cfg.provider as TranscribeProvider;
    }
    baseUrls.value = { ...(cfg.baseUrls ?? {}) };
    keys.value = { ...(cfg.keys ?? {}) };
    models.value = { ...(cfg.models ?? {}) };
    if (cfg.sourceLanguage) sourceLanguage.value = cfg.sourceLanguage;
    if (typeof cfg.translate === "boolean") translate.value = cfg.translate;
    if (cfg.targetLanguage) targetLanguage.value = cfg.targetLanguage;
    if (typeof cfg.accurateTiming === "boolean") {
        accurateTiming.value = cfg.accurateTiming;
    }
};

export const useSubtitleAiConfig = () => {
    void load();
    const aiConfig = useAiConfig();

    const equivalent = (name: string) => AI_ENHANCE_EQUIVALENT[name] ?? "";

    // Fall back to the AI-Enhance key/base for the same provider (e.g. OpenAI),
    // so a key you already set there is reused for transcription.
    const currentBaseUrl = computed(
        () =>
            baseUrls.value[provider.value] ||
            aiConfig.baseUrlFor(equivalent(provider.value)) ||
            DEFAULT_BASE[provider.value],
    );
    const currentKey = computed(
        () =>
            keys.value[provider.value] ||
            aiConfig.keyFor(equivalent(provider.value)),
    );
    const currentModel = computed(
        () => models.value[provider.value] || DEFAULT_MODEL[provider.value],
    );
    const isCustom = computed(() => provider.value === "Custom");
    const baseUrlPlaceholder = computed(() =>
        isCustom.value
            ? "https://…/v1  (required)"
            : `Default: ${DEFAULT_BASE[provider.value]}`,
    );

    const setProvider = (value: string) => {
        if ((TRANSCRIBE_PROVIDERS as readonly string[]).includes(value)) {
            provider.value = value as TranscribeProvider;
            persist();
        }
    };
    const setKey = (value: string) => {
        keys.value = { ...keys.value, [provider.value]: value };
        persist();
    };
    const setBaseUrl = (value: string) => {
        baseUrls.value = { ...baseUrls.value, [provider.value]: value };
        persist();
    };
    const setModel = (value: string) => {
        models.value = { ...models.value, [provider.value]: value };
        persist();
    };
    const setSourceLanguage = (value: string) => {
        sourceLanguage.value = value;
        persist();
    };
    const setTranslate = (value: boolean) => {
        translate.value = value;
        persist();
    };
    const setTargetLanguage = (value: string) => {
        targetLanguage.value = value;
        persist();
    };
    const setAccurateTiming = (value: boolean) => {
        accurateTiming.value = value;
        persist();
    };

    return {
        provider,
        currentBaseUrl,
        currentKey,
        currentModel,
        isCustom,
        baseUrlPlaceholder,
        sourceLanguage,
        translate,
        targetLanguage,
        accurateTiming,
        setProvider,
        setKey,
        setBaseUrl,
        setModel,
        setSourceLanguage,
        setTranslate,
        setTargetLanguage,
        setAccurateTiming,
    };
};
