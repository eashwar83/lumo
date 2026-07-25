<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

// Merge several media files into one. Lossless (stream copy) when they share the
// same format; otherwise an opt-in re-encode combines them anyway. All ffmpeg
// work happens in the backend `merge_files` command.

const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{
    (e: "close"): void;
    (e: "notify", message: string): void;
    (e: "load", path: string): void;
}>();

const VIDEO_EXTS = [
    "mp4", "mkv", "mov", "m4v", "avi", "webm", "ts", "flv", "wmv", "mpg", "mpeg",
];

const files = ref<string[]>([]);
const busy = ref(false);
const statusText = ref("");
// null = not yet inspected; describes lossless viability.
const inspect = ref<{
    compatible: boolean;
    probed: boolean;
    note: string;
} | null>(null);
// When compatibility can't be confirmed, the user chooses whether to re-encode.
const reencodeChoice = ref(false);

const baseName = (path: string) =>
    path.split(/[\\/]/).pop() || path;
const extOf = (path: string) =>
    (path.split(".").pop() || "mp4").toLowerCase();

const reset = () => {
    files.value = [];
    inspect.value = null;
    reencodeChoice.value = false;
    statusText.value = "";
    busy.value = false;
};

watch(
    () => props.open,
    (open) => {
        if (open) reset();
    },
);

const addFiles = async () => {
    const selected = await openDialog({
        multiple: true,
        directory: false,
        filters: [{ name: "Video", extensions: VIDEO_EXTS }],
    });
    const paths = Array.isArray(selected)
        ? selected
        : selected
          ? [selected]
          : [];
    for (const path of paths) {
        if (!files.value.includes(path)) files.value = [...files.value, path];
    }
};

const removeFile = (i: number) => {
    files.value = files.value.filter((_, idx) => idx !== i);
};
const moveFile = (i: number, dir: -1 | 1) => {
    const j = i + dir;
    if (j < 0 || j >= files.value.length) return;
    const next = [...files.value];
    [next[i], next[j]] = [next[j], next[i]];
    files.value = next;
};

// Re-inspect (debounced) whenever the set of files changes.
let inspectTimer: number | null = null;
watch(
    files,
    () => {
        inspect.value = null;
        if (inspectTimer) window.clearTimeout(inspectTimer);
        if (files.value.length < 2) return;
        const snapshot = [...files.value];
        inspectTimer = window.setTimeout(async () => {
            try {
                const result = await invoke<{
                    compatible: boolean;
                    probed: boolean;
                    note: string;
                }>("inspect_merge", { paths: snapshot });
                // Ignore a stale result if the list changed meanwhile.
                if (snapshot.join("|") === files.value.join("|")) {
                    inspect.value = result;
                    reencodeChoice.value = !result.probed ? false : !result.compatible;
                }
            } catch (error) {
                statusText.value = String(error)
                    .replace(/^Error:\s*/, "")
                    .slice(0, 180);
            }
        }, 250);
    },
    { deep: false },
);

// Whether the actual merge will re-encode.
const willReencode = computed(() => {
    const info = inspect.value;
    if (!info) return false;
    if (info.probed && !info.compatible) return true; // lossless impossible
    if (!info.probed) return reencodeChoice.value; // user's call
    return false; // compatible → lossless
});

const canMerge = computed(() => files.value.length >= 2 && !busy.value);

const runMerge = async () => {
    if (!canMerge.value) return;
    const first = files.value[0];
    const ext = extOf(first);
    const output = await saveDialog({
        defaultPath: `merged.${ext}`,
        filters: [{ name: "Video", extensions: [ext] }],
    });
    if (!output) return;
    busy.value = true;
    statusText.value = willReencode.value
        ? "Merging (re-encoding)… this can take a while."
        : "Merging losslessly…";
    try {
        const saved = await invoke<string>("merge_files", {
            paths: [...files.value],
            output,
            reencode: willReencode.value,
        });
        emit("notify", `Merged ${files.value.length} files → ${baseName(saved)}`);
        emit("load", saved);
        emit("close");
    } catch (error) {
        statusText.value = String(error)
            .replace(/^Error:\s*/, "")
            .slice(0, 220);
    } finally {
        busy.value = false;
    }
};
</script>

