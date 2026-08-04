<script setup lang="ts">
import { ref, watch } from "vue";
import type { YoutubeItem } from "../../composables/useYouTubeModule";
import {
    DEFAULT_SUBTITLE_LANGUAGE,
    YOUTUBE_SUBTITLE_LANGUAGES,
} from "../../constants/youtubeSubtitleLanguages";
import { useYouTubeSettings } from "../../composables/useYouTubeSettings";

const props = defineProps<{
    open: boolean;
    item: YoutubeItem | null;
    queueAhead: number;
}>();

const emit = defineEmits<{
    (e: "close"): void;
    (
        e: "confirm",
        payload: {
            qualityMaxHeight: number | null;
            container: string;
            audioOnly: boolean;
            audioFormat: string;
            embedSubs: boolean;
            subLangs: string;
            embedThumbnail: boolean;
            embedChapters: boolean;
            front: boolean;
        },
    ): void;
}>();

const QUALITIES: { height: number | null; label: string; note: string }[] = [
    { height: 2160, label: "2160p", note: "4K · large" },
    { height: 1440, label: "1440p", note: "QHD" },
    { height: 1080, label: "1080p", note: "Full HD" },
    { height: 720, label: "720p", note: "Smaller" },
    { height: 480, label: "480p", note: "Smallest" },
    { height: null, label: "Best available", note: "No cap" },
];

// Downloads start from the same default as playback (Settings → YouTube).
const { settings: youtubeSettings } = useYouTubeSettings();

const qualityNote = (option: (typeof QUALITIES)[number]) =>
    option.height === youtubeSettings.qualityMaxHeight
        ? "Default"
        : option.note;

const quality = ref<number | null>(youtubeSettings.qualityMaxHeight);
const container = ref("mp4");
const audioOnly = ref(false);
const audioFormat = ref("mp3");
const embedSubs = ref(false);
// One language keeps a single track; a pattern like "en.*" would pull en,
// en-orig, en-US… and save each of them.
const subLangs = ref(DEFAULT_SUBTITLE_LANGUAGE);
const embedThumbnail = ref(true);
const embedChapters = ref(true);

watch(
    () => props.open,
    (open) => {
        if (!open) return;
        // Fresh defaults each time the dialog opens.
        quality.value = youtubeSettings.qualityMaxHeight;
        container.value = "mp4";
        audioOnly.value = false;
        embedSubs.value = false;
        subLangs.value = DEFAULT_SUBTITLE_LANGUAGE;
        embedThumbnail.value = true;
        embedChapters.value = true;
    },
);

const confirm = (front: boolean) => {
    emit("confirm", {
        qualityMaxHeight: quality.value,
        container: container.value,
        audioOnly: audioOnly.value,
        audioFormat: audioFormat.value,
        embedSubs: embedSubs.value,
        subLangs: subLangs.value,
        embedThumbnail: embedThumbnail.value,
        embedChapters: embedChapters.value,
        front,
    });
};
</script>

