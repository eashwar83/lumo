import { ref } from "vue";
import type { HistoryEntry } from "../types/history";
import { getPathDisplayName } from "../utils/getPathDisplayName";
import {
    getPlaybackDisplayPath,
    parsePlaybackSource,
} from "../utils/playbackSource";

type ProtocolBadge = {
    id: string;
    label: string;
};

const PROTOCOL_BADGE_LIST: ProtocolBadge[] = [
    { id: "local", label: "Local" },
    { id: "webdav", label: "WebDAV" },
    { id: "smb", label: "SMB" },
    { id: "ftp", label: "FTP" },
    { id: "ftps", label: "FTPS" },
    { id: "http", label: "HTTP" },
    { id: "https", label: "HTTPS" },
];

const getProtocolBadge = (id: string): ProtocolBadge => {
    const badge = PROTOCOL_BADGE_LIST.find((item) => item.id === id);
    return badge ?? PROTOCOL_BADGE_LIST[0];
};

export const useHistoryPanel = () => {
    const expandedPath = ref<string | null>(null);

    const getDisplayName = (entry: HistoryEntry) => {
        const title = typeof entry.title === "string" ? entry.title.trim() : "";
        if (title) return title;
        const source = parsePlaybackSource(entry.path);
        if (source.type === "webdav") {
            return getPathDisplayName(source.filePath, source.filePath);
        }
        return getPathDisplayName(source.path, source.path);
    };

    const getDisplayPath = (path: string) => {
        return getPlaybackDisplayPath(path);
    };

    const getProtocolBadges = (path: string): ProtocolBadge[] => {
        const source = parsePlaybackSource(path);
        if (source.type === "webdav") {
            return [getProtocolBadge("webdav")];
        }

        const normalized = source.path.trim().toLowerCase();
        if (normalized.startsWith("smb://")) {
            return [getProtocolBadge("smb")];
        }
        if (normalized.startsWith("ftps://")) {
            return [getProtocolBadge("ftps")];
        }
        if (normalized.startsWith("ftp://")) {
            return [getProtocolBadge("ftp")];
        }
        if (normalized.startsWith("https://")) {
            return [getProtocolBadge("https")];
        }
        if (normalized.startsWith("http://")) {
            return [getProtocolBadge("http")];
        }

        return [getProtocolBadge("local")];
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
