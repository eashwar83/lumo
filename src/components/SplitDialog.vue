<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

// Split one media file into pieces without re-encoding (segment muxer, stream
// copy). Cuts snap to the nearest keyframe — inherent to a lossless split.

const props = defineProps<{
    open: boolean;
    currentPath: string;
    currentDuration: number;
    currentPosition: number;
}>();
const emit = defineEmits<{
    (e: "close"): void;
    (e: "notify", message: string): void;
}>();

const VIDEO_EXTS = [
    "mp4", "mkv", "mov", "m4v", "avi", "webm", "ts", "flv", "wmv", "mpg", "mpeg",
];

type Mode = "timestamps" | "equal" | "everyN" | "chapters";

const target = ref("");
const duration = ref(0);
const chapters = ref<number[]>([]);
const mode = ref<Mode>("timestamps");
const timestamps = ref<string[]>([""]);
const equalParts = ref(2);
const everyMinutes = ref(10);
const outDir = ref("");
const baseName = ref("");
const busy = ref(false);
const statusText = ref("");

const fileName = (path: string) => path.split(/[\\/]/).pop() || path;
const folderOf = (path: string) => {
    const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return i >= 0 ? path.slice(0, i) : "";
};
const stemOf = (path: string) => {
    const name = fileName(path);
    const dot = name.lastIndexOf(".");
    return dot > 0 ? name.slice(0, dot) : name;
};

const loadInfo = async (path: string) => {
    if (!path) return;
    // Duration is already known for the loaded file; still probe for chapters.
    if (path === props.currentPath && props.currentDuration > 0) {
        duration.value = props.currentDuration;
    }
    try {
        const info = await invoke<{
            duration: number;
            chapters: number[];
            probed: boolean;
        }>("media_edit_info", { path });
        if (info.duration > 0) duration.value = info.duration;
        chapters.value = info.chapters ?? [];
        if (!info.probed && mode.value === "chapters") mode.value = "timestamps";
    } catch {
        chapters.value = [];
    }
};

const initialise = () => {
    target.value = props.currentPath || "";
    duration.value = props.currentDuration || 0;
    chapters.value = [];
    mode.value = "timestamps";
    timestamps.value = [""];
    equalParts.value = 2;
    everyMinutes.value = 10;
    outDir.value = props.currentPath ? folderOf(props.currentPath) : "";
    baseName.value = props.currentPath ? stemOf(props.currentPath) : "clip";
    statusText.value = "";
    busy.value = false;
    if (target.value) void loadInfo(target.value);
};

watch(
    () => props.open,
    (open) => {
        if (open) initialise();
    },
);

const chooseTarget = async () => {
    const selected = await openDialog({
        multiple: false,
        directory: false,
        filters: [{ name: "Video", extensions: VIDEO_EXTS }],
    });
    const path = Array.isArray(selected) ? selected[0] : selected;
    if (!path) return;
    target.value = path;
    outDir.value = folderOf(path);
    baseName.value = stemOf(path);
    duration.value = 0;
    await loadInfo(path);
};

const chooseFolder = async () => {
    const selected = await openDialog({ directory: true, multiple: false });
    const dir = Array.isArray(selected) ? selected[0] : selected;
    if (dir) outDir.value = dir;
};

const isCurrent = computed(() => target.value === props.currentPath);

const addTimestamp = () => {
    timestamps.value = [...timestamps.value, ""];
};
const addCurrentPosition = () => {
    timestamps.value = [...timestamps.value, formatTime(props.currentPosition)];
};
const removeTimestamp = (i: number) => {
    timestamps.value = timestamps.value.filter((_, idx) => idx !== i);
    if (!timestamps.value.length) timestamps.value = [""];
};

function formatTime(seconds: number): string {
    const s = Math.max(0, Math.floor(seconds));
    const hh = Math.floor(s / 3600);
    const mm = Math.floor((s % 3600) / 60);
    const ss = s % 60;
    const pad = (n: number) => String(n).padStart(2, "0");
    return hh > 0 ? `${hh}:${pad(mm)}:${pad(ss)}` : `${mm}:${pad(ss)}`;
}

// Parse "H:MM:SS" / "MM:SS" / "SS" / decimal seconds → seconds, or null.
function parseTime(raw: string): number | null {
    const text = raw.trim();
    if (!text) return null;
    if (/^\d+(\.\d+)?$/.test(text)) return parseFloat(text);
    const parts = text.split(":").map((p) => p.trim());
    if (parts.some((p) => p === "" || Number.isNaN(Number(p)))) return null;
    let seconds = 0;
    for (const part of parts) seconds = seconds * 60 + Number(part);
    return seconds;
}

