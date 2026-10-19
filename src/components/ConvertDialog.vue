<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";

// Convert the currently playing local media file to another container format,
// with or without re-encoding and with an optional display aspect ratio change.

const props = defineProps<{
    open: boolean;
    currentPath: string;
    currentAspectLabel: string;
}>();

const emit = defineEmits<{
    (e: "close"): void;
    (e: "notify", message: string): void;
}>();

type TargetFormat = "mp4" | "mkv" | "mov" | "webm" | "avi" | "ts";

type ConvertInspect = {
    vcodec: string;
    acodec: string;
    width: number;
    height: number;
    displayAspectRatio: string;
    hasVideo: boolean;
    hasAudio: boolean;
    probed: boolean;
};

const FORMATS: { id: TargetFormat; label: string }[] = [
    { id: "mp4", label: "MP4" },
    { id: "mkv", label: "MKV" },
    { id: "mov", label: "MOV" },
    { id: "webm", label: "WEBM" },
    { id: "avi", label: "AVI" },
    { id: "ts", label: "TS" },
];

const ASPECT_OPTIONS: { label: string; value: string; matchLabel: string }[] = [
    { label: "Keep Original", value: "-1", matchLabel: "Default" },
    { label: "16:9", value: "16:9", matchLabel: "16:9" },
    { label: "4:3", value: "4:3", matchLabel: "4:3" },
    { label: "21:9", value: "21:9", matchLabel: "21:9" },
    { label: "2.35:1", value: "2.35", matchLabel: "2.35:1" },
];

const format = ref<TargetFormat>("mp4");
const aspect = ref<string>("-1");
const reencodeChoice = ref(false);
const inspect = ref<ConvertInspect | null>(null);
const busy = ref(false);
const statusText = ref("");

const fileName = (path: string) => path.split(/[\\/]/).pop() || path;
const folderOf = (path: string) => {
    const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return i >= 0 ? path.slice(0, i) : "";
};
const sepOf = (path: string) => (path.includes("\\") ? "\\" : "/");
const stemOf = (path: string) => {
    const name = fileName(path);
    const dot = name.lastIndexOf(".");
    return dot > 0 ? name.slice(0, dot) : name;
};
const extOf = (path: string) => {
    const name = fileName(path);
    const dot = name.lastIndexOf(".");
    return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
};

const loadInspect = async (path: string) => {
    if (!path) return;
    try {
        const result = await invoke<ConvertInspect>("inspect_convert", { path });
        if (props.currentPath === path) {
            inspect.value = result;
        }
    } catch {
        inspect.value = null;
    }
};

const initialise = () => {
    format.value = "mp4";
    const matched = ASPECT_OPTIONS.find(
        (opt) => opt.matchLabel === props.currentAspectLabel,
    );
    aspect.value = matched ? matched.value : "-1";
    reencodeChoice.value = false;
    inspect.value = null;
    statusText.value = "";
    busy.value = false;
    if (props.currentPath) {
        void loadInspect(props.currentPath);
    }
};

watch(
    () => props.open,
    (open) => {
        if (open) initialise();
    },
);

const streamSummary = computed(() => {
    const info = inspect.value;
    if (!info || !info.probed) return "";
    const parts: string[] = [];
    if (info.vcodec) parts.push(info.vcodec.toUpperCase());
    if (info.width > 0 && info.height > 0) {
        parts.push(`${info.width}×${info.height}`);
    }
    if (
        info.displayAspectRatio &&
        info.displayAspectRatio !== "0:1" &&
        info.displayAspectRatio !== "N/A"
    ) {
        parts.push(`DAR ${info.displayAspectRatio}`);
    }
    if (info.acodec) parts.push(info.acodec.toUpperCase());
    return parts.join(" · ");
});

const CONTAINER_VCODECS: Record<TargetFormat, string[] | null> = {
    mkv: null, // Matroska accepts virtually any video stream.
    mp4: ["h264", "hevc", "av1", "mpeg4", "vp9", "mpeg2video", "mjpeg"],
    mov: ["h264", "hevc", "prores", "mpeg4", "mjpeg", "dnxhd"],
    webm: ["vp8", "vp9", "av1"],
    avi: ["h264", "mpeg4", "msmpeg4v3", "msmpeg4v2", "mpeg2video", "mjpeg", "wmv3", "dvvideo"],
    ts: ["h264", "hevc", "mpeg2video", "mpeg1video", "mpeg4"],
};

