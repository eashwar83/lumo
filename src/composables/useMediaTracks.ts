import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { MediaTrack } from "../types/media";
import { normalizeLocalPathForCompare } from "../utils/localMediaSiblings";
import { useSubtitleState, type SubtitleTarget } from "./useSubtitleState";

type HistoryApi = {
    getExternalTracks: (path: string) => { audio: string[]; sub: string[] };
    recordExternalTrack: (
        path: string,
        kind: "audio" | "sub",
        trackPath: string,
    ) => Promise<void>;
};

type ExternalSubtitleMatch = {
    path: string;
};

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

export const useMediaTracks = (
    getCurrentMediaUrl?: () => string,
    history?: HistoryApi,
) => {
    const videoTracks = ref<MediaTrack[]>([]);
    const audioTracks = ref<MediaTrack[]>([]);
    const subTracks = ref<MediaTrack[]>([]);

    const showAudioMenu = ref(false);
    const showSubMenu = ref(false);
    const subtitleState = useSubtitleState(subTracks, showSubMenu);

    const normalizeSelectedPaths = (
        selected: string | string[] | null,
    ): string[] => {
        if (!selected) return [];
        return Array.isArray(selected) ? selected : [selected];
    };
    const isSameTrackId = (left: MediaTrack["id"], right: MediaTrack["id"]) =>
        String(left) === String(right);

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

    const getAutoSubtitlePaths = async (
        mediaKey: string,
        mediaTitle?: string,
    ): Promise<string[]> => {
        const matches = await invoke<ExternalSubtitleMatch[]>(
            "find_fuzzy_external_subtitle_matches",
            {
                payload: {
                    playbackKey: mediaKey,
                    mediaTitle,
                },
            },
        ).catch(() => []);
        return matches.map((match) => match.path).filter(Boolean);
    };

    const addExternalSubPath = async (
        path: string,
        mode: "select" | "auto",
        mediaKey?: string,
    ): Promise<void> => {
        if (mediaKey && getMediaKey() !== mediaKey) return;
        try {
            await invoke("mpv_run_command", {
                args: ["sub-add", path, mode],
            });
        } catch {
            // ignore if file is missing or mpv rejects it
        }
    };

    const handleTracksUpdate = (payload: { tracks: MediaTrack[] }) => {
        const all = payload.tracks;
        videoTracks.value = all.filter((t) => t.track_type === "video");
        audioTracks.value = all.filter((t) => t.track_type === "audio");
        const subs = all.filter((t) => t.track_type === "sub");
        const previousPrimaryId = subtitleState.primarySubId.value;
        const selectedIds = subs
            .filter((track) => track.selected)
            .map((track) => track.id);
        const hasSelected = (id: MediaTrack["id"]) =>
            selectedIds.some((selectedId) => isSameTrackId(selectedId, id));
        const normalizedPrimaryId = (() => {
            if (!selectedIds.length) return 0;
            if (selectedIds.length === 1) return selectedIds[0];
            if (previousPrimaryId !== 0 && hasSelected(previousPrimaryId)) {
                return previousPrimaryId;
            }
            const nonSecondary = selectedIds.find(
                (id) => !isSameTrackId(id, subtitleState.secondarySubId.value),
            );
            return nonSecondary ?? selectedIds[0];
        })();
        const normalizedSubs = subs.map((track) => ({
            ...track,
            selected:
                normalizedPrimaryId !== 0 &&
                isSameTrackId(track.id, normalizedPrimaryId),
        }));
        void subtitleState.reconcileSubtitleState(normalizedSubs);
        subTracks.value = [
            {
                id: 0,
                title: "None (Off)",
                lang: "",
                selected: normalizedPrimaryId === 0,
                track_type: "sub",
            },
            ...normalizedSubs,
        ];
    };

    const selectAudio = async (track: MediaTrack) => {
        audioTracks.value.forEach((t) => (t.selected = t.id === track.id));
        showAudioMenu.value = false;
        await invoke("mpv_set_option_string", { name: "aid", value: track.id });
    };

    const selectSubTrack = async (payload: {
        target: SubtitleTarget;
        track: MediaTrack;
    }) => {
        await subtitleState.selectSubTrack(payload);
    };

    const setDualSubEnabled = async (enabled: boolean) => {
        await subtitleState.setDualSubEnabled(enabled);
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

    const applyExternalTracksForUrl = async (url: string, mediaTitle?: string) => {
        const mediaKey = getMediaKey(url);
        if (!mediaKey) return;
        const { audio: audioPaths, sub: subPaths } = history
            ? history.getExternalTracks(mediaKey)
            : { audio: [], sub: [] };
        const rememberedSubPaths = new Set(subPaths.map(normalizeLocalPathForCompare));
        if (getMediaKey() !== mediaKey) return;
        for (const audioPath of audioPaths) {
            if (getMediaKey() !== mediaKey) return;
            try {
                await invoke("mpv_run_command", {
                    args: ["audio-add", audioPath, "select"],
                });
            } catch {
                // ignore if file is missing or mpv rejects it
            }
        }
        const autoSubPaths = (await getAutoSubtitlePaths(mediaKey, mediaTitle)).filter(
            (path) => !rememberedSubPaths.has(normalizeLocalPathForCompare(path)),
        );
        if (getMediaKey() !== mediaKey) return;
        for (const [index, subPath] of autoSubPaths.entries()) {
            await addExternalSubPath(
                subPath,
                index === 0 ? "select" : "auto",
                mediaKey,
            );
        }
        for (const subPath of subPaths) {
            await addExternalSubPath(subPath, "select", mediaKey);
        }
    };

    const resetTracks = () => {
        videoTracks.value = [];
        audioTracks.value = [];
        subTracks.value = [];
        showAudioMenu.value = false;
        showSubMenu.value = false;
        subtitleState.resetSubtitleState();
    };

    const setSubtitlesDisabled = async (disabled: boolean) => {
        if (disabled) {
            await subtitleState.disableAllSubtitles();
            subTracks.value.forEach((track) => {
                track.selected = String(track.id) === "0";
            });
            return;
        }
        await subtitleState.enableAutoSubtitleSelection();
    };

    return {
        videoTracks,
        audioTracks,
        subTracks,
        handleTracksUpdate,
        showAudioMenu,
        showSubMenu,
        dualSubEnabled: subtitleState.dualSubEnabled,
        secondarySubId: subtitleState.secondarySubId,
        activeSubTarget: subtitleState.activeSubTarget,
        selectAudio,
        selectSubTrack,
        setDualSubEnabled,
        setActiveSubTarget: subtitleState.setActiveSubTarget,
        addExternalAudioTrack,
        addExternalSubtitleTrack,
        applyExternalTracksForUrl,
        setSubtitlesDisabled,
        resetTracks,
    };
};