// The computed, validated split points for the chosen mode.
const splitTimes = computed<number[]>(() => {
    const dur = duration.value;
    let times: number[] = [];
    if (mode.value === "timestamps") {
        times = timestamps.value
            .map(parseTime)
            .filter((t): t is number => t !== null);
    } else if (mode.value === "equal") {
        const n = Math.floor(equalParts.value);
        if (n >= 2 && dur > 0) {
            for (let k = 1; k < n; k++) times.push((dur * k) / n);
        }
    } else if (mode.value === "everyN") {
        const step = everyMinutes.value * 60;
        if (step > 0 && dur > 0) {
            for (let t = step; t < dur - 0.5; t += step) times.push(t);
        }
    } else if (mode.value === "chapters") {
        times = [...chapters.value];
    }
    // Keep only points strictly inside the file, sorted and de-duplicated.
    const max = dur > 0 ? dur : Infinity;
    const cleaned = times
        .filter((t) => t > 0.05 && t < max - 0.05)
        .sort((a, b) => a - b);
    return cleaned.filter((t, i) => i === 0 || t - cleaned[i - 1] > 0.05);
});

const partCount = computed(() => splitTimes.value.length + 1);

const canSplit = computed(
    () =>
        !busy.value &&
        !!target.value &&
        !!outDir.value.trim() &&
        !!baseName.value.trim() &&
        splitTimes.value.length >= 1,
);

const runSplit = async () => {
    if (!canSplit.value) return;
    busy.value = true;
    statusText.value = "Splitting…";
    try {
        const produced = await invoke<string[]>("split_file", {
            path: target.value,
            outDir: outDir.value.trim(),
            base: baseName.value.trim(),
            times: splitTimes.value,
        });
        emit("notify", `Split into ${produced.length} files`);
        emit("close");
    } catch (error) {
        statusText.value = String(error)
            .replace(/^Error:\s*/, "")
            .slice(0, 220);
    } finally {
        busy.value = false;
    }
};

const modeTabs: { id: Mode; label: string }[] = [
    { id: "timestamps", label: "Timestamps" },
    { id: "equal", label: "Equal parts" },
    { id: "everyN", label: "Every N min" },
    { id: "chapters", label: "Chapters" },
];
</script>

