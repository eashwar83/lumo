<script setup lang="ts">
import { computed, ref } from "vue";
import { AI_PROVIDERS, defaultBaseUrlFor } from "../constants/aiModels";
import { useAiConfig } from "../composables/useAiConfig";

// Settings UI for the cloud-AI configuration. All state lives in the shared
// useAiConfig store (also used by the correction prompt), so a key entered here
// is instantly available there, and vice versa.

const CUSTOM_MODEL = "__custom__";

const {
    provider,
    currentBaseUrl,
    currentModel,
    currentKey,
    modelOptions,
    isCustom,
    setProvider,
    setKey,
    setModel,
    setBaseUrl,
    fetchModels,
} = useAiConfig();

const baseUrlPlaceholder = computed(() =>
    isCustom.value
        ? "https://…/v1  (required)"
        : `Default: ${defaultBaseUrlFor(provider.value)}`,
);

const showCustomModelInput = ref(false);

const onSelectModel = (value: string) => {
    if (value === CUSTOM_MODEL) {
        showCustomModelInput.value = true;
        return;
    }
    showCustomModelInput.value = false;
    setModel(value);
};

const fetching = ref(false);
const fetchStatus = ref("");
const onFetch = async () => {
    if (fetching.value) return;
    fetching.value = true;
    fetchStatus.value = "Fetching…";
    const result = await fetchModels();
    fetchStatus.value = result.message;
    fetching.value = false;
};
</script>

<template>
    <div class="ai-settings">
        <p class="ai-settings__intro">
            Sends a few frames to a cloud vision model for a tailored colour
            correction. Bring your own key — it stays on this device, and each
            run uploads frames to the provider you choose. You can also switch
            provider and model at correction time from the prompt window.
        </p>

        <label class="ai-settings__row">
            <span class="ai-settings__label">Provider</span>
            <select
                :value="provider"
                class="ai-settings__control"
                @change="
                    setProvider(($event.target as HTMLSelectElement).value);
                    showCustomModelInput = false;
                "
            >
                <option v-for="p in AI_PROVIDERS" :key="p" :value="p">
                    {{ p }}
                </option>
            </select>
        </label>

        <label class="ai-settings__row">
            <span class="ai-settings__label">API Key</span>
            <input
                :value="currentKey"
                class="ai-settings__control"
                type="password"
                autocomplete="off"
                spellcheck="false"
                placeholder="Paste this provider's API key"
                @input="setKey(($event.target as HTMLInputElement).value)"
            />
        </label>

        <label class="ai-settings__row">
            <span class="ai-settings__label">Base URL</span>
            <input
                :value="currentBaseUrl"
                class="ai-settings__control"
                type="text"
                spellcheck="false"
                :placeholder="baseUrlPlaceholder"
                @input="setBaseUrl(($event.target as HTMLInputElement).value)"
            />
        </label>

        <div class="ai-settings__row">
            <span class="ai-settings__label">Model</span>
            <div class="ai-settings__model">
                <select
                    v-if="!isCustom"
                    :value="showCustomModelInput ? CUSTOM_MODEL : currentModel"
                    class="ai-settings__control"
                    @change="onSelectModel(($event.target as HTMLSelectElement).value)"
                >
                    <option v-for="m in modelOptions" :key="m" :value="m">
                        {{ m }}
                    </option>
                    <option :value="CUSTOM_MODEL">Custom model…</option>
                </select>
                <input
                    v-if="isCustom || showCustomModelInput"
                    :value="currentModel"
                    class="ai-settings__control"
                    type="text"
                    spellcheck="false"
                    placeholder="Exact model id"
                    @input="setModel(($event.target as HTMLInputElement).value)"
                />
            </div>
        </div>

        <div class="ai-settings__actions">
            <button
                class="ai-settings__fetch"
                type="button"
                :disabled="fetching"
                @click="onFetch"
            >
                {{ fetching ? "Fetching…" : "Fetch latest models" }}
            </button>
            <span v-if="fetchStatus" class="ai-settings__status">
                {{ fetchStatus }}
            </span>
        </div>

        <p v-if="provider === 'DeepSeek'" class="ai-settings__warn">
            DeepSeek's public API has no vision model, so image analysis will
            fail. Use it only if you've configured a vision-capable DeepSeek
            deployment.
        </p>
        <p v-else-if="provider === 'Qwen'" class="ai-settings__status">
            Pick a <strong>vision</strong> model (name contains
            <code>vl</code>, e.g. qwen-vl-max, qwen3-vl-plus) — the plain
            <code>max</code>/<code>plus</code> models can't read images. For Qwen
            International or a workspace key, paste your OpenAI-compatible
            endpoint (ends in <code>/compatible-mode/v1</code>) into Base URL.
        </p>
    </div>
</template>

<style scoped>
.ai-settings {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.ai-settings__intro {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-muted, rgba(255, 255, 255, 0.55));
}

.ai-settings__row {
    display: flex;
    align-items: center;
    gap: 12px;
}

.ai-settings__label {
    flex: none;
    width: 92px;
    font-size: 13px;
    font-weight: 600;
}

.ai-settings__control {
    flex: 1;
    min-width: 0;
    padding: 8px 10px;
    border: 1px solid var(--glass-border, rgba(255, 255, 255, 0.16));
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.22);
    color: inherit;
    font-size: 13px;
    color-scheme: dark;
}

.ai-settings__control option {
    background-color: #1a1c21;
    color: #f2f2f2;
}

.ai-settings__model {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
}

.ai-settings__actions {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 2px;
}

.ai-settings__fetch {
    padding: 8px 14px;
    border: 1px solid rgba(196, 160, 255, 0.45);
    border-radius: 8px;
    background: rgba(170, 130, 255, 0.16);
    color: #e7dcff;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
}

.ai-settings__fetch:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.3);
}

.ai-settings__fetch:disabled {
    opacity: 0.55;
    cursor: default;
}

.ai-settings__status {
    font-size: 12px;
    color: var(--text-muted, rgba(255, 255, 255, 0.55));
}

.ai-settings__warn {
    margin: 0;
    font-size: 12px;
    line-height: 1.45;
    color: #ffb454;
}
</style>
