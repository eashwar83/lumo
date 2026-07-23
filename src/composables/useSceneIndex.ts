import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

// Scene navigation.
//
// If the file carries real chapters, those are authoritative and free — mpv
// hands them over with no analysis. Only when there are none (which is the case
// for essentially every old film) do we fall back to detecting cuts, which
// costs a background pass over the file and is cached from then on.

export type SceneMarker = {
    start: number;
    label: string;
};

type UseSceneIndexOptions = {
    getPath: () => string;
    getDuration: () => number;
    seekTo: (seconds: number) => void | Promise<void>;
    formatTime: (seconds: number) => string;
    onMessage?: (text: string) => void;
};

type MpvChapter = { title?: string | null; time?: number | null };

const isLocalPath = (path: string): boolean =>
    !!path && !/^(https?|rtsp|rtmp|smb|webdav):\/\//i.test(path);

export const useSceneIndex = ({
    getPath,
    getDuration,
    seekTo,
    formatTime,
    onMessage,
}: UseSceneIndexOptions) => {
    const markers = ref<SceneMarker[]>([]);
    /** True when the markers came from the container rather than detection. */
    const fromChapters = ref(false);
    const isScanning = ref(false);

    const hasMarkers = computed(() => markers.value.length > 1);

    const readChapters = async (): Promise<SceneMarker[]> => {
        try {
            const raw = await invoke<string | null>("mpv_get_property_string", {
                name: "chapter-list",
            });
            if (!raw) return [];
            const list = JSON.parse(raw) as MpvChapter[];
            if (!Array.isArray(list)) return [];
            return list
                .map((chapter, index) => ({
                    start: Number(chapter.time ?? 0),
                    label: chapter.title?.trim() || `Chapter ${index + 1}`,
                }))
                .filter((chapter) => Number.isFinite(chapter.start));
        } catch {
            return [];
        }
    };

    const applyDetected = (starts: number[]) => {
        markers.value = starts.map((start, index) => ({
            start,
            label: `Scene ${index + 1} · ${formatTime(start)}`,
        }));
        fromChapters.value = false;
    };

    /** Load chapters if present; otherwise use a cached scene index if there is one. */
    const load = async (allowScan = false) => {
        markers.value = [];
        fromChapters.value = false;
        const path = getPath().trim();
        if (!path) return;

        const chapters = await readChapters();
        if (chapters.length > 1) {
            markers.value = chapters;
            fromChapters.value = true;
            return;
        }
        if (!isLocalPath(path)) return;

        // Without `refresh` this returns instantly from cache, or performs the
        // scan — so only ask when the caller is prepared to wait.
        if (!allowScan) return;
        await scan(false);
    };

    const scan = async (refresh: boolean) => {
        const path = getPath().trim();
        const duration = getDuration();
        if (!path || !isLocalPath(path) || duration <= 0 || isScanning.value) return;
        isScanning.value = true;
        if (refresh) onMessage?.("Scanning for scenes…");
        try {
            const starts = await invoke<number[]>("get_scene_index", {
                path,
                duration,
                refresh,
            });
            applyDetected(starts);
            if (refresh) {
                onMessage?.(
                    starts.length > 1
                        ? `Found ${starts.length} scenes`
                        : "No scene changes detected",
                );
            }
        } catch (error) {
            console.warn("[scenes] detection failed", error);
            if (refresh) onMessage?.("Scene detection failed");
        } finally {
            isScanning.value = false;
        }
    };

    const indexAt = (position: number): number => {
        let index = -1;
        markers.value.forEach((marker, i) => {
            if (marker.start <= position + 0.25) index = i;
        });
        return index;
    };

    const jumpTo = async (marker: SceneMarker) => {
        await seekTo(marker.start);
    };

    const next = async (position: number) => {
        if (!hasMarkers.value) return;
        const target = markers.value.find((marker) => marker.start > position + 0.5);
        if (target) await seekTo(target.start);
    };

    const previous = async (position: number) => {
        if (!hasMarkers.value) return;
        // Match the usual "restart this one first" behaviour of chapter-back.
        const current = indexAt(position);
        if (current < 0) return;
        const marker = markers.value[current];
        const target =
            position - marker.start > 3 ? marker : markers.value[Math.max(0, current - 1)];
        await seekTo(target.start);
    };

    const onFileLoaded = () => {
        void load(false);
    };

    return {
        markers,
        fromChapters,
        isScanning,
        hasMarkers,
        load,
        scan,
        jumpTo,
        next,
        previous,
        onFileLoaded,
    };
};

export type SceneIndexController = ReturnType<typeof useSceneIndex>;
