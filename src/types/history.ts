export type HistoryEntry = {
    path: string;
    title?: string;
    lastPosition: number;
    duration: number;
    lastPlayedAt: number;
    isPinned: boolean;
    externalAudioTracks?: string[];
    externalSubTracks?: string[];
};