<template>
    <transition name="split-fade">
        <div
            v-if="open"
            class="split"
            @keydown.esc.stop.prevent="emit('close')"
        >
            <div class="split__backdrop" @click="emit('close')" />
            <div class="split__box" role="dialog" aria-label="Split file">
                <div class="split__title">Split File</div>

                <div class="split__row">
                    <span class="split__label">File</span>
                    <span class="split__file" :title="target">
                        {{ target ? fileName(target) : "No file selected" }}
                    </span>
                    <button class="split__btn" type="button" @click="chooseTarget">
                        Choose…
                    </button>
                </div>

                <div class="split__tabs">
                    <button
                        v-for="tab in modeTabs"
                        :key="tab.id"
                        class="split__tab"
                        :class="{ 'split__tab--active': mode === tab.id }"
                        type="button"
                        :disabled="tab.id === 'chapters' && !chapters.length"
                        @click="mode = tab.id"
                    >
                        {{ tab.label }}
                    </button>
                </div>

                <!-- Timestamps -->
                <div v-if="mode === 'timestamps'" class="split__mode">
                    <div
                        v-for="(_, i) in timestamps"
                        :key="i"
                        class="split__ts-row"
                    >
                        <input
                            v-model="timestamps[i]"
                            class="split__input"
                            type="text"
                            placeholder="e.g. 12:30 or 1:05:00"
                        />
                        <button
                            class="split__icon split__icon--danger"
                            type="button"
                            title="Remove"
                            @click="removeTimestamp(i)"
                        >
                            ×
                        </button>
                    </div>
                    <div class="split__ts-actions">
                        <button class="split__btn" type="button" @click="addTimestamp">
                            + Add time
                        </button>
                        <button
                            v-if="isCurrent"
                            class="split__btn"
                            type="button"
                            @click="addCurrentPosition"
                        >
                            Add current position ({{ formatTime(currentPosition) }})
                        </button>
                    </div>
                </div>

                <!-- Equal parts -->
                <div v-else-if="mode === 'equal'" class="split__mode">
                    <label class="split__row">
                        <span class="split__label">Number of parts</span>
                        <input
                            v-model.number="equalParts"
                            class="split__input split__input--num"
                            type="number"
                            min="2"
                            max="50"
                        />
                    </label>
                </div>

                <!-- Every N minutes -->
                <div v-else-if="mode === 'everyN'" class="split__mode">
                    <label class="split__row">
                        <span class="split__label">Every</span>
                        <input
                            v-model.number="everyMinutes"
                            class="split__input split__input--num"
                            type="number"
                            min="1"
                            max="600"
                        />
                        <span class="split__label">minutes</span>
                    </label>
                </div>

                <!-- Chapters -->
                <div v-else class="split__mode">
                    <p class="split__note">
                        {{ chapters.length }} chapter marker{{
                            chapters.length === 1 ? "" : "s"
                        }}
                        found — the file will be cut at each one.
                    </p>
                </div>

                <div class="split__row">
                    <span class="split__label">Save to</span>
                    <span class="split__file" :title="outDir">
                        {{ outDir || "Choose a folder" }}
                    </span>
                    <button class="split__btn" type="button" @click="chooseFolder">
                        Folder…
                    </button>
                </div>
                <label class="split__row">
                    <span class="split__label">Name</span>
                    <input
                        v-model="baseName"
                        class="split__input"
                        type="text"
                        placeholder="Base name"
                    />
                </label>

                <p class="split__summary">
                    <template v-if="splitTimes.length">
                        {{ partCount }} parts · lossless (cuts snap to keyframes)
                    </template>
                    <template v-else>Add at least one split point.</template>
                </p>

                <p v-if="statusText" class="split__status">{{ statusText }}</p>

                <div class="split__actions">
                    <button class="split__btn" type="button" @click="emit('close')">
                        Cancel
                    </button>
                    <button
                        class="split__btn split__btn--primary"
                        type="button"
                        :disabled="!canSplit"
                        @click="runSplit"
                    >
                        {{ busy ? "Working…" : "Split & Save" }}
                    </button>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.split {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    align-items: center;
    justify-content: center;
}
.split__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}
.split__box {
    position: relative;
    width: min(560px, 92vw);
    max-height: 88vh;
    overflow-y: auto;
    padding: 20px;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    color: #fff;
}
.split__title {
    font-size: 15px;
    font-weight: 700;
    margin-bottom: 12px;
}
.split__row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 8px 0;
}
.split__label {
    flex: none;
    font-size: 12.5px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.75);
}
.split__file {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: rgba(255, 255, 255, 0.85);
}
.split__tabs {
    display: flex;
    gap: 4px;
    margin: 14px 0 10px;
    background: rgba(0, 0, 0, 0.25);
    padding: 4px;
    border-radius: 9px;
}
.split__tab {
    flex: 1;
    padding: 7px 6px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.65);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}
.split__tab--active {
    background: rgba(170, 130, 255, 0.3);
    color: #fff;
}
.split__tab:disabled {
    opacity: 0.4;
    cursor: default;
}
.split__mode {
    margin: 10px 0;
    min-height: 40px;
}
.split__ts-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
}
.split__ts-actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
}
.split__input {
    flex: 1;
    min-width: 0;
    padding: 8px 10px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.28);
    color: #fff;
    font-size: 13px;
}
.split__input--num {
    flex: none;
    width: 90px;
}
.split__icon {
    width: 30px;
    height: 30px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
    font-size: 15px;
    cursor: pointer;
}
.split__icon--danger:hover {
    background: rgba(220, 60, 60, 0.8);
}
.split__note {
    margin: 0;
    font-size: 12.5px;
    color: rgba(255, 255, 255, 0.6);
}
.split__summary {
    margin: 10px 0 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.55);
}
.split__status {
    margin: 8px 0 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.7);
}
.split__btn {
    padding: 8px 14px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}
.split__btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
}
.split__btn:disabled {
    opacity: 0.5;
    cursor: default;
}
.split__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
}
.split__btn--primary {
    border-color: rgba(196, 160, 255, 0.5);
    background: rgba(170, 130, 255, 0.28);
}
.split__btn--primary:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.42);
}
.split-fade-enter-active,
.split-fade-leave-active {
    transition: opacity 0.15s ease;
}
.split-fade-enter-from,
.split-fade-leave-to {
    opacity: 0;
}
</style>
