<script setup lang="ts">
import type { YoutubeItem } from "../../composables/useYouTubeModule";
import type {
    CaptionTrack,
    YoutubeChapter,
    YoutubeComment,
} from "../../composables/useYouTubeWatch";

type DrawerTab = "upnext" | "chapters" | "captions" | "comments";

const props = defineProps<{
    open: boolean;
    activeTab: DrawerTab;
    related: YoutubeItem[];
    chapters: YoutubeChapter[];
    isLoading: boolean;
    autoplayNext: boolean;
    currentTime: number;
    captionTracks: CaptionTrack[];
    isLoadingCaptions: boolean;
    loadingCaptionCode: string;
    loadedCaptionCodes: string[];
    comments: YoutubeComment[];
    commentsTotal: string;
    commentsCursor: string | null;
    isLoadingComments: boolean;
    commentsError: string;
    isTranslating: boolean;
    showTranslated: boolean;
    translateLanguage: string;
    selectedCommentIds: string[];
    aiProvider: string;
    aiModel: string;
    aiProviders: readonly string[];
    aiModels: readonly string[];
}>();

const emit = defineEmits<{
    (e: "close"): void;
    (e: "set-tab", tab: DrawerTab): void;
    (e: "set-autoplay", enabled: boolean): void;
    (e: "play", payload: { url: string; title?: string }): void;
    (e: "seek", seconds: number): void;
    (e: "use-caption", track: CaptionTrack): void;
    (e: "load-more-comments"): void;
    (e: "translate-comments", onlySelected: boolean): void;
    (e: "toggle-translated", value: boolean): void;
    (e: "toggle-comment-selection", id: string): void;
    (e: "set-ai-provider", provider: string): void;
    (e: "set-ai-model", model: string): void;
}>();

const chapterIndexAt = (time: number) => {
    let index = -1;
    for (let i = 0; i < props.chapters.length; i += 1) {
        if (props.chapters[i].startSeconds <= time) index = i;
    }
    return index;
};

