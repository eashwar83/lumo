import type { SettingGroup } from "../mock/settings";

// The Keyboard Shortcuts configuration is persisted as a regular settings group
// (title + label/value item pairs) so it rides the existing ui-state channel
// without any backend change. Each item's `label` is a ShortcutActionId and its
// `value` is a chord string (see `chordFromEvent`) or UNBOUND_CHORD.
export const KEYBOARD_SHORTCUTS_GROUP_TITLE = "Keyboard Shortcuts";

// Sentinel used when an action has no key assigned. A non-empty value is
// required because the settings merge skips empty stored values (which would
// otherwise silently restore the default binding on reload).
export const UNBOUND_CHORD = "None";

const IS_MAC =
    typeof navigator !== "undefined" && /mac|darwin/i.test(navigator.userAgent);

export type ShortcutActionId =
    | "togglePlayPause"
    | "seekBackward"
    | "seekForward"
    | "seekBackwardExact"
    | "seekForwardExact"
    | "seekBackwardLong"
    | "seekForwardLong"
    | "seekToStart"
    | "frameStepBackward"
    | "frameStepForward"
    | "speedDown"
    | "speedUp"
    | "resetSpeed"
    | "toggleLoop"
    | "autoCropNow"
    | "clearCrop"
    | "volumeUp"
    | "volumeDown"
    | "toggleMute"
    | "cycleSubtitleForward"
    | "cycleSubtitleBackward"
    | "toggleSubtitleVisibility"
    | "subtitleDelayDown"
    | "subtitleDelayUp"
    | "cycleAudioTrack"
    | "audioDelayDown"
    | "audioDelayUp"
    | "screenshotWithSubtitles"
    | "screenshotVideoOnly"
    | "toggleFullscreen"
    | "toggleInfo"
    | "showProgress"
    | "togglePlaylist"
    | "previousTrack"
    | "nextTrack"
    | "toggleAlwaysOnTop"
    | "toggleFavorite"
    | "cycleAspectRatio"
    | "fitWindowToVideo"
    | "windowSizeUp"
    | "windowSizeDown";

export type ShortcutActionGroup =
    | "Playback"
    | "Volume"
    | "Subtitles & Audio"
    | "Screenshots"
    | "Interface & Window";

export type ShortcutActionDef = {
    id: ShortcutActionId;
    label: string;
    group: ShortcutActionGroup;
    /** Canonical default chord (see `chordFromEvent`), assuming a US layout. */
    defaultChord: string;
};

export const SHORTCUT_GROUP_ORDER: ShortcutActionGroup[] = [
    "Playback",
    "Volume",
    "Subtitles & Audio",
    "Screenshots",
    "Interface & Window",
];