<template>
    <transition name="ytdl-fade">
        <div
            v-if="props.open && props.item"
            class="ytdl"
            data-window-no-drag
            @keydown.esc.stop.prevent="emit('close')"
        >
            <div class="ytdl__backdrop" @click="emit('close')"></div>
            <div class="ytdl__box" role="dialog" aria-label="Download video">
                <div class="ytdl__head">
                    <div class="ytdl__title">{{ props.item.title }}</div>
                    <div class="ytdl__sub">
                        {{ props.item.channel || "" }}
                        <span v-if="props.item.durationText">
                            · {{ props.item.durationText }}</span
                        >
                    </div>
                </div>

                <div class="ytdl__body">
                    <div class="ytdl__section">
                        <div class="ytdl__label">Quality</div>
                        <label
                            v-for="option in QUALITIES"
                            :key="option.label"
                            class="ytdl__radio"
                            :class="{ 'ytdl__radio--disabled': audioOnly }"
                        >
                            <input
                                type="radio"
                                name="yt-quality"
                                :value="option.height"
                                :checked="quality === option.height"
                                :disabled="audioOnly"
                                @change="quality = option.height"
                            />
                            <span class="ytdl__radio-label">{{
                                option.label
                            }}</span>
                            <span class="ytdl__radio-note">{{
                                qualityNote(option)
                            }}</span>
                        </label>
                    </div>

                    <div class="ytdl__section">
                        <div class="ytdl__label">Container</div>
                        <div class="ytdl__seg">
                            <button
                                v-for="value in ['mp4', 'mkv', 'webm']"
                                :key="value"
                                class="ytdl__seg-btn"
                                :class="{
                                    'ytdl__seg-btn--active':
                                        container === value && !audioOnly,
                                }"
                                type="button"
                                :disabled="audioOnly"
                                @click="container = value"
                            >
                                {{ value.toUpperCase() }}
                            </button>
                        </div>
                        <div class="ytdl__hint">
                            MKV keeps any codec without re-encoding.
                        </div>
                    </div>

                    <div class="ytdl__section">
                        <div class="ytdl__label">Extras</div>
                        <label class="ytdl__check">
                            <input v-model="audioOnly" type="checkbox" />
                            <span>Audio only</span>
                            <select
                                v-model="audioFormat"
                                class="ytdl__select"
                                :disabled="!audioOnly"
                            >
                                <option value="mp3">MP3</option>
                                <option value="m4a">M4A</option>
                            </select>
                        </label>
                        <label class="ytdl__check">
                            <input v-model="embedSubs" type="checkbox" />
                            <span>Subtitles</span>
                            <select
                                v-model="subLangs"
                                class="ytdl__select ytdl__select--lang"
                                :disabled="!embedSubs"
                                :title="
                                    embedSubs
                                        ? 'Auto-translated when the video has no track in this language'
                                        : ''
                                "
                            >
                                <option
                                    v-for="language in YOUTUBE_SUBTITLE_LANGUAGES"
                                    :key="language.code"
                                    :value="language.code"
                                >
                                    {{ language.name }}
                                </option>
                            </select>
                        </label>
                        <label class="ytdl__check">
                            <input v-model="embedThumbnail" type="checkbox" />
                            <span>Embed thumbnail cover</span>
                        </label>
                        <label class="ytdl__check">
                            <input v-model="embedChapters" type="checkbox" />
                            <span>Keep chapters → become Lumo Scenes</span>
                        </label>
                    </div>
                </div>

                <div class="ytdl__foot">
                    <span class="ytdl__queue">
                        {{
                            props.queueAhead === 0
                                ? "Nothing ahead in the queue"
                                : `${props.queueAhead} download${props.queueAhead === 1 ? "" : "s"} ahead of this one`
                        }}
                    </span>
                    <div class="ytdl__actions">
                        <button
                            class="ytdl__btn"
                            type="button"
                            @click="emit('close')"
                        >
                            Cancel
                        </button>
                        <button
                            class="ytdl__btn ytdl__btn--outline"
                            type="button"
                            @click="confirm(false)"
                        >
                            Add to queue
                        </button>
                        <button
                            class="ytdl__btn ytdl__btn--primary"
                            type="button"
                            @click="confirm(true)"
                        >
                            Download now
                        </button>
                    </div>
                </div>
            </div>
        </div>
    </transition>
</template>

<style scoped>
.ytdl {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    align-items: center;
    justify-content: center;
}

.ytdl__backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(2px);
}

