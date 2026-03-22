import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { MediaTrack } from "../types/media";

type HistoryApi = {
    getExternalTracks: (path: string) => { audio: string[]; sub: string[] };
    recordExternalTrack: (
        path: string,
        kind: "audio" | "sub",
        trackPath: string,
    ) => Promise<void>;
};

export const useMediaTracks = (
    getCurrentMediaUrl?: () => string,
    history?: HistoryApi,
) => {
    const videoTracks = ref<MediaTrack[]>([]);
    const audioTracks = ref<MediaTrack[]>([]);
    const subTracks = ref<MediaTrack[]>([]);

    const showAudioMenu = ref(false);
    const showSubMenu = ref(false);
    const subtitleExtensions = [
        "srt",
        "ass",
        "ssa",
        "vtt",
        "sub",
        "idx",
        "sup",
        "smi",
        "smil",
        "lrc",
        "ttml",
        "dfxp",
    ];

    const normalizeSelectedPaths = (
        selected: string | string[] | null,
    ): string[] => {
        if (!selected) return [];
        return Array.isArray(selected) ? selected : [selected];
    };

    const getMediaKey = (explicitUrl?: string): string | null => {
        const url = (explicitUrl ?? getCurrentMediaUrl?.() ?? "").trim();
        if (!url) return null;
        return url;
    };

    const recordExternalTrack = async (kind: "audio" | "sub", path: string) => {
        const mediaKey = getMediaKey();
        if (!mediaKey || !history) return;
        await history.recordExternalTrack(mediaKey, kind, path);
    };

    const handleTracksUpdate = (payload: { tracks: MediaTrack[] }) => {
        const all = payload.tracks;
        videoTracks.value = all.filter((t) => t.track_type === "video");
        audioTracks.value = all.filter((t) => t.track_type === "audio");
        const subs = all.filter((t) => t.track_type === "sub");
        subTracks.value = [
            {
                id: 0,
                title: "None (Off)",
                lang: "",
                selected: !subs.some((s) => s.selected),
                track_type: "sub",
            },
            ...subs,
        ];
    };

    const selectAudio = async (track: MediaTrack) => {
        audioTracks.value.forEach((t) => (t.selected = t.id === track.id));
        showAudioMenu.value = false;
        await invoke("mpv_set_option_string", { name: "aid", value: track.id });
    };

    const selectSub = async (track: MediaTrack) => {
        subTracks.value.forEach((t) => (t.selected = t.id === track.id));
        showSubMenu.value = false;
        await invoke("mpv_set_option_string", { name: "sid", value: track.id });
    };

    const addExternalAudioTrack = async () => {
        const selected = await open({
            multiple: false,
            directory: false,
        });
        const [path] = normalizeSelectedPaths(selected);
        if (!path) return;
        await recordExternalTrack("audio", path);
        await invoke("mpv_run_command", { args: ["audio-add", path, "select"] });
    };

    const addExternalSubtitleTrack = async () => {
        const selected = await open({
            multiple: false,
            directory: false,
            filters: [
                {
                    name: "Subtitle Files",
                    extensions: subtitleExtensions,
                },
            ],
        });
        const [path] = normalizeSelectedPaths(selected);
        if (!path) return;
        await recordExternalTrack("sub", path);
        await invoke("mpv_run_command", { args: ["sub-add", path, "select"] });
    };

    const applyExternalTracksForUrl = async (url: string) => {
        const mediaKey = getMediaKey(url);
        if (!mediaKey || !history) return;
        const { audio: audioPaths, sub: subPaths } =
            history.getExternalTracks(mediaKey);
        for (const audioPath of audioPaths) {
            try {
                await invoke("mpv_run_command", {
                    args: ["audio-add", audioPath, "select"],
                });
            } catch {
                // ignore if file is missing or mpv rejects it
            }
        }
        for (const subPath of subPaths) {
            try {
                await invoke("mpv_run_command", {
                    args: ["sub-add", subPath, "select"],
                });
            } catch {
                // ignore if file is missing or mpv rejects it
            }
        }
    };

    const resetTracks = () => {
        videoTracks.value = [];
        audioTracks.value = [];
        subTracks.value = [];
        showAudioMenu.value = false;
        showSubMenu.value = false;
    };

    return {
        videoTracks,
        audioTracks,
        subTracks,
        handleTracksUpdate,
        showAudioMenu,
        showSubMenu,
        selectAudio,
        selectSub,
        addExternalAudioTrack,
        addExternalSubtitleTrack,
        applyExternalTracksForUrl,
        resetTracks,
    };
};