export const SHORTCUT_ACTIONS: ShortcutActionDef[] = [
    { id: "togglePlayPause", label: "Play / Pause", group: "Playback", defaultChord: "Space" },
    { id: "seekBackward", label: "Seek backward", group: "Playback", defaultChord: "ArrowLeft" },
    { id: "seekForward", label: "Seek forward", group: "Playback", defaultChord: "ArrowRight" },
    { id: "seekBackwardExact", label: "Seek backward 1s (exact)", group: "Playback", defaultChord: "Shift+ArrowLeft" },
    { id: "seekForwardExact", label: "Seek forward 1s (exact)", group: "Playback", defaultChord: "Shift+ArrowRight" },
    { id: "seekBackwardLong", label: "Seek backward 60s", group: "Playback", defaultChord: "PageDown" },
    { id: "seekForwardLong", label: "Seek forward 60s", group: "Playback", defaultChord: "PageUp" },
    { id: "seekToStart", label: "Jump to start", group: "Playback", defaultChord: "Home" },
    { id: "frameStepBackward", label: "Frame step back", group: "Playback", defaultChord: "Comma" },
    { id: "frameStepForward", label: "Frame step forward", group: "Playback", defaultChord: "Period" },
    { id: "speedDown", label: "Slower playback", group: "Playback", defaultChord: "BracketLeft" },
    { id: "speedUp", label: "Faster playback", group: "Playback", defaultChord: "BracketRight" },
    { id: "resetSpeed", label: "Reset playback speed", group: "Playback", defaultChord: "Backspace" },
    { id: "toggleLoop", label: "Loop current file", group: "Playback", defaultChord: "KeyL" },
    { id: "autoCropNow", label: "Auto-crop now", group: "Playback", defaultChord: "KeyC" },
    { id: "clearCrop", label: "Clear crop", group: "Playback", defaultChord: "Shift+KeyC" },
    { id: "cycleAspectRatio", label: "Cycle aspect ratio", group: "Playback", defaultChord: "KeyE" },
    { id: "fitWindowToVideo", label: "Fit window to video", group: "Playback", defaultChord: "KeyG" },

    { id: "volumeUp", label: "Volume up", group: "Volume", defaultChord: "ArrowUp" },
    { id: "volumeDown", label: "Volume down", group: "Volume", defaultChord: "ArrowDown" },
    { id: "toggleMute", label: "Mute / unmute", group: "Volume", defaultChord: "KeyM" },

    { id: "cycleSubtitleForward", label: "Next subtitle track", group: "Subtitles & Audio", defaultChord: "KeyJ" },
    { id: "cycleSubtitleBackward", label: "Previous subtitle track", group: "Subtitles & Audio", defaultChord: "Shift+KeyJ" },
    { id: "toggleSubtitleVisibility", label: "Show / hide subtitles", group: "Subtitles & Audio", defaultChord: "KeyV" },
    { id: "subtitleDelayDown", label: "Subtitle delay −0.1s", group: "Subtitles & Audio", defaultChord: "KeyZ" },
    { id: "subtitleDelayUp", label: "Subtitle delay +0.1s", group: "Subtitles & Audio", defaultChord: "Shift+KeyZ" },
    { id: "cycleAudioTrack", label: "Cycle audio track", group: "Subtitles & Audio", defaultChord: "KeyA" },
    { id: "audioDelayDown", label: "Audio delay −0.1s", group: "Subtitles & Audio", defaultChord: "Ctrl+Minus" },
    { id: "audioDelayUp", label: "Audio delay +0.1s", group: "Subtitles & Audio", defaultChord: "Ctrl+Equal" },

    { id: "screenshotWithSubtitles", label: "Screenshot (with subtitles)", group: "Screenshots", defaultChord: "KeyS" },
    { id: "screenshotVideoOnly", label: "Screenshot (video only)", group: "Screenshots", defaultChord: "Shift+KeyS" },

    { id: "toggleFullscreen", label: "Toggle fullscreen", group: "Interface & Window", defaultChord: "KeyF" },
    { id: "toggleInfo", label: "Media info", group: "Interface & Window", defaultChord: "KeyI" },
    { id: "showProgress", label: "Show position / duration", group: "Interface & Window", defaultChord: "KeyO" },
    { id: "togglePlaylist", label: "Toggle playlist", group: "Interface & Window", defaultChord: "Tab" },
    { id: "previousTrack", label: "Previous playlist item", group: "Interface & Window", defaultChord: "Shift+Comma" },
    { id: "nextTrack", label: "Next playlist item", group: "Interface & Window", defaultChord: "Shift+Period" },
    { id: "toggleAlwaysOnTop", label: "Always on top", group: "Interface & Window", defaultChord: "KeyT" },
    { id: "toggleFavorite", label: "Toggle favourite", group: "Interface & Window", defaultChord: "KeyB" },
    { id: "windowSizeUp", label: "Grow window", group: "Interface & Window", defaultChord: "Alt+Equal" },
    { id: "windowSizeDown", label: "Shrink window", group: "Interface & Window", defaultChord: "Alt+Minus" },
];

const ACTION_BY_ID: Record<ShortcutActionId, ShortcutActionDef> =
    SHORTCUT_ACTIONS.reduce(
        (acc, action) => {
            acc[action.id] = action;
            return acc;
        },
        {} as Record<ShortcutActionId, ShortcutActionDef>,
    );

export const getDefaultChord = (id: ShortcutActionId): string =>
    ACTION_BY_ID[id]?.defaultChord ?? UNBOUND_CHORD;

export const isShortcutActionId = (value: string): value is ShortcutActionId =>
    value in ACTION_BY_ID;

// --- Chord model -----------------------------------------------------------
// A chord is captured from `event.code` (physical, layout-independent) plus the
// active modifiers, joined in a fixed order, e.g. "KeyS", "Shift+ArrowLeft",
// "Ctrl+Equal". Capture and dispatch use the exact same representation.

