export const SOIA_WEBDAV_KEY_PREFIX = "soia-webdav://";
export const SOIA_DLNA_KEY_PREFIX = "soia-dlna://";

export type PlaybackSource =
    | {
          type: "local";
          key: string;
          path: string;
      }
    | {
          type: "webdav";
          key: string;
          connectionId: string;
          filePath: string;
      }
    | {
          type: "dlna";
          key: string;
          connectionId: string;
          resourceUrl: string;
      };

const normalizeFilePath = (path: string) => {
    const trimmed = path.trim();
    if (!trimmed || trimmed === "/") return "/";
    const withLeading = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
    return withLeading.replace(/\/+$/, "");
};

const parseWebdavKey = (
    key: string,
    prefix: string,
): {
    connectionId: string;
    filePath: string;
} | null => {
    if (!key.startsWith(prefix)) return null;
    const value = key.slice(prefix.length);
    const slashIndex = value.indexOf("/");
    if (slashIndex <= 0) return null;
    const connectionId = value.slice(0, slashIndex);
    const filePath = normalizeFilePath(value.slice(slashIndex));
    if (!connectionId || !filePath) return null;
    return { connectionId, filePath };
};

const parseDlnaKey = (
    key: string,
): {
    connectionId: string;
    resourceUrl: string;
} | null => {
    if (!key.startsWith(SOIA_DLNA_KEY_PREFIX)) return null;
    const value = key.slice(SOIA_DLNA_KEY_PREFIX.length);
    const slashIndex = value.indexOf("/");
    if (slashIndex <= 0) return null;
    const encodedConnectionId = value.slice(0, slashIndex);
    const encodedResourceUrl = value.slice(slashIndex + 1);
    if (!encodedConnectionId || !encodedResourceUrl) return null;
    try {
        return {
            connectionId: decodeURIComponent(encodedConnectionId),
            resourceUrl: decodeURIComponent(encodedResourceUrl),
        };
    } catch {
        return null;
    }
};

const toWebdavKeyConnectionId = (connectionId: string): string =>
    connectionId.trim().replace(/^webdav-(?=\d+$)/i, "");

const toRuntimeWebdavConnectionId = (connectionIdInKey: string): string =>
    /^\d+$/.test(connectionIdInKey)
        ? `webdav-${connectionIdInKey}`
        : connectionIdInKey;

export const createWebdavPlaybackKey = (
    connectionId: string,
    filePath: string,
): string =>
    `${SOIA_WEBDAV_KEY_PREFIX}${toWebdavKeyConnectionId(connectionId)}${normalizeFilePath(filePath)}`;

export const createDlnaPlaybackKey = (
    connectionId: string,
    resourceUrl: string,
): string =>
    `${SOIA_DLNA_KEY_PREFIX}${encodeURIComponent(connectionId.trim())}/${encodeURIComponent(resourceUrl.trim())}`;

export const parsePlaybackSource = (key: string): PlaybackSource => {
    const soiaWebdav = parseWebdavKey(key, SOIA_WEBDAV_KEY_PREFIX);
    if (soiaWebdav) {
        return {
            type: "webdav",
            key,
            connectionId: toRuntimeWebdavConnectionId(soiaWebdav.connectionId),
            filePath: soiaWebdav.filePath,
        };
    }

    const soiaDlna = parseDlnaKey(key);
    if (soiaDlna) {
        return {
            type: "dlna",
            key,
            connectionId: soiaDlna.connectionId,
            resourceUrl: soiaDlna.resourceUrl,
        };
    }

    return {
        type: "local",
        key,
        path: key,
    };
};

export const normalizePlaybackKey = (key: string): string =>
    parsePlaybackSource(key).key;

const formatWebdavDisplayHost = (connectionId: string): string => {
    const trimmed = connectionId.trim();
    if (!trimmed) return "webdav";
    const stripped = trimmed.replace(/^webdav-/i, "");
    return stripped || trimmed;
};

export const formatPathWithHomePrefix = (path: string): string => {
    const trimmed = path.trim();
    if (!trimmed) return path;
    const match = trimmed.match(/^\/(?:Users|home)\/[^/]+(\/.*)?$/);
    if (!match) return path;
    return `~${match[1] ?? ""}`;
};

export const getPlaybackDisplayPath = (key: string): string => {
    const source = parsePlaybackSource(key);
    if (source.type === "webdav") {
        const host = formatWebdavDisplayHost(source.connectionId);
        const normalizedPath = source.filePath.replace(/^\/+/, "");
        return normalizedPath
            ? `webdav://${host}/${normalizedPath}`
            : `webdav://${host}`;
    }
    if (source.type === "dlna") {
        return source.resourceUrl;
    }
    return source.path;
};

export const getPlaybackDisplayPathWithHomePrefix = (key: string): string => {
    const display = getPlaybackDisplayPath(key);
    return display === key ? formatPathWithHomePrefix(display) : display;
};