const CONTAINER_ACODECS: Record<TargetFormat, string[] | null> = {
    mkv: null,
    mp4: ["aac", "mp3", "ac3", "eac3", "alac", "opus", "mp2", "flac"],
    mov: ["aac", "mp3", "ac3", "eac3", "alac", "pcm_s16le", "pcm_s24le", "mp2"],
    webm: ["vorbis", "opus"],
    avi: ["mp3", "ac3", "mp2", "pcm_s16le", "pcm_u8", "aac"],
    ts: ["aac", "mp3", "mp2", "ac3", "eac3", "opus", "dts", "dca"],
};

const copyCompatibility = computed<{ compatible: boolean; reason: string }>(() => {
    const info = inspect.value;
    if (!info || !info.probed) {
        return { compatible: true, reason: "" };
    }
    const fmt = format.value;
    const allowedV = CONTAINER_VCODECS[fmt];
    const allowedA = CONTAINER_ACODECS[fmt];
    const vc = info.vcodec.toLowerCase();
    const ac = info.acodec.toLowerCase();

    const vOk = !info.hasVideo || !vc || !allowedV || allowedV.includes(vc);
    const aOk = !info.hasAudio || !ac || !allowedA || allowedA.includes(ac);
    if (vOk && aOk) {
        return { compatible: true, reason: "" };
    }
    const codecs = [vc, ac].filter(Boolean).join(" / ").toUpperCase();
    return {
        compatible: false,
        reason: `${codecs} cannot be copied directly into ${fmt.toUpperCase()} — will re-encode.`,
    };
});

const willReencode = computed(() => {
    if (!copyCompatibility.value.compatible) return true;
    return reencodeChoice.value;
});

const canConvert = computed(() => !busy.value && !!props.currentPath);

