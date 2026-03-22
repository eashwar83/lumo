export type NetworkConnection = {
    id: string;
    label: string;
    status: string;
};

export type NetworkFile = {
    name: string;
    type: "DIR" | "FILE";
    size: string;
    modified: string;
    isParent?: boolean;
};

export const networkConnections: NetworkConnection[] = [
    { id: "smb-office", label: "SMB", status: "Connected" },
    { id: "webdav-home", label: "WebDAV", status: "Idle" },
    { id: "ftp-lab", label: "FTP", status: "Idle" },
];

export const defaultNetworkConnectionId = networkConnections[0]?.id ?? "smb-office";
export const defaultNetworkPath = "/Media/Movies/2024";

export const networkFiles: NetworkFile[] = [
    { name: "Films", type: "DIR", size: "—", modified: "2025-12-22 10:14" },
    { name: "Series", type: "DIR", size: "—", modified: "2025-12-18 21:02" },
    {
        name: "Demo_Reel_1080p.mp4",
        type: "FILE",
        size: "1.2 GB",
        modified: "2025-12-20 09:44",
    },
    {
        name: "Nature_4K_HDR.mkv",
        type: "FILE",
        size: "6.4 GB",
        modified: "2025-12-19 14:03",
    },
    {
        name: "Trailer_ProRes.mov",
        type: "FILE",
        size: "2.1 GB",
        modified: "2025-12-17 16:30",
    },
];

