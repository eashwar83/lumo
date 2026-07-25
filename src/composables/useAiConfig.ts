import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { loadUiState, saveUiState } from "./useUiStateStore";
import {
    CURATED_AI_MODELS,
    defaultModelFor,
    type AiProvider,
} from "../constants/aiModels";

// Single source of truth for the cloud-AI configuration, shared by the Settings
// panel and the correction prompt. Stored in the `aiConfig` ui-state slice:
//
//   provider  — currently selected provider
//   baseUrl   — Custom provider endpoint
//   keys      — { provider: apiKey }      (per-provider)
//   models    — { provider: modelId }     (per-provider chosen model)
//   fetched   — { provider: string[] }    (models discovered via the API)
//
// A module-level singleton so every caller sees the same reactive state.

export type AiPromptEntry = {
    prompt: string;
    provider: string;
    model: string;
    at: number;
};

type AiConfigShape = {
    provider: string;
    // Per-provider base URL override (regional / workspace / self-hosted).
    baseUrls: Record<string, string>;
    keys: Record<string, string>;
    models: Record<string, string>;
    fetched: Record<string, string[]>;
    // Recent prompt + provider + model used, newest first (for reuse).
    promptHistory: AiPromptEntry[];
};

const MAX_PROMPT_HISTORY = 20;

const provider = ref<string>("Gemini");
const baseUrls = ref<Record<string, string>>({});
const keys = ref<Record<string, string>>({});
const models = ref<Record<string, string>>({});
const fetched = ref<Record<string, string[]>>({});
const promptHistory = ref<AiPromptEntry[]>([]);

let initialized = false;
let saveTimer: number | null = null;

const persist = () => {
    if (saveTimer) window.clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => {
        saveTimer = null;
        void saveUiState({
            aiConfig: {
                provider: provider.value,
                baseUrls: { ...baseUrls.value },
                keys: { ...keys.value },
                models: { ...models.value },
                fetched: { ...fetched.value },
                promptHistory: [...promptHistory.value],
            } satisfies AiConfigShape,
        });
    }, 300);
};

// One-time migration from the earlier flat/settings-group storage so keys the
// user already entered aren't lost.
type LegacySettings = {
    aiConfig?: Partial<AiConfigShape>;
    settings?: { groups?: Array<{ items?: Array<{ label: string; value: string }> }> };
};
const migrateFromSettings = (stored: LegacySettings | null) => {
    if (!stored?.settings?.groups) return;
    const items = stored.settings.groups.flatMap((g) => g.items ?? []);
    const byLabel = (label: string) =>
        items.find((i) => i.label === label)?.value ?? "";
    const parseMap = (raw: string): Record<string, string> => {
        try {
            return raw ? (JSON.parse(raw) as Record<string, string>) : {};
        } catch {
            return {};
        }
    };
    const parseListMap = (raw: string): Record<string, string[]> => {
        try {
            return raw ? (JSON.parse(raw) as Record<string, string[]>) : {};
        } catch {
            return {};
        }
    };
    const legacyProvider = byLabel("AI_PROVIDER");
    if (legacyProvider) provider.value = legacyProvider;
    const legacyBase = byLabel("AI_BASE_URL");
    if (legacyBase) baseUrls.value["Custom (OpenAI-compatible)"] = legacyBase;
    keys.value = parseMap(byLabel("AI_API_KEYS"));
    models.value = parseMap(byLabel("AI_MODELS_SELECTED"));
    fetched.value = parseListMap(byLabel("AI_FETCHED_MODELS"));
    // The old flat key/model, if only those were set.
    const flatKey = byLabel("AI_API_KEY");
    if (flatKey && !keys.value[provider.value]) {
        keys.value[provider.value] = flatKey;
    }
    const flatModel = byLabel("AI_MODEL");
    if (flatModel && !models.value[provider.value]) {
        models.value[provider.value] = flatModel;
    }
};