const formatTime = (seconds: number) => {
    const total = Math.max(0, Math.floor(seconds));
    const hours = Math.floor(total / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    const secs = total % 60;
    return hours > 0
        ? `${hours}:${String(minutes).padStart(2, "0")}:${String(secs).padStart(2, "0")}`
        : `${minutes}:${String(secs).padStart(2, "0")}`;
};
</script>

<template>
    <transition name="yt-drawer">
        <aside
            v-if="props.open"
            class="yt-drawer"
            data-window-no-drag
            aria-label="YouTube panel"
            @keydown.esc.stop.prevent="emit('close')"
        >
            <div class="yt-drawer__head">
                <div class="yt-drawer__tabs" role="tablist">
                    <button
                        class="yt-drawer__tab"
                        :class="{
                            'yt-drawer__tab--active': props.activeTab === 'upnext',
                        }"
                        type="button"
                        role="tab"
                        @click="emit('set-tab', 'upnext')"
                    >
                        Up next
                    </button>
                    <button
                        class="yt-drawer__tab"
                        :class="{
                            'yt-drawer__tab--active':
                                props.activeTab === 'chapters',
                        }"
                        type="button"
                        role="tab"
                        @click="emit('set-tab', 'chapters')"
                    >
                        Chapters
                        <span
                            v-if="props.chapters.length"
                            class="yt-drawer__count"
                            >{{ props.chapters.length }}</span
                        >
                    </button>
                    <button
                        class="yt-drawer__tab"
                        :class="{
                            'yt-drawer__tab--active':
                                props.activeTab === 'comments',
                        }"
                        type="button"
                        role="tab"
                        @click="emit('set-tab', 'comments')"
                    >
                        Comments
                    </button>
                </div>
                <button
                    class="yt-drawer__close"
                    type="button"
                    title="Close"
                    aria-label="Close panel"
                    @click="emit('close')"
                >
                    ✕
                </button>
            </div>

            <template v-if="props.activeTab === 'upnext'">
                <button
                    class="yt-drawer__autoplay"
                    type="button"
                    :aria-pressed="props.autoplayNext"
                    @click="emit('set-autoplay', !props.autoplayNext)"
                >
                    <span>Autoplay next</span>
                    <span
                        class="yt-drawer__switch"
                        :class="{
                            'yt-drawer__switch--on': props.autoplayNext,
                        }"
                    >
                        <span class="yt-drawer__switch-thumb"></span>
                    </span>
                </button>
                <div class="yt-drawer__list">
                    <div v-if="props.isLoading" class="yt-drawer__state">
                        Loading…
                    </div>
                    <div
                        v-else-if="!props.related.length"
                        class="yt-drawer__state"
                    >
                        No related videos
                    </div>
                    <div
                        v-for="item in props.related"
                        :key="item.id"
                        class="yt-drawer__row"
                        role="button"
                        tabindex="0"
                        @click="emit('play', { url: item.url, title: item.title })"
                        @keydown.enter="
                            emit('play', { url: item.url, title: item.title })
                        "
                    >
                        <div class="yt-drawer__thumb">
                            <img
                                v-if="item.thumbnailUrl"
                                :src="item.thumbnailUrl"
                                alt=""
                                loading="lazy"
                                referrerpolicy="no-referrer"
                            />
                            <span
                                v-if="item.durationText"
                                class="yt-drawer__badge"
                                >{{ item.durationText }}</span
                            >
                        </div>
                        <div class="yt-drawer__meta">
                            <div class="yt-drawer__title">{{ item.title }}</div>
                            <div class="yt-drawer__sub">
                                {{ item.channel || "" }}
                            </div>
                        </div>
                    </div>
                </div>
            </template>

            <template v-else-if="props.activeTab === 'comments'">
                <div class="yt-drawer__comments-head">
                    <div class="yt-drawer__comments-row">
                        <span class="yt-drawer__tagline">{{
                            props.commentsTotal || "Comments"
                        }}</span>
                        <button
                            v-if="props.showTranslated"
                            class="yt-drawer__translate"
                            type="button"
                            @click="emit('toggle-translated', false)"
                        >
                            Show original
                        </button>
                    </div>
                    <template v-if="props.comments.length">
                        <div class="yt-drawer__comments-row">
                            <select
                                class="yt-drawer__select"
                                :value="props.aiProvider"
                                :title="`Translate to ${props.translateLanguage} using…`"
                                @change="
                                    emit(
                                        'set-ai-provider',
                                        ($event.target as HTMLSelectElement)
                                            .value,
                                    )
                                "
                            >
                                <option
                                    v-for="provider in props.aiProviders"
                                    :key="provider"
                                    :value="provider"
                                >
                                    {{ provider }}
                                </option>
                            </select>
                            <select
                                class="yt-drawer__select"
                                :value="props.aiModel"
                                @change="
                                    emit(
                                        'set-ai-model',
                                        ($event.target as HTMLSelectElement)
                                            .value,
                                    )
                                "
                            >
                                <option
                                    v-for="model in props.aiModels"
                                    :key="model"
                                    :value="model"
                                >
                                    {{ model }}
                                </option>
                            </select>
                        </div>
                        <div class="yt-drawer__comments-row">
                            <button
                                class="yt-drawer__translate"
                                type="button"
                                :disabled="props.isTranslating"
                                @click="emit('translate-comments', false)"
                            >
                                {{
                                    props.isTranslating
                                        ? "Translating…"
                                        : `Translate all to ${props.translateLanguage}`
                                }}
                            </button>
                            <button
                                class="yt-drawer__translate"
                                type="button"
                                :disabled="
                                    props.isTranslating ||
                                    !props.selectedCommentIds.length
                                "
                                @click="emit('translate-comments', true)"
                            >
                                Translate selected ({{
                                    props.selectedCommentIds.length
                                }})
                            </button>
                        </div>
                    </template>
                </div>
                <div class="yt-drawer__list">
                    <div
                        v-if="props.commentsError"
                        class="yt-drawer__state yt-drawer__state--error"
                    >
                        {{ props.commentsError }}
                    </div>
                    <div
                        v-else-if="
                            props.isLoadingComments && !props.comments.length
                        "
                        class="yt-drawer__state"
                    >
                        Loading comments…
                    </div>
                    <div
                        v-else-if="!props.comments.length"
                        class="yt-drawer__state"
                    >
                        No comments
                    </div>
                    <div
                        v-for="comment in props.comments"
                        :key="comment.id"
                        class="yt-drawer__comment"
                    >
                        <div class="yt-drawer__comment-head">
                            <input
                                class="yt-drawer__check"
                                type="checkbox"
                                :checked="
                                    props.selectedCommentIds.includes(
                                        comment.id,
                                    )
                                "
                                :aria-label="`Select comment by ${comment.author}`"
                                @change="
                                    emit(
                                        'toggle-comment-selection',
                                        comment.id,
                                    )
                                "
                            />
                            <img
                                v-if="comment.authorThumbnail"
                                class="yt-drawer__avatar"
                                :src="comment.authorThumbnail"
                                alt=""
                                loading="lazy"
                                referrerpolicy="no-referrer"
                            />
                            <span class="yt-drawer__comment-author">{{
                                comment.author
                            }}</span>
                            <span class="yt-drawer__tagline">{{
                                comment.publishedText
                            }}</span>
                        </div>
                        <div class="yt-drawer__comment-text">
                            {{
                                props.showTranslated && comment.translated
                                    ? comment.translated
                                    : comment.text
                            }}
                        </div>
                        <div class="yt-drawer__comment-meta">
                            <span v-if="comment.likeCountText"
                                >♥ {{ comment.likeCountText }}</span
                            >
                            <span v-if="comment.replyCountText"
                                >💬 {{ comment.replyCountText }}</span
                            >
                        </div>
                    </div>
                    <button
                        v-if="props.commentsCursor"
                        class="yt-drawer__more"
                        type="button"
                        :disabled="props.isLoadingComments"
                        @click="emit('load-more-comments')"
                    >
                        {{
                            props.isLoadingComments ? "Loading…" : "Load more"
                        }}
                    </button>
                </div>
            </template>

            <div v-else class="yt-drawer__list">
                <div v-if="!props.chapters.length" class="yt-drawer__state">
                    No chapters in this video
                </div>
                <div
                    v-for="(chapter, index) in props.chapters"
                    :key="chapter.startSeconds"
                    class="yt-drawer__row yt-drawer__row--chapter"
                    :class="{
                        'yt-drawer__row--now':
                            index === chapterIndexAt(props.currentTime),
                    }"
                    role="button"
                    tabindex="0"
                    @click="emit('seek', chapter.startSeconds)"
                    @keydown.enter="emit('seek', chapter.startSeconds)"
                >
                    <span class="yt-drawer__chapter-time">{{
                        formatTime(chapter.startSeconds)
                    }}</span>
                    <span class="yt-drawer__title">{{ chapter.title }}</span>
                    <span
                        v-if="index === chapterIndexAt(props.currentTime)"
                        class="yt-drawer__now"
                        >NOW</span
                    >
                </div>
            </div>
        </aside>
    </transition>
