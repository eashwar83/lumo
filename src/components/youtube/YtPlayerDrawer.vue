<script setup lang="ts">
import { computed } from "vue";
import YtCommentRow from "./YtCommentRow.vue";
import type { YoutubeItem } from "../../composables/useYouTubeModule";
import type {
    CaptionTrack,
    CommentSortOption,
    ReplyThread,
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
    /** What to render: the search result when searching, else all. */
    visibleList: YoutubeComment[];
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
    isFetchingModels: boolean;
    commentSortOptions: CommentSortOption[];
    activeCommentSort: string;
    commentsCount: number | null;
    replyThreads: Record<string, ReplyThread>;
    commentQuery: string;
    matchCount: number;
    isCrawling: boolean;
    crawlLoaded: number;
    crawlStopped: boolean;
    isExporting: boolean;
}>();

const replyThreadOf = (comment: YoutubeComment): ReplyThread | undefined =>
    props.replyThreads[comment.id];

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
    (e: "fetch-ai-models"): void;
    (e: "set-comment-sort", option: CommentSortOption): void;
    (e: "comments-scroll", element: HTMLElement): void;
    (e: "clear-comment-selection"): void;
    (e: "toggle-replies", comment: YoutubeComment): void;
    (e: "load-more-replies", comment: YoutubeComment): void;
    (e: "set-comment-query", value: string): void;
    (e: "search-all"): void;
    (e: "stop-search"): void;
    (e: "export-comments", onlySelected: boolean): void;
}>();

// One button covers both scopes: ticking comments narrows it, clearing the
// selection widens it again — so there is never a wrong button to press.
const hasSelection = computed(() => props.selectedCommentIds.length > 0);
const exportLabel = computed(() =>
    hasSelection.value
        ? `Export ${props.selectedCommentIds.length} selected to PDF`
        : "Export all comments to PDF",
);

const searchStatus = computed(() => {
    if (props.isCrawling) {
        return `${props.matchCount} matches · reading all comments (${props.crawlLoaded} so far)`;
    }
    const found = `${props.matchCount} match${props.matchCount === 1 ? "" : "es"}`;
    if (props.crawlStopped) return `${found} · search stopped early`;
    return found;
});