.ytdl__box {
    position: relative;
    width: min(520px, 92vw);
    max-height: 86vh;
    display: flex;
    flex-direction: column;
    background: var(--yt-bg, #141417);
    border: 1px solid var(--yt-border, #26262c);
    border-radius: 13px;
    color: var(--yt-text, #ececef);
    box-shadow: 0 24px 60px rgba(0, 0, 0, 0.55);
    overflow: hidden;
}

.ytdl__head {
    padding: 14px 16px 10px;
    border-bottom: 1px solid var(--yt-border, #26262c);
}

.ytdl__title {
    font-size: 14px;
    font-weight: 700;
    line-height: 1.35;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
}

.ytdl__sub {
    margin-top: 3px;
    font-size: 12px;
    color: var(--yt-text-dim, #9a9aa2);
}

.ytdl__body {
    padding: 12px 16px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
}

.ytdl__label {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--yt-text-faint, #6f6f78);
    margin-bottom: 7px;
}

.ytdl__radio {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 8px;
    border-radius: 8px;
    cursor: pointer;
    font-size: 13px;
}

.ytdl__radio:hover {
    background: var(--yt-card-hover, #212127);
}

.ytdl__radio--disabled {
    opacity: 0.45;
    cursor: default;
}

.ytdl__radio-note {
    margin-left: auto;
    font-size: 11.5px;
    color: var(--yt-text-faint, #6f6f78);
}

.ytdl__seg {
    display: inline-flex;
    gap: 4px;
    padding: 3px;
    border-radius: 9px;
    background: var(--yt-card, #1c1c21);
    border: 1px solid var(--yt-border, #26262c);
}

.ytdl__seg-btn {
    border: none;
    background: transparent;
    color: var(--yt-text-dim, #9a9aa2);
    font-size: 12px;
    font-weight: 700;
    padding: 4px 14px;
    border-radius: 7px;
    cursor: pointer;
}

.ytdl__seg-btn--active {
    background: rgba(139, 124, 247, 0.2);
    color: var(--yt-text, #ececef);
}

.ytdl__seg-btn:disabled {
    opacity: 0.45;
    cursor: default;
}

.ytdl__hint {
    margin-top: 6px;
    font-size: 11.5px;
    color: var(--yt-text-faint, #6f6f78);
}

.ytdl__check {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 5px 0;
    font-size: 13px;
    cursor: pointer;
}

.ytdl__select,
.ytdl__input {
    margin-left: auto;
    background: var(--yt-card, #1c1c21);
    border: 1px solid var(--yt-border, #26262c);
    border-radius: 7px;
    color: var(--yt-text, #ececef);
    font-size: 12px;
    padding: 3px 7px;
    outline: none;
}

.ytdl__input {
    width: 150px;
}

.ytdl__select--lang {
    max-width: 190px;
}

.ytdl__select:disabled,
.ytdl__input:disabled {
    opacity: 0.45;
}

.ytdl__foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 11px 16px;
    border-top: 1px solid var(--yt-border, #26262c);
}

.ytdl__queue {
    font-size: 11.5px;
    color: var(--yt-text-faint, #6f6f78);
}

.ytdl__actions {
    display: flex;
    gap: 7px;
}

.ytdl__btn {
    border: 1px solid transparent;
    background: transparent;
    color: var(--yt-text-dim, #9a9aa2);
    font-size: 12.5px;
    font-weight: 600;
    padding: 6px 13px;
    border-radius: 8px;
    cursor: pointer;
}

.ytdl__btn:hover {
    color: var(--yt-text, #ececef);
}

.ytdl__btn--outline {
    border-color: var(--yt-accent, #8b7cf7);
    color: var(--yt-accent, #8b7cf7);
}

.ytdl__btn--primary {
    background: var(--yt-accent, #8b7cf7);
    color: #17151f;
    font-weight: 700;
}

.ytdl__btn--primary:hover {
    filter: brightness(1.08);
    color: #17151f;
}

.ytdl-fade-enter-active,
.ytdl-fade-leave-active {
    transition: opacity 0.15s ease;
}

.ytdl-fade-enter-from,
.ytdl-fade-leave-to {
    opacity: 0;
}
</style>
