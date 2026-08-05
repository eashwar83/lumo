import { computed, ref, watch, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { YoutubeItem } from "./useYouTubeModule";

export type YoutubeChapter = { title: string; startSeconds: number };

type YoutubeVideoContext = {
    related: YoutubeItem[];
    chapters: YoutubeChapter[];
};

export type SponsorSegment = {
    category: string;
    startSeconds: number;
    endSeconds: number;
};

export type CaptionTrack = { code: string; name: string; auto: boolean };

export type YoutubeComment = {
    id: string;
    author: string;
    authorThumbnail?: string | null;
    text: string;
    publishedText?: string | null;
    likeCountText?: string | null;
    replyCountText?: string | null;
    isPinned: boolean;
    isHearted: boolean;
    /** Continuation token for this comment's replies, when it has any. */
    replyToken?: string | null;
    /** Filled in by the AI translation pass. */
    translated?: string;
    /**
     * Which language+model produced `translated`. Changing either has to
     * invalidate the cached text, or the old translation sticks forever.
     */
    translatedBy?: string;
};

/** Replies to one comment, loaded on demand. */
export type ReplyThread = {
    open: boolean;
    loading: boolean;
    replies: YoutubeComment[];
    cursor: string | null;
    error: string;
};

export type CommentSortOption = {
    title: string;
    token: string;
    selected: boolean;
};

type CommentPage = {
    comments: YoutubeComment[];
    nextCursor: string | null;
    totalText: string | null;
    sortOptions: CommentSortOption[];
    totalCount: number | null;
};

type UseYouTubeWatchOptions = {
    mediaUrl: () => string;
    isFileLoaded: () => boolean;
    /** Replaces the transient Up-next playlist ([] clears it). */
    setUpNextQueue: (
        items: { path: string; title?: string; iconUrl?: string }[],
    ) => void;
    /** Pushes chapter marks onto the seek bar / scene navigation. */
    setSceneMarkers: (markers: { start: number; label: string }[]) => void;
    seekTo: (seconds: number) => void;
    /** Loads a downloaded caption file into the player. */
    addSubtitleFile: (path: string, title: string) => Promise<boolean>;
    notify: (message: string) => void;
    /** Closes panels that would overlap this drawer. */
    closeOtherDrawers?: () => void;
    /** Live values from Settings → YouTube. */
    settings: {
        autoplayNext: boolean;
        chaptersToScenes: boolean;
        sponsorBlockEnabled: boolean;
        sponsorCategories: string[];
    };
};

const AUTOPLAY_STORAGE_KEY = "lumo.youtubeAutoplayNext";

export const extractYoutubeVideoId = (url: string): string | null => {
    const stripped = url
        .replace(/^https?:\/\//i, "")
        .replace(/^(www\.|m\.)/i, "");
    const watchMatch = stripped.match(
        /^youtube\.com\/watch\?(?:.*&)?v=([\w-]{11})/i,
    );
    if (watchMatch) return watchMatch[1];
    const shortMatch = stripped.match(
        /^(?:youtu\.be\/|youtube\.com\/(?:shorts|live)\/)([\w-]{11})/i,
    );
    return shortMatch ? shortMatch[1] : null;
};

export const useYouTubeWatch = (options: UseYouTubeWatchOptions) => {
    const isDrawerOpen = ref(false);
    const activeTab: Ref<"upnext" | "chapters" | "captions" | "comments"> =
        ref("upnext");
    const related = ref<YoutubeItem[]>([]);
    const chapters = ref<YoutubeChapter[]>([]);
    const sponsorSegments = ref<SponsorSegment[]>([]);
    const isLoadingContext = ref(false);
    // Segment start-times already skipped (or undone) this session — a
    // segment fires at most once so deliberate seek-backs aren't fought.
    let suppressedSegments = new Set<number>();
    const sponsorToast = ref<{ from: number; to: number } | null>(null);
    let sponsorToastTimer: number | null = null;
    // Settings provide the default; the drawer toggle overrides per session.
    const storedAutoplay =
        typeof localStorage === "undefined"
            ? null
            : localStorage.getItem(AUTOPLAY_STORAGE_KEY);
    const autoplayNext = ref(
        storedAutoplay === null
            ? options.settings.autoplayNext
            : storedAutoplay !== "0",
    );

    const currentVideoId = computed(() =>
        options.isFileLoaded()
            ? extractYoutubeVideoId(options.mediaUrl() || "")
            : null,
    );
    const isYoutubeWatch = computed(() => currentVideoId.value !== null);

    let contextToken = 0;

    const applyUpNextQueue = () => {
        if (!autoplayNext.value || !related.value.length) {
            options.setUpNextQueue([]);
            return;
        }
        const current = options.mediaUrl();
        options.setUpNextQueue([
            // The current video leads the queue so Previous/Next know where
            // "here" is inside it.
            { path: current },
            ...related.value.map((item) => ({
                path: item.url,
                title: item.title,
            })),
        ]);
    };

    const setAutoplayNext = (enabled: boolean) => {
        autoplayNext.value = enabled;
        try {
            localStorage.setItem(AUTOPLAY_STORAGE_KEY, enabled ? "1" : "0");
        } catch {
            // Storage unavailable — session-only toggle.
        }
        applyUpNextQueue();
    };

    watch(
        currentVideoId,
        async (videoId) => {
            const token = ++contextToken;
            related.value = [];
            chapters.value = [];
            captionTracks.value = [];
            loadedCaptionCodes.value = [];
            comments.value = [];
            commentsCursor.value = null;
            commentsTotal.value = "";
            commentsCount.value = null;
            commentSortOptions.value = [];
            activeCommentSort.value = "";
            replyThreads.value = {};
            commentQuery.value = "";
            if (isCrawling.value) stopSearchingAll();
            crawlLoaded.value = 0;
            crawlStopped.value = false;
            commentsError.value = "";
            showTranslated.value = false;
            selectedCommentIds.value = [];
            captionStatus.value = "";
            sponsorSegments.value = [];
            suppressedSegments = new Set();
            sponsorToast.value = null;
            if (!videoId) {
                isDrawerOpen.value = false;
                options.setUpNextQueue([]);
                return;
            }
            if (options.settings.sponsorBlockEnabled) {
                invoke<SponsorSegment[]>("youtube_sponsorblock", {
                    videoId,
                    categories: options.settings.sponsorCategories,
                })
                    .then((segments) => {
                        if (token === contextToken) {
                            sponsorSegments.value = segments;
                        }
                    })
                    .catch(() => {});
            }
            isLoadingContext.value = true;
            try {
                const context = await invoke<YoutubeVideoContext>(
                    "youtube_video_context",
                    { videoId },
                );
                if (token !== contextToken) return;
                related.value = context.related;
                chapters.value = context.chapters;
                applyUpNextQueue();
                if (context.chapters.length > 1 && options.settings.chaptersToScenes) {
                    options.setSceneMarkers(
                        context.chapters.map((chapter) => ({
                            start: chapter.startSeconds,
                            label: chapter.title,
                        })),
                    );
                }
            } catch {
                // Related/chapters are enrichment — playback works without.
            } finally {
                if (token === contextToken) isLoadingContext.value = false;
            }
        },
        { immediate: true },
    );

    // --- captions --------------------------------------------------------
    const captionTracks = ref<CaptionTrack[]>([]);
    const isLoadingCaptions = ref(false);
    const loadingCaptionCode = ref("");
    const loadedCaptionCodes = ref<string[]>([]);
    /** Backend chatter while a fetch waits out a YouTube rate limit. */
    const captionStatus = ref("");

    void listen<{ videoId: string; language: string; message: string }>(
        "youtube://caption-progress",
        (event) => {
            if (event.payload.videoId !== currentVideoId.value) return;
            captionStatus.value = event.payload.message;
        },
    );

    const loadCaptionTracks = async () => {
        const videoId = currentVideoId.value;
        if (!videoId || captionTracks.value.length || isLoadingCaptions.value) {
            return;
        }
        isLoadingCaptions.value = true;
        try {
            captionTracks.value = await invoke<CaptionTrack[]>(
                "youtube_caption_tracks",
                { videoId },
            );
        } catch {
            captionTracks.value = [];
        } finally {
            isLoadingCaptions.value = false;
        }
    };

    /** Downloads a caption track and hands it to the player. */
    const useCaption = async (track: CaptionTrack) => {
        const videoId = currentVideoId.value;
        if (!videoId || loadingCaptionCode.value) return;
        loadingCaptionCode.value = track.code;
        captionStatus.value = "";
        try {
            const path = await invoke<string>("youtube_caption_file", {
                videoId,
                language: track.code,
            });
            const label = track.auto ? `${track.name} (auto)` : track.name;
            const added = await options.addSubtitleFile(path, label);
            if (added) {
                if (!loadedCaptionCodes.value.includes(track.code)) {
                    loadedCaptionCodes.value = [
                        ...loadedCaptionCodes.value,
                        track.code,
                    ];
                }
                options.notify(`${label} subtitles on`);
            } else {
                options.notify("Couldn't load that caption track");
            }
        } catch (error) {
            options.notify(
                String(error).replace(/^Error:\s*/, "").slice(0, 160),
            );
        } finally {
            loadingCaptionCode.value = "";
            captionStatus.value = "";
        }
    };

    // --- comments --------------------------------------------------------
    const comments = ref<YoutubeComment[]>([]);
    const commentsCursor = ref<string | null>(null);
    const commentsTotal = ref("");
    const commentsCount = ref<number | null>(null);
    const commentSortOptions = ref<CommentSortOption[]>([]);
    const activeCommentSort = ref("");
    const isLoadingComments = ref(false);
    const commentsError = ref("");

    const loadComments = async (more = false, sortToken?: string) => {
        const videoId = currentVideoId.value;
        if (!videoId || isLoadingComments.value) return;
        if (more && !commentsCursor.value) return;
        isLoadingComments.value = true;
        if (!more) commentsError.value = "";
        try {
            const page = await invoke<CommentPage>("youtube_comments", {
                payload: {
                    videoId,
                    cursor: more ? commentsCursor.value : null,
                    sortToken: sortToken ?? null,
                },
            });
            comments.value = more
                ? [...comments.value, ...page.comments]
                : page.comments;
            commentsCursor.value = page.nextCursor;
            if (page.totalText) commentsTotal.value = page.totalText;
            if (page.totalCount !== null) commentsCount.value = page.totalCount;
            // Only the first page carries the menu; a re-sort keeps the
            // options already on screen.
            if (page.sortOptions.length) {
                commentSortOptions.value = page.sortOptions;
                if (!activeCommentSort.value) {
                    activeCommentSort.value =
                        page.sortOptions.find((option) => option.selected)
                            ?.title ?? "";
                }
            }
        } catch (error) {
            commentsError.value = String(error)
                .replace(/^Error:\s*/, "")
                .slice(0, 160);
        } finally {
            isLoadingComments.value = false;
        }
    };

    /** Reloads the list in YouTube's "Top" or "Newest" order. */
    const setCommentSort = async (option: CommentSortOption) => {
        if (isLoadingComments.value || activeCommentSort.value === option.title) {
            return;
        }
        activeCommentSort.value = option.title;
        comments.value = [];
        commentsCursor.value = null;
        selectedCommentIds.value = [];
        await loadComments(false, option.token);
    };

    // --- replies -----------------------------------------------------
    const replyThreads = ref<Record<string, ReplyThread>>({});

    const loadReplies = async (comment: YoutubeComment, more = false) => {
        const existing = replyThreads.value[comment.id];
        const token = more ? existing?.cursor : comment.replyToken;
        if (!token || existing?.loading) return;
        replyThreads.value = {
            ...replyThreads.value,
            [comment.id]: {
                open: true,
                loading: true,
                replies: existing?.replies ?? [],
                cursor: existing?.cursor ?? null,
                error: "",
            },
        };
        try {
            const page = await invoke<CommentPage>("youtube_comment_replies", {
                payload: { token },
            });
            const thread = replyThreads.value[comment.id];
            replyThreads.value = {
                ...replyThreads.value,
                [comment.id]: {
                    open: true,
                    loading: false,
                    replies: more
                        ? [...thread.replies, ...page.comments]
                        : page.comments,
                    cursor: page.nextCursor,
                    error: "",
                },
            };
        } catch (error) {
            const thread = replyThreads.value[comment.id];
            replyThreads.value = {
                ...replyThreads.value,
                [comment.id]: {
                    ...thread,
                    loading: false,
                    error: String(error)
                        .replace(/^Error:\s*/, "")
                        .slice(0, 160),
                },
            };
        }
    };

    /** Opens a reply thread (loading it the first time) or closes it. */
    const toggleReplies = (comment: YoutubeComment) => {
        const thread = replyThreads.value[comment.id];
        if (!thread) {
            void loadReplies(comment);
            return;
        }
        replyThreads.value = {
            ...replyThreads.value,
            [comment.id]: { ...thread, open: !thread.open },
        };
    };

    /** Top-level comments plus every reply currently on screen. */
    const visibleComments = computed<YoutubeComment[]>(() =>
        comments.value.flatMap((comment) => {
            const thread = replyThreads.value[comment.id];
            return thread?.open ? [comment, ...thread.replies] : [comment];
        }),
    );

    // --- search ------------------------------------------------------
    // YouTube has no comment-search endpoint, so reaching comments that
    // were never scrolled to means walking every page locally. The crawl
    // streams batches in so matches appear while the rest still loads.
    const commentQuery = ref("");
    const isCrawling = ref(false);
    const crawlLoaded = ref(0);
    const crawlStopped = ref(false);
    let crawlRunId = 0;

    const matches = (comment: YoutubeComment, needle: string) =>
        comment.text.toLowerCase().includes(needle) ||
        comment.author.toLowerCase().includes(needle) ||
        (comment.translated ?? "").toLowerCase().includes(needle);

    void listen<{ runId: number; comments: YoutubeComment[] }>(
        "youtube://comments-batch",
        (event) => {
            if (event.payload.runId !== crawlRunId) return;
            const seen = new Set(comments.value.map((entry) => entry.id));
            const fresh = event.payload.comments.filter(
                (entry) => !seen.has(entry.id),
            );
            if (fresh.length) comments.value = [...comments.value, ...fresh];
        },
    );

    void listen<{
        runId: number;
        loaded: number;
        done: boolean;
        stopped: boolean;
        error: string | null;
    }>("youtube://comments-progress", (event) => {
        if (event.payload.runId !== crawlRunId) return;
        crawlLoaded.value = event.payload.loaded;
        if (event.payload.done) {
            isCrawling.value = false;
            crawlStopped.value = event.payload.stopped;
            // Everything is in hand, so there is nothing left to page.
            if (!event.payload.stopped) commentsCursor.value = null;
            if (event.payload.error) commentsError.value = event.payload.error;
        }
    });

    /** Walks the rest of the thread so search covers every comment. */
    const searchAllComments = async () => {
        const videoId = currentVideoId.value;
        if (!videoId || isCrawling.value) return;
        isCrawling.value = true;
        crawlStopped.value = false;
        crawlLoaded.value = 0;
        try {
            crawlRunId = await invoke<number>("youtube_comments_fetch_all", {
                payload: {
                    videoId,
                    cursor: commentsCursor.value,
                    includeReplies: true,
                },
            });
        } catch (error) {
            isCrawling.value = false;
            options.notify(
                String(error).replace(/^Error:\s*/, "").slice(0, 160),
            );
        }
    };

    const stopSearchingAll = () => {
        void invoke("youtube_comments_stop").catch(() => {});
        isCrawling.value = false;
        crawlStopped.value = true;
    };

    const setCommentQuery = (value: string) => {
        commentQuery.value = value;
        // A search is only trustworthy once everything has been read.
        if (value.trim() && commentsCursor.value && !isCrawling.value) {
            void searchAllComments();
        }
    };

    /** What the drawer shows: the search result, or everything. */
    const filteredComments = computed<YoutubeComment[]>(() => {
        const needle = commentQuery.value.trim().toLowerCase();
        if (!needle) return comments.value;
        return comments.value.filter((comment) => {
            if (matches(comment, needle)) return true;
            // Keep a parent whose reply matched, so the hit has context.
            const thread = replyThreads.value[comment.id];
            return (thread?.replies ?? []).some((reply) =>
                matches(reply, needle),
            );
        });
    });

    const matchCount = computed(() => {
        const needle = commentQuery.value.trim().toLowerCase();
        if (!needle) return 0;
        return visibleComments.value.filter((comment) =>
            matches(comment, needle),
        ).length;
    });

    /** Pulls the next page when the list is scrolled near its end. */
    const loadMoreCommentsIfNeeded = (element: HTMLElement) => {
        if (!commentsCursor.value || isLoadingComments.value) return;
        const remaining =
            element.scrollHeight - element.scrollTop - element.clientHeight;
        if (remaining < 240) void loadComments(true);
    };

    const isTranslating = ref(false);
    const showTranslated = ref(false);
    const selectedCommentIds = ref<string[]>([]);

    const toggleCommentSelection = (id: string) => {
        selectedCommentIds.value = selectedCommentIds.value.includes(id)
            ? selectedCommentIds.value.filter((entry) => entry !== id)
            : [...selectedCommentIds.value, id];
    };

    const clearCommentSelection = () => {
        selectedCommentIds.value = [];
    };

    /**
     * Translates the loaded comments — or only the ticked ones — and keeps
     * showing translations afterwards.
     */
    const translateComments = async (
        targetLanguage: string,
        credentials: { base: string; key: string; model: string },
        onlySelected = false,
    ) => {
        if (isTranslating.value || !comments.value.length) return;
        // Expanded replies read like comments and translate like them.
        const scope =
            onlySelected && selectedCommentIds.value.length
                ? visibleComments.value.filter((comment) =>
                      selectedCommentIds.value.includes(comment.id),
                  )
                : visibleComments.value;
        // Anything not yet translated, plus anything translated by a
        // different model or into a different language.
        const stamp = `${targetLanguage}|${credentials.model}`;
        const pending = scope.filter(
            (comment) => comment.translatedBy !== stamp,
        );
        if (!pending.length) {
            showTranslated.value = true;
            options.notify(`Already translated to ${targetLanguage}`);
            return;
        }
        isTranslating.value = true;
        try {
            const translations = await invoke<(string | null)[]>(
                "youtube_translate_comments",
                {
                    payload: {
                        texts: pending.map((comment) => comment.text),
                        targetLanguage,
                        chatBase: credentials.base,
                        chatKey: credentials.key,
                        chatModel: credentials.model,
                    },
                },
            );
            let failed = 0;
            pending.forEach((comment, index) => {
                const translated = translations[index];
                // A null means the model failed on this one. Leaving it
                // unstamped is what keeps it retryable — recording the
                // original text as its translation is the bug that made
                // failures look permanent.
                if (translated == null) {
                    failed += 1;
                    return;
                }
                comment.translated = translated;
                comment.translatedBy = stamp;
            });
            showTranslated.value = true;
            const done = pending.length - failed;
            options.notify(
                failed
                    ? `Translated ${done} · ${failed} failed — press Translate again to retry`
                    : `Translated ${done} comment${done === 1 ? "" : "s"}`,
            );
        } catch (error) {
            options.notify(
                String(error).replace(/^Error:\s*/, "").slice(0, 160),
            );
        } finally {
            isTranslating.value = false;
        }
    };

    const hideSponsorToast = () => {
        sponsorToast.value = null;
        if (sponsorToastTimer !== null) {
            window.clearTimeout(sponsorToastTimer);
            sponsorToastTimer = null;
        }
    };

    /** Driven by the playback clock; skips over sponsor segments once each. */
    const onPlaybackTick = (currentTime: number) => {
        if (!sponsorSegments.value.length) return;
        const segment = sponsorSegments.value.find(
            (candidate) =>
                currentTime >= candidate.startSeconds &&
                currentTime < candidate.endSeconds - 0.3 &&
                !suppressedSegments.has(candidate.startSeconds),
        );
        if (!segment) return;
        suppressedSegments.add(segment.startSeconds);
        options.seekTo(segment.endSeconds + 0.05);
        sponsorToast.value = {
            from: segment.startSeconds,
            to: segment.endSeconds,
        };
        if (sponsorToastTimer !== null) window.clearTimeout(sponsorToastTimer);
        sponsorToastTimer = window.setTimeout(() => {
            sponsorToast.value = null;
            sponsorToastTimer = null;
        }, 6000);
    };

    const undoSponsorSkip = () => {
        const toast = sponsorToast.value;
        if (!toast) return;
        hideSponsorToast();
        options.seekTo(toast.from);
    };

    const toggleDrawer = () => {
        isDrawerOpen.value = !isDrawerOpen.value;
        // Both drawers hug the right edge; never show them stacked.
        if (isDrawerOpen.value) options.closeOtherDrawers?.();
    };

    const closeDrawer = (): boolean => {
        if (!isDrawerOpen.value) return false;
        isDrawerOpen.value = false;
        return true;
    };

    return {
        isDrawerOpen,
        activeTab,
        related,
        chapters,
        captionTracks,
        isLoadingCaptions,
        loadingCaptionCode,
        loadedCaptionCodes,
        captionStatus,
        loadCaptionTracks,
        useCaption,
        comments,
        commentsCursor,
        commentsTotal,
        commentsCount,
        commentSortOptions,
        activeCommentSort,
        setCommentSort,
        loadMoreCommentsIfNeeded,
        replyThreads,
        toggleReplies,
        loadReplies,
        visibleComments,
        commentQuery,
        setCommentQuery,
        filteredComments,
        matchCount,
        isCrawling,
        crawlLoaded,
        crawlStopped,
        searchAllComments,
        stopSearchingAll,
        isLoadingComments,
        commentsError,
        loadComments,
        translateComments,
        isTranslating,
        showTranslated,
        selectedCommentIds,
        toggleCommentSelection,
        clearCommentSelection,
        sponsorSegments,
        sponsorToast,
        isLoadingContext,
        autoplayNext,
        isYoutubeWatch,
        setAutoplayNext,
        onPlaybackTick,
        undoSponsorSkip,
        hideSponsorToast,
        toggleDrawer,
        closeDrawer,
    };
};
