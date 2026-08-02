import { computed, ref, watch, type Ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { YoutubeItem } from "./useYouTubeModule";

export type YoutubeChapter = { title: string; startSeconds: number };

type YoutubeVideoContext = {
    related: YoutubeItem[];
    chapters: YoutubeChapter[];
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
    const activeTab: Ref<"upnext" | "chapters"> = ref("upnext");
    const related = ref<YoutubeItem[]>([]);
    const chapters = ref<YoutubeChapter[]>([]);
    const isLoadingContext = ref(false);
    const autoplayNext = ref(
        (typeof localStorage === "undefined"
            ? "1"
            : localStorage.getItem(AUTOPLAY_STORAGE_KEY) ?? "1") !== "0",
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
            if (!videoId) {
                isDrawerOpen.value = false;
                options.setUpNextQueue([]);
                return;
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
                if (context.chapters.length > 1) {
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

    const toggleDrawer = () => {
        isDrawerOpen.value = !isDrawerOpen.value;
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
        isLoadingContext,
        autoplayNext,
        isYoutubeWatch,
        setAutoplayNext,
        toggleDrawer,
        closeDrawer,
    };
};
