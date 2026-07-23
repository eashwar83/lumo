import { computed, onMounted, ref } from "vue";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";

// Skip intro / credits.
//
// Detection is deliberately not automatic: fingerprinting an opening title
// sequence across episodes is expensive and unreliable, and getting it wrong
// means silently skipping real content. Instead the markers are set once per
// *folder* — which for a ripped or downloaded series is one season — from the
// A-B range, and every other episode in that folder reuses them.
//
// `introStart`/`introEnd` are absolute seconds from the file start.
// `creditsStart` is measured backwards from the end of each file, so it stays
// correct across episodes of slightly different length.

export type SkipMarkers = {
    introStart?: number;
    introEnd?: number;
    /** Seconds before the end of the file where the credits begin. */
    creditsFromEnd?: number;
};

type StoredSkipMarkers = Record<string, SkipMarkers>;

const MAX_ENTRIES = 200;
/** Don't offer to skip credits that start in the last few seconds. */
const MIN_CREDITS_TAIL = 5;

const isLocalPath = (path: string): boolean =>
    !!path && !/^(https?|rtsp|rtmp|smb|webdav):\/\//i.test(path);

/** Folder key for a local path — the series/season directory. */
export const folderKeyForPath = (path: string): string => {
    if (!isLocalPath(path)) return "";
    const trimmed = path.trim().replace(/[\\/]+$/, "");
    const cut = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
    if (cut <= 0) return "";
    return trimmed.slice(0, cut).toLowerCase();
};

type UseSkipMarkersOptions = {
    getPosition: () => number;
    getDuration: () => number;
    onMessage?: (text: string) => void;
    seekTo: (seconds: number) => void | Promise<void>;
    playNext: () => void | Promise<void>;
};

