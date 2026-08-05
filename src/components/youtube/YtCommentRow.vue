<script setup lang="ts">
import type { YoutubeComment } from "../../composables/useYouTubeWatch";

defineProps<{
    comment: YoutubeComment;
    selected: boolean;
    showTranslated: boolean;
    /** Replies render the same way, just narrower and indented. */
    isReply?: boolean;
}>();

const emit = defineEmits<{
    (e: "toggle-selection", id: string): void;
}>();
</script>

<template>
    <div class="yt-comment" :class="{ 'yt-comment--reply': isReply }">
        <div class="yt-comment__head">
            <input
                class="yt-comment__check"
                type="checkbox"
                :checked="selected"
                :aria-label="`Select comment by ${comment.author}`"
                @change="emit('toggle-selection', comment.id)"
            />
            <img
                v-if="comment.authorThumbnail"
                class="yt-comment__avatar"
                :src="comment.authorThumbnail"
                alt=""
                loading="lazy"
                referrerpolicy="no-referrer"
            />
            <span class="yt-comment__author">{{ comment.author }}</span>
            <span class="yt-comment__tagline">{{ comment.publishedText }}</span>
        </div>
        <div class="yt-comment__text">
            {{
                showTranslated && comment.translated
                    ? comment.translated
                    : comment.text
            }}
        </div>
        <div class="yt-comment__meta">
            <span v-if="comment.likeCountText">♥ {{ comment.likeCountText }}</span>
            <slot name="actions" />
        </div>
    </div>
</template>

<style scoped>
.yt-comment {
    border-bottom: 1px solid var(--yt-border, #26262c);
    padding: 8px 2px 10px;
    display: flex;
    flex-direction: column;
    gap: 5px;
}

.yt-comment--reply {
    border-bottom: none;
    padding: 6px 2px 6px 10px;
    margin-left: 12px;
    border-left: 2px solid var(--yt-border, #26262c);
}

.yt-comment__head {
    display: flex;
    align-items: center;
    gap: 7px;
}

.yt-comment__check {
    flex: none;
    accent-color: var(--yt-accent, #8b7cf7);
    cursor: pointer;
}

.yt-comment__avatar {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    flex: none;
}

.yt-comment--reply .yt-comment__avatar {
    width: 16px;
    height: 16px;
}

.yt-comment__author {
    font-size: 12px;
    font-weight: 700;
    color: var(--yt-text, #ececef);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.yt-comment--reply .yt-comment__author {
    font-size: 11.5px;
}

.yt-comment__tagline {
    font-size: 11px;
    color: var(--yt-text-faint, #6f6f78);
    flex: none;
}

.yt-comment__text {
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--yt-text, #ececef);
    white-space: pre-wrap;
    word-break: break-word;
}

.yt-comment--reply .yt-comment__text {
    font-size: 12px;
}

.yt-comment__meta {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 11px;
    color: var(--yt-text-faint, #6f6f78);
}
</style>