<template>
    <transition name="merge-fade">
        <div
            v-if="open"
            class="merge"
            @keydown.esc.stop.prevent="emit('close')"
        >
            <div class="merge__backdrop" @click="emit('close')" />
            <div class="merge__box" role="dialog" aria-label="Merge files">
                <div class="merge__title">Merge Files</div>
                <p class="merge__hint">
                    Combine clips into one file, in the order shown. Same-format
                    files merge instantly with no quality loss.
                </p>

                <div class="merge__toolbar">
                    <button class="merge__btn" type="button" @click="addFiles">
                        + Add files
                    </button>
                    <span v-if="files.length" class="merge__count">
                        {{ files.length }} file{{ files.length === 1 ? "" : "s" }}
                    </span>
                </div>

                <ul v-if="files.length" class="merge__list">
                    <li v-for="(file, i) in files" :key="file" class="merge__item">
                        <span class="merge__index">{{ i + 1 }}</span>
                        <span class="merge__name" :title="file">
                            {{ baseName(file) }}
                        </span>
                        <div class="merge__item-actions">
                            <button
                                class="merge__icon"
                                type="button"
                                :disabled="i === 0"
                                title="Move up"
                                @click="moveFile(i, -1)"
                            >
                                ↑
                            </button>
                            <button
                                class="merge__icon"
                                type="button"
                                :disabled="i === files.length - 1"
                                title="Move down"
                                @click="moveFile(i, 1)"
                            >
                                ↓
                            </button>
                            <button
                                class="merge__icon merge__icon--danger"
                                type="button"
                                title="Remove"
                                @click="removeFile(i)"
                            >
                                ×
                            </button>
                        </div>
                    </li>
                </ul>
                <p v-else class="merge__empty">No files added yet.</p>

                <p
                    v-if="inspect && !inspect.compatible"
                    class="merge__note merge__note--warn"
                >
                    {{ inspect.note }}
                </p>
                <p
                    v-else-if="inspect && inspect.compatible"
                    class="merge__note merge__note--ok"
                >
                    Same format — will merge losslessly.
                </p>

                <label
                    v-if="inspect && !inspect.probed"
                    class="merge__reencode"
                >
                    <input v-model="reencodeChoice" type="checkbox" />
                    Re-encode to force the merge (slower, use if lossless fails)
                </label>

                <p v-if="statusText" class="merge__status">{{ statusText }}</p>

                <div class="merge__actions">
                    <button
                        class="merge__btn"
                        type="button"
                        @click="emit('close')"
                    >
                        Cancel
                    </button>
                    <button
                        class="merge__btn merge__btn--primary"
                        type="button"
                        :disabled="!canMerge"
                        @click="runMerge"
                    >
                        {{
                            busy
                                ? "Working…"
                                : willReencode
                                  ? "Merge (re-encode) & Save…"
                                  : "Merge & Save…"
                        }}
                    </button>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.merge {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    align-items: center;
    justify-content: center;
}
.merge__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}
.merge__box {
    position: relative;
    width: min(560px, 92vw);
    max-height: 86vh;
    overflow-y: auto;
    padding: 20px;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    color: #fff;
}
.merge__title {
    font-size: 15px;
    font-weight: 700;
}
.merge__hint {
    margin: 6px 0 12px;
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.55);
}
.merge__toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 10px;
}
.merge__count {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.5);
}
.merge__list {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
}
.merge__item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
}
.merge__index {
    flex: none;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: rgba(170, 130, 255, 0.25);
    font-size: 11px;
    font-weight: 700;
}
.merge__name {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.merge__item-actions {
    display: flex;
    gap: 4px;
}
.merge__icon {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
    font-size: 13px;
    cursor: pointer;
}
.merge__icon:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
}
.merge__icon:disabled {
    opacity: 0.35;
    cursor: default;
}
.merge__icon--danger:hover:not(:disabled) {
    background: rgba(220, 60, 60, 0.8);
}
.merge__empty {
    margin: 0 0 10px;
    font-size: 12.5px;
    color: rgba(255, 255, 255, 0.4);
}
.merge__note {
    margin: 6px 0;
    font-size: 12px;
    line-height: 1.45;
}
.merge__note--warn {
    color: #ffb454;
}
.merge__note--ok {
    color: #7fd08a;
}
.merge__reencode {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0;
    font-size: 12.5px;
    color: rgba(255, 255, 255, 0.8);
}
.merge__status {
    margin: 6px 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.7);
}
.merge__toolbar .merge__btn,
.merge__btn {
    padding: 8px 14px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}
.merge__btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
}
.merge__btn:disabled {
    opacity: 0.5;
    cursor: default;
}
.merge__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
}
.merge__btn--primary {
    border-color: rgba(196, 160, 255, 0.5);
    background: rgba(170, 130, 255, 0.28);
    color: #fff;
}
.merge__btn--primary:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.42);
}
.merge-fade-enter-active,
.merge-fade-leave-active {
    transition: opacity 0.15s ease;
}
.merge-fade-enter-from,
.merge-fade-leave-to {
    opacity: 0;
}
</style>
