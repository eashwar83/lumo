import { computed, onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type DownloadItem = {
    id: string;
    url: string;
    title: string;
    status:
        | "queued"
        | "downloading"
        | "paused"
        | "failed"
        | "done"
        | "cancelled";
    progressPercent: number;
    speedBps: number;
    etaSeconds: number;
    destDir: string;
    filePath?: string | null;
    error?: string | null;
    retries: number;
    addedAt: number;
    /** Non-fatal note from the subtitle pass. */
    subtitleNote?: string | null;
};

export type DownloadOptions = {
    qualityMaxHeight: number | null;
    container: string;
    audioOnly: boolean;
    audioFormat: string;
    embedSubs: boolean;
    subLangs: string;
    embedThumbnail: boolean;
    embedChapters: boolean;
    front: boolean;
};

// Shared across the panel and the player controls: one queue view, one
// event listener, regardless of how many components ask for it.
const items = ref<DownloadItem[]>([]);
let unlisten: UnlistenFn | null = null;
let subscribers = 0;

export const useYouTubeDownloads = () => {

    const activeCount = computed(
        () =>
            items.value.filter(
                (item) =>
                    item.status === "downloading" ||
                    item.status === "queued" ||
                    item.status === "failed",
            ).length,
    );

    const refresh = async () => {
        try {
            items.value = await invoke<DownloadItem[]>("youtube_download_list");
        } catch {
            // Backend unavailable (non-Tauri dev preview).
        }
    };

    onMounted(async () => {
        subscribers += 1;
        await refresh();
        if (unlisten) return;
        try {
            unlisten = await listen<DownloadItem>(
                "youtube_download_update",
                (event) => {
                    const incoming = event.payload;
                    const index = items.value.findIndex(
                        (item) => item.id === incoming.id,
                    );
                    if (index >= 0) {
                        items.value[index] = incoming;
                    } else {
                        items.value = [...items.value, incoming];
                    }
                },
            );
        } catch {
            // Ignore in non-Tauri environments.
        }
    });

    onUnmounted(() => {
        subscribers = Math.max(0, subscribers - 1);
        if (subscribers === 0) {
            unlisten?.();
            unlisten = null;
        }
    });

    const add = async (
        video: { url: string; title: string },
        options: DownloadOptions,
    ) => {
        items.value = await invoke<DownloadItem[]>("youtube_download_add", {
            payload: { url: video.url, title: video.title, ...options },
        });
    };

    const pause = (id: string) => invoke("youtube_download_pause", { id });
    const resume = (id: string) => invoke("youtube_download_resume", { id });
    const cancel = (id: string) => invoke("youtube_download_cancel", { id });
    const remove = async (id: string) => {
        items.value = await invoke<DownloadItem[]>("youtube_download_remove", {
            id,
        });
    };
    const clearDone = async () => {
        items.value = await invoke<DownloadItem[]>(
            "youtube_download_clear_done",
        );
    };
    const openFolder = () => invoke("youtube_download_open_folder");

    return {
        items,
        activeCount,
        refresh,
        add,
        pause,
        resume,
        cancel,
        remove,
        clearDone,
        openFolder,
    };
};

export const formatSpeed = (bytesPerSecond: number) => {
    if (!bytesPerSecond || bytesPerSecond <= 0) return "";
    const units = ["B/s", "KB/s", "MB/s", "GB/s"];
    let value = bytesPerSecond;
    let unit = 0;
    while (value >= 1024 && unit < units.length - 1) {
        value /= 1024;
        unit += 1;
    }
    return `${value.toFixed(value >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
};

export const formatEta = (seconds: number) => {
    if (!seconds || seconds <= 0) return "";
    const total = Math.round(seconds);
    if (total < 60) return `${total}s left`;
    const minutes = Math.floor(total / 60);
    if (minutes < 60) return `${minutes}m left`;
    return `${Math.floor(minutes / 60)}h ${minutes % 60}m left`;
};