const load = async () => {
    if (initialized) return;
    initialized = true;
    const stored = await loadUiState<LegacySettings>();
    const cfg = stored?.aiConfig;
    if (cfg && (cfg.provider || cfg.keys)) {
        provider.value = cfg.provider ?? "Gemini";
        baseUrls.value = { ...(cfg.baseUrls ?? {}) };
        keys.value = { ...(cfg.keys ?? {}) };
        models.value = { ...(cfg.models ?? {}) };
        fetched.value = { ...(cfg.fetched ?? {}) };
        promptHistory.value = Array.isArray(cfg.promptHistory)
            ? cfg.promptHistory
            : [];
    } else {
        migrateFromSettings(stored);
        persist();
    }
};

export const useAiConfig = () => {
    void load();

    const currentModel = computed(
        () => models.value[provider.value] || defaultModelFor(provider.value),
    );
    const currentKey = computed(() => keys.value[provider.value] ?? "");
    const currentBaseUrl = computed(() => baseUrls.value[provider.value] ?? "");

    const modelOptions = computed<string[]>(() => {
        const curated = CURATED_AI_MODELS[provider.value as AiProvider] ?? [];
        const fetchedList = fetched.value[provider.value] ?? [];
        const set = new Set<string>();
        for (const id of [...curated, ...fetchedList, currentModel.value]) {
            if (id) set.add(id);
        }
        return [...set].sort((a, b) =>
            a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" }),
        );
    });

    const isCustom = computed(
        () => provider.value === "Custom (OpenAI-compatible)",
    );

    const setProvider = (p: string) => {
        provider.value = p;
        persist();
    };
    const setKey = (value: string) => {
        keys.value = { ...keys.value, [provider.value]: value };
        persist();
    };
    const setModel = (value: string) => {
        models.value = { ...models.value, [provider.value]: value };
        persist();
    };
    const setBaseUrl = (value: string) => {
        baseUrls.value = { ...baseUrls.value, [provider.value]: value };
        persist();
    };

    /** Record a prompt/provider/model that was just used, newest first. */
    const addPromptHistory = (entry: {
        prompt: string;
        provider: string;
        model: string;
    }) => {
        const prompt = entry.prompt.trim();
        const next: AiPromptEntry = {
            prompt,
            provider: entry.provider,
            model: entry.model,
            at: Date.now(),
        };
        // Drop an identical earlier entry (same prompt + provider + model) so the
        // list stays a de-duplicated most-recently-used history.
        const rest = promptHistory.value.filter(
            (e) =>
                !(
                    e.prompt === next.prompt &&
                    e.provider === next.provider &&
                    e.model === next.model
                ),
        );
        promptHistory.value = [next, ...rest].slice(0, MAX_PROMPT_HISTORY);
        persist();
    };

    /** Query the provider's models API and merge the results. */
    const fetchModels = async (): Promise<{ ok: boolean; message: string }> => {
        if (!currentKey.value.trim()) {
            return { ok: false, message: "Enter this provider's API key first." };
        }
        try {
            const list = await invoke<string[]>("ai_list_models", {
                provider: provider.value,
                apiKey: currentKey.value,
                baseUrl: currentBaseUrl.value || null,
            });
            if (!list.length) {
                return { ok: false, message: "No models reported." };
            }
            fetched.value = { ...fetched.value, [provider.value]: list };
            if (!list.includes(currentModel.value)) setModel(list[0]);
            persist();
            return {
                ok: true,
                message: `Found ${list.length} model${list.length === 1 ? "" : "s"}.`,
            };
        } catch (error) {
            return {
                ok: false,
                message: String(error).replace(/^Error:\s*/, "").slice(0, 160),
            };
        }
    };

    return {
        provider,
        currentBaseUrl,
        currentModel,
        currentKey,
        modelOptions,
        isCustom,
        promptHistory,
        setProvider,
        setKey,
        setModel,
        setBaseUrl,
        addPromptHistory,
        fetchModels,
    };
};
