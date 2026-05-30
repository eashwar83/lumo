export type HistoryEntry = {
    id?: string;
    path: string;
    title?: string;
    lastPosition: number;
    duration: number;
    lastPlayedAt: number;
    isPinned: boolean;
    isLivePlayback: boolean;
    externalAudioTracks?: string[];
    externalSubTracks?: string[];
};