const translateLabel = computed(() => {
    if (props.isTranslating) return "Translating…";
    return hasSelection.value
        ? `Translate ${props.selectedCommentIds.length} selected to ${props.translateLanguage}`
        : `Translate all to ${props.translateLanguage}`;
});

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
                            props.commentsCount !== null
                                ? `${props.comments.length} of ${props.commentsCount} comments`
                                : props.commentsTotal || "Comments"
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
                    <div class="yt-drawer__comments-row">
                        <input
                            :value="props.commentQuery"
                            class="yt-drawer__search"
                            type="search"
                            placeholder="Search comments…"
                            spellcheck="false"
                            @input="
                                emit(
                                    'set-comment-query',
                                    ($event.target as HTMLInputElement).value,
                                )
                            "
                            @keydown.stop
                        />
                    </div>
                    <div
                        v-if="props.commentQuery.trim() || props.isCrawling"
                        class="yt-drawer__comments-row yt-drawer__search-state"
                    >
                        <span>{{ searchStatus }}</span>
                        <button
                            v-if="props.isCrawling"
                            class="yt-drawer__icon-button"
                            type="button"
                            @click="emit('stop-search')"
                        >
                            Stop
                        </button>
                        <button
                            v-else-if="props.commentsCursor"
                            class="yt-drawer__icon-button"
                            type="button"
                            @click="emit('search-all')"
                        >
                            Search all
                        </button>
                    </div>
                    <div
                        v-if="props.commentSortOptions.length > 1"
                        class="yt-drawer__comments-row yt-drawer__sorts"
                    >
                        <button
                            v-for="option in props.commentSortOptions"
                            :key="option.title"
                            class="yt-drawer__sort"
                            :class="{
                                'yt-drawer__sort--active':
                                    props.activeCommentSort === option.title,
                            }"
                            type="button"
                            :disabled="props.isLoadingComments"
                            @click="emit('set-comment-sort', option)"
                        >
                            {{ option.title }}
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
                            <button
                                class="yt-drawer__icon-button"
                                type="button"
                                :disabled="props.isFetchingModels"
                                title="Fetch this provider's current models"
                                aria-label="Fetch this provider's current models"
                                @click="emit('fetch-ai-models')"
                            >
                                {{ props.isFetchingModels ? "…" : "⟳" }}
                            </button>
                        </div>
                        <div class="yt-drawer__comments-row">
                            <button
                                class="yt-drawer__translate"
                                type="button"
                                :disabled="props.isTranslating"
                                @click="
                                    emit(
                                        'translate-comments',
                                        hasSelection,
                                    )
                                "
                            >
                                {{ translateLabel }}
                            </button>
                            <button
                                v-if="hasSelection"
                                class="yt-drawer__icon-button"
                                type="button"
                                title="Clear selection"
                                aria-label="Clear selection"
                                @click="emit('clear-comment-selection')"
                            >
                                ✕
                            </button>
                            <button
                                class="yt-drawer__icon-button"
                                type="button"
                                :disabled="props.isExporting"
                                :title="exportLabel"
                                :aria-label="exportLabel"
                                @click="emit('export-comments', hasSelection)"
                            >
                                {{ props.isExporting ? "…" : "PDF" }}
                            </button>
                        </div>
                    </template>
                </div>
                <div
                    class="yt-drawer__list"
                    @scroll.passive="
                        emit(
                            'comments-scroll',
                            $event.currentTarget as HTMLElement,
                        )
                    "
                >
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
                        v-else-if="!props.visibleList.length"
                        class="yt-drawer__state"
                    >
                        {{
                            props.commentQuery.trim()
                                ? "No comments match that search"
                                : "No comments"
                        }}
                    </div>
                    <template
                        v-for="comment in props.visibleList"
                        :key="comment.id"
                    >
                        <YtCommentRow
                            :comment="comment"
                            :selected="
                                props.selectedCommentIds.includes(comment.id)
                            "
                            :show-translated="props.showTranslated"
                            @toggle-selection="
                                emit('toggle-comment-selection', $event)
                            "
                        >
                            <template #actions>
                                <button
                                    v-if="comment.replyToken"
                                    class="yt-drawer__replies-toggle"
                                    type="button"
                                    @click="emit('toggle-replies', comment)"
                                >
                                    💬
                                    {{
                                        replyThreadOf(comment)?.open
                                            ? "Hide replies"
                                            : `${comment.replyCountText ?? ""} replies`.trim()
                                    }}
                                    {{
                                        replyThreadOf(comment)?.open ? "▴" : "▾"
                                    }}
                                </button>
                                <span
                                    v-else-if="comment.replyCountText"
                                    >💬 {{ comment.replyCountText }}</span
                                >
                            </template>
                        </YtCommentRow>
                        <template v-if="replyThreadOf(comment)?.open">
                            <YtCommentRow
                                v-for="reply in replyThreadOf(comment)!.replies"
                                :key="reply.id"
                                :comment="reply"
                                :is-reply="true"
                                :selected="
                                    props.selectedCommentIds.includes(reply.id)
                                "
                                :show-translated="props.showTranslated"
                                @toggle-selection="
                                    emit('toggle-comment-selection', $event)
                                "
                            />
                            <div
                                v-if="replyThreadOf(comment)!.loading"
                                class="yt-drawer__replies-state"
                            >
                                Loading replies…
                            </div>
                            <div
                                v-else-if="replyThreadOf(comment)!.error"
                                class="yt-drawer__replies-state yt-drawer__state--error"
                            >
                                {{ replyThreadOf(comment)!.error }}
                            </div>
                            <button
                                v-else-if="replyThreadOf(comment)!.cursor"
                                class="yt-drawer__replies-more"
                                type="button"
                                @click="emit('load-more-replies', comment)"
                            >
                                Show more replies
                            </button>
                        </template>
                    </template>
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

.yt-drawer__icon-button {
    flex: none;
    border: 1px solid var(--yt-border, #26262c);
    background: var(--yt-surface, #17171b);
    color: var(--yt-text, #ececef);
    border-radius: 6px;
    padding: 3px 8px;
    font-size: 12px;
    line-height: 1.2;
    cursor: pointer;
}

.yt-drawer__icon-button:disabled {
    opacity: 0.6;
    cursor: default;
}

.yt-drawer__sorts {
    justify-content: flex-start;
}

.yt-drawer__search {
    flex: 1 1 auto;
    min-width: 0;
    border: 1px solid var(--yt-border, #26262c);
    background: var(--yt-surface, #17171b);
    color: var(--yt-text, #ececef);
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 12px;
}

.yt-drawer__search:focus {
    outline: none;
    border-color: var(--yt-accent, #8b7cf7);
}

.yt-drawer__search-state {
    font-size: 11px;
    color: var(--yt-text-faint, #6f6f78);
}

.yt-drawer__replies-toggle {
    border: none;
    background: transparent;
    padding: 0;
    color: var(--yt-accent, #8b7cf7);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
}

.yt-drawer__replies-state {
    margin-left: 24px;
    padding: 4px 0 8px;
    font-size: 11px;
    color: var(--yt-text-faint, #6f6f78);
}

.yt-drawer__replies-more {
    margin: 0 0 10px 24px;
    border: none;
    background: transparent;
    padding: 2px 0;
    color: var(--yt-accent, #8b7cf7);
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    text-align: left;
}

.yt-drawer__sort {
    border: 1px solid var(--yt-border, #26262c);
    background: transparent;
    color: var(--yt-text-faint, #6f6f78);
    border-radius: 999px;
    padding: 3px 12px;
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
}

.yt-drawer__sort--active {
    border-color: var(--yt-accent, #8b7cf7);
    color: var(--yt-accent, #8b7cf7);
}

.yt-drawer__sort:disabled {
    opacity: 0.6;
    cursor: default;
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
