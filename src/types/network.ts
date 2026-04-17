export type NetworkConnectionStatus = "Idle" | "Connected" | "Error";

export type NetworkConnection = {
    id: string;
    label: string;
    protocol: string;
    baseUrl: string;
    username: string;
    password: string;
    defaultPath: string;
};

export type NetworkBrowseEntry = {
    name: string;
    path: string;
    entryType: "dir" | "file";
    size: number | null;
    modifiedAt: string | null;
};

export type NetworkBrowseResult = {
    path: string;
    entries: NetworkBrowseEntry[];
};

export type NetworkFileRow = {
    name: string;
    path: string;
    type: "DIR" | "FILE";
    size: string;
    modified: string;
    isParent?: boolean;
};

export type NetworkPlayRequest = {
    protocol: string;
    connectionId: string;
    filePath: string;
    playbackKey: string;
    displayName: string;
};