export const networkFilesByPath: Record<string, NetworkFile[]> = {
    [defaultNetworkPath]: networkFiles,
    [`${defaultNetworkPath}/Films`]: [
        {
            name: "Festival",
            type: "DIR",
            size: "—",
            modified: "2025-12-11 09:12",
        },
        {
            name: "Classic_Collection.mkv",
            type: "FILE",
            size: "14.2 GB",
            modified: "2025-12-10 22:04",
        },
    ],
    [`${defaultNetworkPath}/Series`]: [
        {
            name: "Season_01",
            type: "DIR",
            size: "—",
            modified: "2025-12-09 08:20",
        },
        {
            name: "Episode_01.mp4",
            type: "FILE",
            size: "701 MB",
            modified: "2025-12-07 18:53",
        },
        {
            name: "Episode_02.mp4",
            type: "FILE",
            size: "702 MB",
            modified: "2025-12-06 19:54",
        },
        {
            name: "Episode_03.mp4",
            type: "FILE",
            size: "703 MB",
            modified: "2025-12-05 17:55",
        },
        {
            name: "Episode_04.mp4",
            type: "FILE",
            size: "704 MB",
            modified: "2025-12-04 18:56",
        },
        {
            name: "Episode_05.mp4",
            type: "FILE",
            size: "705 MB",
            modified: "2025-12-08 19:57",
        },
        {
            name: "Episode_06.mp4",
            type: "FILE",
            size: "706 MB",
            modified: "2025-12-07 17:58",
        },
        {
            name: "Episode_07.mp4",
            type: "FILE",
            size: "707 MB",
            modified: "2025-12-06 18:52",
        },
        {
            name: "Episode_08.mp4",
            type: "FILE",
            size: "708 MB",
            modified: "2025-12-05 19:53",
        },
        {
            name: "Episode_09.mp4",
            type: "FILE",
            size: "709 MB",
            modified: "2025-12-04 17:54",
        },
        {
            name: "Episode_10.mp4",
            type: "FILE",
            size: "710 MB",
            modified: "2025-12-08 18:55",
        },
        {
            name: "Episode_11.mp4",
            type: "FILE",
            size: "711 MB",
            modified: "2025-12-07 19:56",
        },
        {
            name: "Episode_12.mp4",
            type: "FILE",
            size: "712 MB",
            modified: "2025-12-06 17:57",
        },
        {
            name: "Episode_13.mp4",
            type: "FILE",
            size: "713 MB",
            modified: "2025-12-05 18:58",
        },
        {
            name: "Episode_14.mp4",
            type: "FILE",
            size: "714 MB",
            modified: "2025-12-04 19:52",
        },
        {
            name: "Episode_15.mp4",
            type: "FILE",
            size: "715 MB",
            modified: "2025-12-08 17:53",
        },
        {
            name: "Episode_16.mp4",
            type: "FILE",
            size: "716 MB",
            modified: "2025-12-07 18:54",
        },
        {
            name: "Episode_17.mp4",
            type: "FILE",
            size: "717 MB",
            modified: "2025-12-06 19:55",
        },
        {
            name: "Episode_18.mp4",
            type: "FILE",
            size: "718 MB",
            modified: "2025-12-05 17:56",
        },
        {
            name: "Episode_19.mp4",
            type: "FILE",
            size: "719 MB",
            modified: "2025-12-04 18:57",
        },
        {
            name: "Episode_20.mp4",
            type: "FILE",
            size: "720 MB",
            modified: "2025-12-08 19:58",
        },
        {
            name: "Episode_21.mp4",
            type: "FILE",
            size: "721 MB",
            modified: "2025-12-07 17:52",
        },
        {
            name: "Episode_22.mp4",
            type: "FILE",
            size: "722 MB",
            modified: "2025-12-06 18:53",
        },
        {
            name: "Episode_23.mp4",
            type: "FILE",
            size: "723 MB",
            modified: "2025-12-05 19:54",
        },
        {
            name: "Episode_24.mp4",
            type: "FILE",
            size: "724 MB",
            modified: "2025-12-04 17:55",
        },
        {
            name: "Episode_25.mp4",
            type: "FILE",
            size: "725 MB",
            modified: "2025-12-08 18:56",
        },
        {
            name: "Episode_26.mp4",
            type: "FILE",
            size: "726 MB",
            modified: "2025-12-07 19:57",
        },
        {
            name: "Episode_27.mp4",
            type: "FILE",
            size: "727 MB",
            modified: "2025-12-06 17:58",
        },
        {
            name: "Episode_28.mp4",
            type: "FILE",
            size: "728 MB",
            modified: "2025-12-05 18:52",
        },
        {
            name: "Episode_29.mp4",
            type: "FILE",
            size: "729 MB",
            modified: "2025-12-04 19:53",
        },
        {
            name: "Episode_30.mp4",
            type: "FILE",
            size: "730 MB",
            modified: "2025-12-08 17:54",
        },
        {
            name: "Episode_31.mp4",
            type: "FILE",
            size: "731 MB",
            modified: "2025-12-07 18:55",
        },
        {
            name: "Episode_32.mp4",
            type: "FILE",
            size: "732 MB",
            modified: "2025-12-06 19:56",
        },
        {
            name: "Episode_33.mp4",
            type: "FILE",
            size: "733 MB",
            modified: "2025-12-05 17:57",
        },
        {
            name: "Episode_34.mp4",
            type: "FILE",
            size: "734 MB",
            modified: "2025-12-04 18:58",
        },
        {
            name: "Episode_35.mp4",
            type: "FILE",
            size: "735 MB",
            modified: "2025-12-08 19:52",
        },
        {
            name: "Episode_36.mp4",
            type: "FILE",
            size: "736 MB",
            modified: "2025-12-07 17:53",
        },
        {
            name: "Episode_37.mp4",
            type: "FILE",
            size: "737 MB",
            modified: "2025-12-06 18:54",
        },
        {
            name: "Episode_38.mp4",
            type: "FILE",
            size: "738 MB",
            modified: "2025-12-05 19:55",
        },
        {
            name: "Episode_39.mp4",
            type: "FILE",
            size: "739 MB",
            modified: "2025-12-04 17:56",
        },
        {
            name: "Episode_40.mp4",
            type: "FILE",
            size: "740 MB",
            modified: "2025-12-08 18:57",
        },
        {
            name: "Episode_41.mp4",
            type: "FILE",
            size: "741 MB",
            modified: "2025-12-07 19:58",
        },
        {
            name: "Episode_42.mp4",
            type: "FILE",
            size: "742 MB",
            modified: "2025-12-06 17:52",
        },
        {
            name: "Episode_43.mp4",
            type: "FILE",
            size: "743 MB",
            modified: "2025-12-05 18:53",
        },
        {
            name: "Episode_44.mp4",
            type: "FILE",
            size: "744 MB",
            modified: "2025-12-04 19:54",
        },
        {
            name: "Episode_45.mp4",
            type: "FILE",
            size: "745 MB",
            modified: "2025-12-08 17:55",
        },
        {
            name: "Episode_46.mp4",
            type: "FILE",
            size: "746 MB",
            modified: "2025-12-07 18:56",
        },
        {
            name: "Episode_47.mp4",
            type: "FILE",
            size: "747 MB",
            modified: "2025-12-06 19:57",
        },
        {
            name: "Episode_48.mp4",
            type: "FILE",
            size: "748 MB",
            modified: "2025-12-05 17:58",
        },
        {
            name: "Episode_49.mp4",
            type: "FILE",
            size: "749 MB",
            modified: "2025-12-04 18:52",
        },
    ],
};
