import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import {
    AI_PROVIDERS,
    CURATED_AI_MODELS,
    DEFAULT_AI_BASE_URLS,
    defaultBaseUrlFor,
    defaultModelFor,
    type AiProvider,
} from "../constants/aiModels";
import { useAiConfig } from "./useAiConfig";

const PROVIDER_KEY = "lumo.commentTranslate.provider";
const MODEL_KEY = "lumo.commentTranslate.model";
const FETCHED_KEY = "lumo.commentTranslate.fetchedModels";

const readStored = (key: string): string => {
    try {
        return localStorage.getItem(key) ?? "";
    } catch {
        return "";
    }
};

const writeStored = (key: string, value: string) => {
    try {
        localStorage.setItem(key, value);
    } catch {
        // Storage unavailable — the choice lasts for this session only.
    }
};

// Comment translation picks its own provider/model so it can differ from the
// one AI Enhance is set to, but it reuses the API keys already stored there.
const provider = ref(readStored(PROVIDER_KEY));
const model = ref(readStored(MODEL_KEY));
// Models discovered from each provider's own API, so the list is not
// frozen to whatever was curated when Lumo shipped.
const fetchedModels = ref<Record<string, string[]>>(
    (() => {
        try {
            return JSON.parse(readStored(FETCHED_KEY) || "{}");
        } catch {
            return {};
        }
    })(),
);
const isFetchingModels = ref(false);

export const useCommentTranslateAi = () => {
    const aiConfig = useAiConfig();

    // Empty selection means "whatever AI Enhance is set to right now".
    const activeProvider = computed(
        () => provider.value || aiConfig.provider.value,
    );
    const activeModel = computed(
        () =>
            (provider.value ? model.value : aiConfig.currentModel.value) ||
            defaultModelFor(activeProvider.value),
    );

    const providerOptions = computed(() => AI_PROVIDERS as readonly string[]);

    const modelOptions = computed<readonly string[]>(() => {
        const curated =
            CURATED_AI_MODELS[activeProvider.value as AiProvider] ?? [];
        const discovered = fetchedModels.value[activeProvider.value] ?? [];
        const set = new Set<string>([...curated, ...discovered]);
        if (activeModel.value) set.add(activeModel.value);
        return [...set].sort((a, b) =>
            a.localeCompare(b, undefined, {
                numeric: true,
                sensitivity: "base",
            }),
        );
    });

    /** Asks the provider which models it currently offers. */
    const fetchModels = async (): Promise<{ ok: boolean; message: string }> => {
        if (isFetchingModels.value) return { ok: false, message: "" };
        const name = activeProvider.value;
        const key = aiConfig.keyFor(name).trim();
        if (!key) {
            return {
                ok: false,
                message: `Add an API key for ${name} in Settings → AI Enhance first.`,
            };
        }
        isFetchingModels.value = true;
        try {
            const list = await invoke<string[]>("ai_list_models", {
                provider: name,
                apiKey: key,
                baseUrl: aiConfig.baseUrlFor(name).trim() || null,
            });
            if (!list.length) {
                return { ok: false, message: `${name} reported no models.` };
            }
            fetchedModels.value = { ...fetchedModels.value, [name]: list };
            writeStored(FETCHED_KEY, JSON.stringify(fetchedModels.value));
            return {
                ok: true,
                message: `${name}: ${list.length} model${list.length === 1 ? "" : "s"} available.`,
            };
        } catch (error) {
            return {
                ok: false,
                message: String(error).replace(/^Error:\s*/, "").slice(0, 160),
            };
        } finally {
            isFetchingModels.value = false;
        }
    };

    const setProvider = (value: string) => {
        provider.value = value;
        // The previous model belongs to the previous provider.
        model.value = defaultModelFor(value);
        writeStored(PROVIDER_KEY, provider.value);
        writeStored(MODEL_KEY, model.value);
    };

    const setModel = (value: string) => {
        // Choosing a model pins the provider too, otherwise the pair drifts
        // apart as soon as AI Enhance changes.
        if (!provider.value) {
            provider.value = activeProvider.value;
            writeStored(PROVIDER_KEY, provider.value);
        }
        model.value = value;
        writeStored(MODEL_KEY, value);
    };

    /** Credentials for the chosen provider, or the reason it can't be used. */
    const resolve = ():
        | { base: string; key: string; model: string }
        | { error: string } => {
        const name = activeProvider.value;
        const key = aiConfig.keyFor(name).trim();
        if (!key) {
            return {
                error: `No API key stored for ${name}. Add it in Settings → AI Enhance.`,
            };
        }
        const base =
            aiConfig.baseUrlFor(name).trim() ||
            DEFAULT_AI_BASE_URLS[name as AiProvider] ||
            defaultBaseUrlFor(name);
        if (!base) {
            return { error: `No endpoint configured for ${name}.` };
        }
        return { base, key, model: activeModel.value };
    };

    return {
        provider: activeProvider,
        model: activeModel,
        providerOptions,
        modelOptions,
        isFetchingModels,
        fetchModels,
        setProvider,
        setModel,
        resolve,
    };
};