</template>

<style scoped>
.yt-drawer {
    position: fixed;
    top: calc(var(--top-bar-height) + 8px);
    right: 12px;
    bottom: calc(var(--controls-bar-height) + 12px);
    width: 330px;
    background: var(--yt-bg, #141417);
    border: 1px solid var(--yt-border, #26262c);
    border-radius: 12px;
    z-index: 115;
    display: flex;
    flex-direction: column;
    color: var(--yt-text, #ececef);
    overflow: hidden;
}

.yt-drawer-enter-active,
.yt-drawer-leave-active {
    transition:
        transform 0.18s ease,
        opacity 0.18s ease;
}

.yt-drawer-enter-from,
.yt-drawer-leave-to {
    transform: translateX(24px);
    opacity: 0;
}

.yt-drawer__head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 10px 8px;
    border-bottom: 1px solid var(--yt-border, #26262c);
}

.yt-drawer__tabs {
    display: flex;
    gap: 6px;
}

.yt-drawer__tab {
    border: 1px solid var(--yt-border, #26262c);
    background: var(--yt-card, #1c1c21);
    color: var(--yt-text-dim, #9a9aa2);
    border-radius: 999px;
    padding: 4px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}

.yt-drawer__tab--active {
    background: rgba(139, 124, 247, 0.16);
    border-color: var(--yt-accent, #8b7cf7);
    color: var(--yt-text, #ececef);
}

.yt-drawer__count {
    margin-left: 4px;
    color: var(--yt-text-faint, #6f6f78);
}

.yt-drawer__close {
    border: none;
    background: transparent;
    color: var(--yt-text-dim, #9a9aa2);
    width: 26px;
    height: 26px;
    border-radius: 7px;
    cursor: pointer;
}

.yt-drawer__close:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--yt-text, #ececef);
}

.yt-drawer__autoplay {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 8px 10px 0;
    padding: 7px 10px;
    border: 1px solid var(--yt-border, #26262c);
    border-radius: 9px;
    background: var(--yt-card, #1c1c21);
    color: var(--yt-text, #ececef);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
}

.yt-drawer__switch {
    width: 28px;
    height: 14px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.22);
    display: inline-flex;
    align-items: center;
    padding: 2px;
    transition: background-color 0.15s ease;
}

.yt-drawer__switch--on {
    background: var(--yt-accent, #8b7cf7);
}

.yt-drawer__switch-thumb {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #fff;
    transition: transform 0.15s ease;
}

.yt-drawer__switch--on .yt-drawer__switch-thumb {
    transform: translateX(14px);
}

.yt-drawer__list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 8px 10px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    scrollbar-width: thin;
    scrollbar-color: var(--yt-border-hover, #38383f) transparent;
}

.yt-drawer__state {
    color: var(--yt-text-faint, #6f6f78);
    font-size: 12.5px;
    text-align: center;
    padding: 18px 0;
}

.yt-drawer__row {
    display: flex;
    align-items: center;
    gap: 9px;
    background: var(--yt-card, #1c1c21);
    border: 1px solid var(--yt-border, #26262c);
    border-radius: 9px;
    padding: 6px;
    cursor: pointer;
    transition:
        background-color 0.13s ease,
        border-color 0.13s ease;
}

.yt-drawer__row:hover {
    background: var(--yt-card-hover, #212127);
    border-color: var(--yt-border-hover, #38383f);
}

.yt-drawer__row--now {
    border-color: var(--yt-accent, #8b7cf7);
}

.yt-drawer__thumb {
    position: relative;
    width: 96px;
    height: 54px;
    flex: 0 0 auto;
    border-radius: 6px;
    overflow: hidden;
    background: #101013;
}

.yt-drawer__thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
}

.yt-drawer__badge {
    position: absolute;
    right: 3px;
    bottom: 3px;
    background: rgba(0, 0, 0, 0.82);
    color: #fff;
    font-size: 9.5px;
    font-weight: 700;
    border-radius: 3px;
    padding: 0 4px;
}

.yt-drawer__meta {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
}

.yt-drawer__title {
    font-size: 12px;
    font-weight: 600;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
}

.yt-drawer__sub {
    font-size: 11px;
    color: var(--yt-text-dim, #9a9aa2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.yt-drawer__row--chapter {
    gap: 8px;
    padding: 7px 9px;
}

.yt-drawer__chapter-time {
    color: var(--yt-progress, #2f8ef5);
    font-size: 11.5px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    flex: 0 0 auto;
}

.yt-drawer__now {
    margin-left: auto;
    color: var(--yt-accent, #8b7cf7);
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.06em;
}

/* --- captions & comments ---------------------------------------------- */

.yt-drawer__row--caption {
    width: 100%;
    text-align: left;
    font: inherit;
    color: inherit;
}

.yt-drawer__row--caption:disabled {
    opacity: 0.6;
    cursor: default;
}

.yt-drawer__tagline {
    font-size: 11px;
    color: var(--yt-text-faint, #6f6f78);
}

.yt-drawer__comments-head {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px 0;
}

.yt-drawer__comments-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
}

.yt-drawer__select {
    flex: 1 1 0;
    min-width: 0;
    border: 1px solid var(--yt-border, #26262c);
    background: var(--yt-surface, #17171b);
    color: var(--yt-text, #ececef);
    border-radius: 6px;
    padding: 3px 6px;
    font-size: 11.5px;
}

.yt-drawer__check {
    flex: none;
    accent-color: var(--yt-accent, #8b7cf7);
    cursor: pointer;
}

.yt-drawer__translate {
    border: 1px solid var(--yt-accent, #8b7cf7);
    background: transparent;
    color: var(--yt-accent, #8b7cf7);
    border-radius: 999px;
    padding: 3px 10px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
}

.yt-drawer__translate:disabled {
    opacity: 0.6;
    cursor: default;
}

.yt-drawer__comment {
    border-bottom: 1px solid var(--yt-border, #26262c);
    padding: 8px 2px 10px;
    display: flex;
    flex-direction: column;
    gap: 5px;
}

.yt-drawer__comment-head {
    display: flex;
    align-items: center;
    gap: 7px;
}

.yt-drawer__avatar {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    flex: none;
}

.yt-drawer__comment-author {
    font-size: 12px;
    font-weight: 700;
    color: var(--yt-text, #ececef);
}

.yt-drawer__comment-text {
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--yt-text, #ececef);
    white-space: pre-wrap;
    word-break: break-word;
}

.yt-drawer__comment-meta {
    display: flex;
    gap: 12px;
    font-size: 11px;
    color: var(--yt-text-faint, #6f6f78);
}

.yt-drawer__more {
    margin: 8px auto;
    border: 1px solid var(--yt-border, #26262c);
    background: var(--yt-card, #1c1c21);
    color: var(--yt-text-dim, #9a9aa2);
    border-radius: 999px;
    padding: 6px 18px;
    font-size: 12px;
    cursor: pointer;
}

.yt-drawer__state--error {
    color: var(--yt-heart, #e8556d);
}
</style>