export const useSkipMarkers = ({
    getPosition,
    getDuration,
    onMessage,
    seekTo,
    playNext,
}: UseSkipMarkersOptions) => {
    const entries = new Map<string, SkipMarkers>();
    const saver = createDebouncedUiStateSaver(400);
    /** Skip without asking instead of showing a button. */
    const autoSkip = ref(false);
    const currentFolder = ref("");
    /** Bumped whenever the stored markers change, to re-evaluate the prompt. */
    const revision = ref(0);
    // Dismissing the prompt (or taking the skip) shouldn't make it pop back up.
    let suppressedIntroFor = "";
    let suppressedCreditsFor = "";
    let currentPath = "";

    const persist = () => {
        const object: StoredSkipMarkers = {};
        entries.forEach((value, key) => {
            object[key] = { ...value };
        });
        saver.saveDebounced({ skipMarkers: object, skipMarkersAuto: autoSkip.value });
    };

    const markersForCurrent = computed<SkipMarkers | null>(() => {
        // `revision` is read so this recomputes when the stored markers change.
        void revision.value;
        if (!currentFolder.value) return null;
        return entries.get(currentFolder.value) ?? null;
    });

    const hasIntroMarkers = computed(() => {
        const markers = markersForCurrent.value;
        return (
            markers?.introStart !== undefined && markers?.introEnd !== undefined
        );
    });

    const hasCreditsMarker = computed(
        () => markersForCurrent.value?.creditsFromEnd !== undefined,
    );

    const remember = (patch: SkipMarkers) => {
        const key = currentFolder.value;
        if (!key) return false;
        const existing = entries.get(key) ?? {};
        const next = { ...existing, ...patch };
        entries.delete(key);
        entries.set(key, next);
        while (entries.size > MAX_ENTRIES) {
            const oldest = entries.keys().next().value;
            if (oldest === undefined) break;
            entries.delete(oldest);
        }
        revision.value += 1;
        persist();
        return true;
    };

    /** Save an A-B range as this folder's intro. */
    const saveIntro = (from: number, to: number): boolean => {
        if (!(to > from)) return false;
        const saved = remember({
            introStart: Math.max(0, from),
            introEnd: to,
        });
        onMessage?.(
            saved
                ? "Intro saved for this folder"
                : "Intro markers need a local file",
        );
        return saved;
    };

    /** Save a position as where the credits start, relative to the file end. */
    const saveCredits = (at: number): boolean => {
        const duration = getDuration();
        if (duration <= 0 || at >= duration - MIN_CREDITS_TAIL) {
            onMessage?.("Credits marker is too close to the end");
            return false;
        }
        const saved = remember({ creditsFromEnd: duration - at });
        onMessage?.(
            saved
                ? "Credits saved for this folder"
                : "Credits markers need a local file",
        );
        return saved;
    };

    const clearMarkers = () => {
        const key = currentFolder.value;
        if (!key || !entries.has(key)) return;
        entries.delete(key);
        revision.value += 1;
        persist();
        onMessage?.("Skip markers cleared for this folder");
    };

    // --- live prompt --------------------------------------------------------
    // Driven by the caller's playback tick rather than its own timer, so it
    // costs nothing when no markers exist.

    const promptKind = ref<"intro" | "credits" | null>(null);

    const introEnd = computed(() => markersForCurrent.value?.introEnd ?? 0);

    const skipIntro = async () => {
        const target = introEnd.value;
        promptKind.value = null;
        suppressedIntroFor = currentPath;
        if (target > 0) await seekTo(target);
    };

    const skipCredits = async () => {
        promptKind.value = null;
        suppressedCreditsFor = currentPath;
        await playNext();
    };

    const dismissPrompt = () => {
        if (promptKind.value === "intro") suppressedIntroFor = currentPath;
        if (promptKind.value === "credits") suppressedCreditsFor = currentPath;
        promptKind.value = null;
    };

    /** Call on each playback tick; decides whether a prompt should be showing. */
    const evaluate = () => {
        const markers = markersForCurrent.value;
        if (!markers) {
            promptKind.value = null;
            return;
        }
        const position = getPosition();
        const duration = getDuration();

        const inIntro =
            markers.introStart !== undefined &&
            markers.introEnd !== undefined &&
            position >= markers.introStart &&
            position < markers.introEnd;
        if (inIntro && suppressedIntroFor !== currentPath) {
            if (autoSkip.value) {
                void skipIntro();
                return;
            }
            promptKind.value = "intro";
            return;
        }

        const inCredits =
            markers.creditsFromEnd !== undefined &&
            duration > 0 &&
            position >= duration - markers.creditsFromEnd;
        if (inCredits && suppressedCreditsFor !== currentPath) {
            if (autoSkip.value) {
                void skipCredits();
                return;
            }
            promptKind.value = "credits";
            return;
        }

        promptKind.value = null;
    };

    const setAutoSkip = (enabled: boolean) => {
        autoSkip.value = enabled;
        persist();
        onMessage?.(enabled ? "Auto-skip on" : "Auto-skip off");
    };

    const onFileLoaded = (path: string) => {
        currentPath = path;
        currentFolder.value = folderKeyForPath(path);
        suppressedIntroFor = "";
        suppressedCreditsFor = "";
        promptKind.value = null;
    };

    onMounted(async () => {
        const stored = await loadUiState<{
            skipMarkers?: StoredSkipMarkers;
            skipMarkersAuto?: boolean;
        }>();
        autoSkip.value = stored?.skipMarkersAuto === true;
        Object.entries(stored?.skipMarkers ?? {}).forEach(([key, value]) => {
            const normalized = key.trim();
            if (!normalized) return;
            entries.set(normalized, {
                introStart:
                    typeof value.introStart === "number" ? value.introStart : undefined,
                introEnd:
                    typeof value.introEnd === "number" ? value.introEnd : undefined,
                creditsFromEnd:
                    typeof value.creditsFromEnd === "number"
                        ? value.creditsFromEnd
                        : undefined,
            });
        });
        revision.value += 1;
    });

    return {
        autoSkip,
        promptKind,
        hasIntroMarkers,
        hasCreditsMarker,
        markersForCurrent,
        saveIntro,
        saveCredits,
        clearMarkers,
        skipIntro,
        skipCredits,
        dismissPrompt,
        evaluate,
        setAutoSkip,
        onFileLoaded,
    };
};

export type SkipMarkersController = ReturnType<typeof useSkipMarkers>;
