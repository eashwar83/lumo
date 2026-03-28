export interface ProgressPayload {
    time_pos: number;
    duration: number;
    buffered_pos?: number;
    is_playing: boolean;
    video_bitrate?: number;
    is_buffering?: boolean;
}

export interface MediaTrack {
    id: number | string;
    track_type: string;
    title: string;
    lang?: string;
    selected?: boolean;
    codec?: string;
    codec_desc?: string;
    decoder_desc?: string;
    demux_w?: number;
    demux_h?: number;
    demux_fps?: number;
    demux_bitrate?: number;
    demux_samplerate?: number;
    demux_channels?: string;
    fps?: number;
    w?: number;
    h?: number;
    is_default?: boolean;
    forced?: boolean;
    external?: boolean;
}
