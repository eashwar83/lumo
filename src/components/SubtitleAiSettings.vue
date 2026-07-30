<script setup lang="ts">
import { computed } from "vue";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
    isWhisper,
    TRANSCRIBE_PROVIDERS,
    useSubtitleAiConfig,
} from "../composables/useSubtitleAiConfig";

// Settings UI for the default subtitle-AI engine. All state lives in the shared
// useSubtitleAiConfig store, so this sets the default that the Generate,
// Translate, and Sync dialogs all inherit (each can still override per run).

const sub = useSubtitleAiConfig();
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
</script>

<template>
    <div class="subai-settings">
        <p class="subai-settings__intro">
            The default engine for AI subtitles — generation, translation and
            sync. Set it once here; each dialog inherits it and can still override
            it per run. Keys stay on this device.
        </p>

        <label class="subai-settings__row">
            <span class="subai-settings__label">Provider</span>
            <select
                :value="sub.provider.value"
                class="subai-settings__control"
                @change="sub.setProvider(($event.target as HTMLSelectElement).value)"
            >
                <option v-for="p in TRANSCRIBE_PROVIDERS" :key="p" :value="p">
                    {{ p }}
                </option>
            </select>
        </label>

        <template v-if="!whisperSelected">
            <label class="subai-settings__row">
                <span class="subai-settings__label">API Key</span>
                <input
                    :value="sub.currentKey.value"
                    class="subai-settings__control"
                    type="password"
                    autocomplete="off"
                    spellcheck="false"
                    placeholder="Paste this provider's API key"
                    @input="sub.setKey(($event.target as HTMLInputElement).value)"
                />
            </label>

            <label class="subai-settings__row">
                <span class="subai-settings__label">Base URL</span>
                <input
                    :value="sub.currentBaseUrl.value"
                    class="subai-settings__control"
                    type="text"
                    spellcheck="false"
                    :placeholder="sub.baseUrlPlaceholder.value"
                    @input="sub.setBaseUrl(($event.target as HTMLInputElement).value)"
                />
            </label>

            <label class="subai-settings__row">
                <span class="subai-settings__label">Model</span>
                <input
                    :value="sub.currentModel.value"
                    class="subai-settings__control"
                    type="text"
                    spellcheck="false"
                    @input="sub.setModel(($event.target as HTMLInputElement).value)"
                />
            </label>
        </template>

        <template v-else>
            <div class="subai-settings__row">
                <span class="subai-settings__label">Whisper program</span>
                <div class="subai-settings__file">
                    <span class="subai-settings__file-name" :title="sub.whisperExe.value">
                        {{ whisperExeName || "whisper-cli(.exe)" }}
                    </span>
                    <button class="subai-settings__btn" type="button" @click="pickWhisperExe">
                        Browse…
                    </button>
                </div>
            </div>
            <div class="subai-settings__row">
                <span class="subai-settings__label">Model file</span>
                <div class="subai-settings__file">
                    <span class="subai-settings__file-name" :title="sub.whisperModel.value">
                        {{ whisperModelName || "ggml-*.bin" }}
                    </span>
                    <button class="subai-settings__btn" type="button" @click="pickWhisperModel">
                        Browse…
                    </button>
                </div>
            </div>
            <p class="subai-settings__note">
                Runs fully offline — no key or quota. Get models from
                huggingface.co/ggerganov/whisper.cpp.
            </p>
        </template>
    </div>
</template>

<style scoped>
.subai-settings {
    display: flex;
    flex-direction: column;
    gap: 12px;
}
.subai-settings__intro {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--text-muted, rgba(255, 255, 255, 0.55));
}
.subai-settings__row {
    display: flex;
    align-items: center;
    gap: 12px;
}
.subai-settings__label {
    flex: none;
    width: 110px;
    font-size: 13px;
    font-weight: 600;
}
.subai-settings__control {
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
.subai-settings__control option {
    background-color: #1a1c21;
    color: #f2f2f2;
}
.subai-settings__file {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
}
.subai-settings__file-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
    font-family: ui-monospace, monospace;
    color: rgba(255, 255, 255, 0.72);
}
.subai-settings__btn {
    padding: 8px 14px;
    border: 1px solid rgba(196, 160, 255, 0.45);
    border-radius: 8px;
    background: rgba(170, 130, 255, 0.16);
    color: #e7dcff;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
}
.subai-settings__btn:hover {
    background: rgba(170, 130, 255, 0.3);
}
.subai-settings__note {
    margin: 0;
    font-size: 12px;
    line-height: 1.45;
    color: var(--text-muted, rgba(255, 255, 255, 0.55));
}
</style>