const MODIFIER_CODES = new Set([
    "ControlLeft",
    "ControlRight",
    "ShiftLeft",
    "ShiftRight",
    "AltLeft",
    "AltRight",
    "MetaLeft",
    "MetaRight",
    "OSLeft",
    "OSRight",
]);

export const isModifierCode = (code: string): boolean => MODIFIER_CODES.has(code);

export const chordFromEvent = (event: KeyboardEvent): string | null => {
    const code = event.code;
    if (!code || isModifierCode(code)) return null;
    const parts: string[] = [];
    if (event.ctrlKey) parts.push("Ctrl");
    if (event.altKey) parts.push("Alt");
    if (event.shiftKey) parts.push("Shift");
    if (event.metaKey) parts.push("Meta");
    parts.push(code);
    return parts.join("+");
};

const MOD_LABELS: Record<string, string> = {
    Ctrl: "Ctrl",
    Alt: "Alt",
    Shift: "Shift",
    Meta: IS_MAC ? "⌘" : "Win",
};

const CODE_LABELS: Record<string, string> = {
    Space: "Space",
    Tab: "Tab",
    Enter: "Enter",
    Escape: "Esc",
    Backspace: "Backspace",
    Delete: "Del",
    Insert: "Ins",
    ArrowLeft: "←",
    ArrowRight: "→",
    ArrowUp: "↑",
    ArrowDown: "↓",
    PageUp: "PgUp",
    PageDown: "PgDn",
    Home: "Home",
    End: "End",
    BracketLeft: "[",
    BracketRight: "]",
    Backslash: "\\",
    Comma: ",",
    Period: ".",
    Slash: "/",
    Semicolon: ";",
    Quote: "'",
    Backquote: "`",
    Minus: "−",
    Equal: "=",
};

const labelForCode = (code: string): string => {
    if (CODE_LABELS[code]) return CODE_LABELS[code];
    if (/^Key[A-Z]$/.test(code)) return code.slice(3);
    if (/^Digit[0-9]$/.test(code)) return code.slice(5);
    if (/^Numpad[0-9]$/.test(code)) return `Num ${code.slice(6)}`;
    if (/^F[0-9]{1,2}$/.test(code)) return code;
    return code;
};

export const formatChord = (chord: string | null | undefined): string => {
    if (!chord || chord === UNBOUND_CHORD) return "—";
    return chord
        .split("+")
        .map((part) => MOD_LABELS[part] ?? labelForCode(part))
        .join(" + ");
};

/** Split a chord into individually renderable key labels (for <kbd> pills). */
export const chordKeyLabels = (chord: string | null | undefined): string[] => {
    if (!chord || chord === UNBOUND_CHORD) return [];
    return chord
        .split("+")
        .map((part) => MOD_LABELS[part] ?? labelForCode(part));
};

// --- Persistence helpers ---------------------------------------------------

type StoredGroupLike = {
    title: string;
    items: Array<{ label: string; value: string }>;
};

export const buildDefaultShortcutMap = (): Record<ShortcutActionId, string> =>
    SHORTCUT_ACTIONS.reduce(
        (acc, action) => {
            acc[action.id] = action.defaultChord;
            return acc;
        },
        {} as Record<ShortcutActionId, string>,
    );

/** Read the persisted Keyboard Shortcuts group into a full binding map. */
export const parseShortcutBindings = (
    groups?: StoredGroupLike[],
): Record<ShortcutActionId, string> => {
    const bindings = buildDefaultShortcutMap();
    const group = groups?.find(
        (candidate) => candidate.title === KEYBOARD_SHORTCUTS_GROUP_TITLE,
    );
    if (!group) return bindings;
    group.items.forEach((item) => {
        if (isShortcutActionId(item.label) && item.value) {
            bindings[item.label] = item.value;
        }
    });
    return bindings;
};

/** Build the default Keyboard Shortcuts settings group from the registry. */
export const buildShortcutSettingGroup = (): SettingGroup => ({
    title: KEYBOARD_SHORTCUTS_GROUP_TITLE,
    items: SHORTCUT_ACTIONS.map((action) => ({
        label: action.id,
        displayLabel: action.label,
        value: action.defaultChord,
        type: "keybind" as const,
        group: action.group,
    })),
});
