export const SOIA_WEBDAV_KEY_PREFIX = "soia-webdav://";
export const SOIA_DLNA_KEY_PREFIX = "soia-dlna://";
export const SOIA_SMB_KEY_PREFIX = "soia-smb://";

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
          parentPath?: string;
      }
    | {
          type: "smb";
          key: string;
          url?: string;
          connectionId?: string;
          filePath?: string;
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
    parentPath?: string;
} | null => {
    if (!key.startsWith(SOIA_DLNA_KEY_PREFIX)) return null;
    const value = key.slice(SOIA_DLNA_KEY_PREFIX.length);
    const slashIndex = value.indexOf("/");
    if (slashIndex <= 0) return null;
    const encodedConnectionId = value.slice(0, slashIndex);
    const encodedParts = value.slice(slashIndex + 1).split("/");
    const encodedResourceUrl = encodedParts[0] ?? "";
    const encodedParentPath = encodedParts[1] ?? "";
    if (!encodedConnectionId || !encodedResourceUrl) return null;
    try {
        const parsed = {
            connectionId: decodeURIComponent(encodedConnectionId),
            resourceUrl: decodeURIComponent(encodedResourceUrl),
        };
        if (!encodedParentPath) return parsed;
        return {
            ...parsed,
            parentPath: decodeURIComponent(encodedParentPath).trim() || "0",
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

const toSmbKeyConnectionId = (connectionId: string): string =>
    connectionId.trim().replace(/^smb-(?=\d+$)/i, "");

const toRuntimeSmbConnectionId = (connectionIdInKey: string): string =>
    /^\d+$/.test(connectionIdInKey)
        ? `smb-${connectionIdInKey}`
        : connectionIdInKey;

export const createWebdavPlaybackKey = (
    connectionId: string,
    filePath: string,
): string =>
    `${SOIA_WEBDAV_KEY_PREFIX}${toWebdavKeyConnectionId(connectionId)}${normalizeFilePath(filePath)}`;

export const createDlnaPlaybackKey = (
    connectionId: string,
    resourceUrl: string,
    parentPath?: string,
): string => {
    const base =
        `${SOIA_DLNA_KEY_PREFIX}${encodeURIComponent(connectionId.trim())}/${encodeURIComponent(resourceUrl.trim())}`;
    const normalizedParent = parentPath?.trim();
    return normalizedParent
        ? `${base}/${encodeURIComponent(normalizedParent)}`
        : base;
};

export const createSmbPlaybackKey = (
    connectionId: string,
    filePath: string,
): string =>
    `${SOIA_SMB_KEY_PREFIX}${encodeURIComponent(toSmbKeyConnectionId(connectionId))}${normalizeFilePath(filePath)}`;

export const createSmbPlaybackUrl = (
    baseUrl: string,
    filePath: string,
): string => {
    const normalizedPath = normalizeFilePath(filePath);
    try {
        const url = new URL(baseUrl.trim());
        if (url.protocol.toLowerCase() !== "smb:") {
            throw new Error("Invalid SMB URL");
        }
        const baseSegments = url.pathname
            .split("/")
            .filter(Boolean)
            .map((segment) => decodeURIComponent(segment));
        const fileSegments =
            normalizedPath === "/"
                ? []
                : normalizedPath
                      .replace(/^\/+/, "")
                      .split("/")
                      .filter(Boolean);
        const encodedPath = [...baseSegments, ...fileSegments]
            .map((segment) => encodeURIComponent(segment))
            .join("/");
        return `smb://${url.host}/${encodedPath}`;
    } catch {
        const base = baseUrl.trim().replace(/\/+$/, "");
        return `${base}${normalizedPath}`;
    }
};

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
            parentPath: soiaDlna.parentPath,
        };
    }

    const soiaSmb = parseWebdavKey(key, SOIA_SMB_KEY_PREFIX);
    if (soiaSmb) {
        return {
            type: "smb",
            key,
            connectionId: toRuntimeSmbConnectionId(
                decodeURIComponent(soiaSmb.connectionId),
            ),
            filePath: soiaSmb.filePath,
        };
    }

    if (/^smb:\/\//i.test(key)) {
        return {
            type: "smb",
            key,
            url: key,
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
    if (source.type === "smb") {
        if (source.url) return source.url;
        const host = source.connectionId?.replace(/^smb-/i, "") || "smb";
        const normalizedPath = source.filePath?.replace(/^\/+/, "") ?? "";
        return normalizedPath ? `smb://${host}/${normalizedPath}` : `smb://${host}`;
    }
    return source.path;
};

export const getPlaybackDisplayPathWithHomePrefix = (key: string): string => {
    const display = getPlaybackDisplayPath(key);
    return display === key ? formatPathWithHomePrefix(display) : display;
};
