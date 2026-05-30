import { ref } from "vue";
import type { HistoryEntry } from "../types/history";
import { getPathDisplayName } from "../utils/getPathDisplayName";
import {
    getPlaybackDisplayNamePath,
    getPlaybackDisplayPath,
    getPlaybackProtocolId,
} from "../utils/playbackDisplay";

type ProtocolBadge = {
    id: string;
    label: string;
    showsProgress?: boolean;
};

const PROTOCOL_BADGE_LIST: ProtocolBadge[] = [
    { id: "live", label: "LIVE" },
    { id: "local", label: "Local" },
    { id: "webdav", label: "WebDAV" },
    { id: "dlna", label: "DLNA" },
    { id: "smb", label: "SMB" },
    { id: "ftp", label: "FTP" },
    { id: "ftps", label: "FTPS" },
    { id: "http", label: "HTTP" },
    { id: "https", label: "HTTPS" },
];

const getProtocolBadge = (id: string): ProtocolBadge => {
    const badge = PROTOCOL_BADGE_LIST.find((item) => item.id === id);
    const resolved =
        badge ??
        PROTOCOL_BADGE_LIST.find((item) => item.id === "local") ??
        PROTOCOL_BADGE_LIST[0];
    return {
        ...resolved,
        showsProgress: resolved.id !== "live",
    };
};

export const useHistoryPanel = () => {
    const expandedPath = ref<string | null>(null);

    const getDisplayName = (entry: HistoryEntry) => {
        const title = typeof entry.title === "string" ? entry.title.trim() : "";
        if (title) return title;
        const displayNamePath = getPlaybackDisplayNamePath(entry.path);
        return getPathDisplayName(displayNamePath, displayNamePath);
    };

    const getDisplayPath = (path: string) => {
        return getPlaybackDisplayPath(path);
    };

    const getProtocolBadges = (entry: HistoryEntry): ProtocolBadge[] => {
        if (entry.isLivePlayback) return [getProtocolBadge("live")];
        return [getProtocolBadge(getPlaybackProtocolId(entry.path))];
    };

    const middleEllipsis = (value: string, maxLength = 68) => {
        if (value.length <= maxLength) return value;
        const keep = Math.max(6, Math.floor((maxLength - 1) / 2));
        return `${value.slice(0, keep)}…${value.slice(-keep)}`;
    };

    const getPlaybackProgressPercent = (entry: HistoryEntry): number => {
        if (!Number.isFinite(entry.duration) || entry.duration <= 0) return 0;
        if (!Number.isFinite(entry.lastPosition) || entry.lastPosition <= 0) {
            return 0;
        }
        const percent = (entry.lastPosition / entry.duration) * 100;
        return Math.max(0, Math.min(100, percent));
    };

    const getPlaybackProgressLabel = (entry: HistoryEntry): string => {
        const rawPercent = getPlaybackProgressPercent(entry);
        return `${rawPercent.toFixed(1)}%`;
    };

    const toggleExpanded = (path: string) => {
        expandedPath.value = expandedPath.value === path ? null : path;
    };

    const clearExpandedIfMatches = (path: string) => {
        if (expandedPath.value === path) {
            expandedPath.value = null;
        }
    };

    return {
        expandedPath,
        getDisplayName,
        getDisplayPath,
        getProtocolBadges,
        getPlaybackProgressPercent,
        getPlaybackProgressLabel,
        middleEllipsis,
        toggleExpanded,
        clearExpandedIfMatches,
    };
};