const runConvert = async () => {
    if (!canConvert.value) return;
    const src = props.currentPath;
    const targetFmt = format.value;
    const folder = folderOf(src);
    const stem = stemOf(src);
    const srcExt = extOf(src);
    const fileStem = srcExt === targetFmt ? `${stem}-converted` : stem;
    const defaultName = `${fileStem}.${targetFmt}`;
    const defaultPath = folder
        ? `${folder}${sepOf(src)}${defaultName}`
        : defaultName;

    const output = await saveDialog({
        defaultPath,
        filters: [{ name: targetFmt.toUpperCase(), extensions: [targetFmt] }],
    });
    if (!output) return;

    busy.value = true;
    statusText.value = willReencode.value
        ? "Converting (re-encoding)… this can take a while."
        : "Converting (stream copy)…";
    try {
        const saved = await invoke<string>("convert_file", {
            path: src,
            output,
            format: targetFmt,
            reencode: willReencode.value,
            aspect: aspect.value === "-1" ? null : aspect.value,
        });
        emit("notify", `Converted → ${fileName(saved)}`);
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
    <transition name="convert-fade">
        <div
            v-if="open"
            class="convert"
            @keydown.esc.stop.prevent="emit('close')"
        >
            <div class="convert__backdrop" @click="emit('close')" />
            <div class="convert__box" role="dialog" aria-label="Convert file">
                <div class="convert__title">Convert</div>
                <p class="convert__hint">
                    Convert the playing file to another format, with or without
                    re-encoding and optional aspect ratio adjustment.
                </p>

                <div class="convert__file-box">
                    <div class="convert__file-name" :title="currentPath">
                        {{ currentPath ? fileName(currentPath) : "No file playing" }}
                    </div>
                    <div v-if="streamSummary" class="convert__file-meta">
                        {{ streamSummary }}
                    </div>
                </div>

                <div class="convert__section">
                    <div class="convert__label">Output Format</div>
                    <div class="convert__pills">
                        <button
                            v-for="item in FORMATS"
                            :key="item.id"
                            class="convert__pill"
                            :class="{ 'convert__pill--active': format === item.id }"
                            type="button"
                            :disabled="busy"
                            @click="format = item.id"
                        >
                            {{ item.label }}
                        </button>
                    </div>
                </div>

                <div class="convert__section">
                    <div class="convert__label">Aspect Ratio</div>
                    <div class="convert__pills">
                        <button
                            v-for="opt in ASPECT_OPTIONS"
                            :key="opt.value"
                            class="convert__pill"
                            :class="{ 'convert__pill--active': aspect === opt.value }"
                            type="button"
                            :disabled="busy"
                            @click="aspect = opt.value"
                        >
                            {{ opt.label }}
                        </button>
                    </div>
                </div>

                <div class="convert__section">
                    <label class="convert__checkbox">
                        <input
                            :checked="willReencode"
                            type="checkbox"
                            :disabled="busy || !copyCompatibility.compatible"
                            @change="
                                reencodeChoice = ($event.target as HTMLInputElement).checked
                            "
                        />
                        <span>Re-encode video &amp; audio</span>
                    </label>
                    <p
                        v-if="!copyCompatibility.compatible"
                        class="convert__note convert__note--warn"
                    >
                        {{ copyCompatibility.reason }}
                    </p>
                    <p
                        v-else-if="!willReencode"
                        class="convert__note convert__note--ok"
                    >
                        Stream copy — audio and video are copied without re-encoding.
                    </p>
                    <p v-else class="convert__note">
                        Video and audio will be re-encoded for the output file.
                    </p>
                </div>

                <p v-if="statusText" class="convert__status">{{ statusText }}</p>

                <div class="convert__actions">
                    <button
                        class="convert__btn"
                        type="button"
                        @click="emit('close')"
                    >
                        Cancel
                    </button>
                    <button
                        class="convert__btn convert__btn--primary"
                        type="button"
                        :disabled="!canConvert"
                        @click="runConvert"
                    >
                        {{
                            busy
                                ? "Working…"
                                : willReencode
                                  ? "Convert (re-encode) & Save…"
                                  : "Convert & Save…"
                        }}
                    </button>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.convert {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: flex;
    align-items: center;
    justify-content: center;
}
.convert__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
}
.convert__box {
    position: relative;
    width: min(520px, 92vw);
    max-height: 88vh;
    overflow-y: auto;
    padding: 20px;
    border-radius: 14px;
    background: rgba(24, 26, 31, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.14);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    color: #fff;
}
.convert__title {
    font-size: 15px;
    font-weight: 700;
}
.convert__hint {
    margin: 6px 0 12px;
    font-size: 12.5px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.55);
}
.convert__file-box {
    padding: 9px 12px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.04);
    margin-bottom: 14px;
}
.convert__file-name {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.convert__file-meta {
    margin-top: 3px;
    font-size: 11.5px;
    color: rgba(255, 255, 255, 0.55);
}
.convert__section {
    margin: 12px 0;
}
.convert__label {
    font-size: 12px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.72);
    margin-bottom: 7px;
}
.convert__pills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
}
.convert__pill {
    padding: 6px 12px;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.8);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
}
.convert__pill:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.11);
}
.convert__pill--active {
    border-color: rgba(196, 160, 255, 0.6);
    background: rgba(170, 130, 255, 0.32);
    color: #fff;
}
.convert__pill:disabled {
    opacity: 0.45;
    cursor: default;
}
.convert__checkbox {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.88);
    cursor: pointer;
}
.convert__note {
    margin: 6px 0 0;
    font-size: 12px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.58);
}
.convert__note--warn {
    color: #ffb454;
}
.convert__note--ok {
    color: #7fd08a;
}
.convert__status {
    margin: 8px 0 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.72);
}
.convert__btn {
    padding: 8px 14px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}
.convert__btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
}
.convert__btn:disabled {
    opacity: 0.5;
    cursor: default;
}
.convert__actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
}
.convert__btn--primary {
    border-color: rgba(196, 160, 255, 0.5);
    background: rgba(170, 130, 255, 0.28);
    color: #fff;
}
.convert__btn--primary:hover:not(:disabled) {
    background: rgba(170, 130, 255, 0.42);
}
.convert-fade-enter-active,
.convert-fade-leave-active {
    transition: opacity 0.15s ease;
}
.convert-fade-enter-from,
.convert-fade-leave-to {
    opacity: 0;
}
</style>
