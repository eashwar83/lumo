import { onMounted, onUnmounted, reactive } from "vue";
import {
    SETTINGS_UPDATED_EVENT,
    YOUTUBE_AUTOPLAY_SETTING_LABEL,
    YOUTUBE_CHAPTERS_TO_SCENES_SETTING_LABEL,
    YOUTUBE_DOWNLOAD_CONCURRENCY_SETTING_LABEL,
    YOUTUBE_DOWNLOAD_DIR_SETTING_LABEL,
    YOUTUBE_DOWNLOAD_RATE_LIMIT_SETTING_LABEL,
    YOUTUBE_QUALITY_SETTING_LABEL,
    YOUTUBE_SPONSORBLOCK_CATEGORIES_SETTING_LABEL,
    YOUTUBE_SPONSORBLOCK_SETTING_LABEL,
    YTDL_COOKIES_FROM_BROWSER_SETTING_LABEL,
} from "../mock/settings";
import { loadUiState } from "./useUiStateStore";

// Reads the Settings → YouTube values and keeps them live: the settings
// panel broadcasts a DOM event on every change, same as the rest of the app.

type StoredGroup = { title: string; items: Array<{ label: string; value: string }> };

export type YoutubeSettings = {
    /** null = Auto (best available). */
    qualityMaxHeight: number | null;
    autoplayNext: boolean;
    chaptersToScenes: boolean;
    sponsorBlockEnabled: boolean;
    sponsorCategories: string[];
    downloadDir: string;
    downloadConcurrency: number;
    /** 0 = unlimited, otherwise MB/s. */
    downloadRateLimit: number;
    cookiesBrowser: string;
};

const DEFAULTS: YoutubeSettings = {
    qualityMaxHeight: 1080,
    autoplayNext: true,
    chaptersToScenes: true,
    sponsorBlockEnabled: true,
    sponsorCategories: ["sponsor", "intro", "selfpromo"],
    downloadDir: "",
    downloadConcurrency: 2,
    downloadRateLimit: 0,
    cookiesBrowser: "",
};

const valueOf = (groups: StoredGroup[] | undefined, label: string) =>
    groups
        ?.flatMap((group) => group.items)
        .find((item) => item.label === label)?.value;

const parseQuality = (value?: string): number | null => {
    if (!value || value.startsWith("Auto")) return null;
    const parsed = Number.parseInt(value.replace(/\D/g, ""), 10);
    return Number.isFinite(parsed) && parsed > 0 ? parsed : null;
};

export const useYouTubeSettings = () => {
    const settings = reactive<YoutubeSettings>({ ...DEFAULTS });

    const applyGroups = (groups?: StoredGroup[]) => {
        if (!groups?.length) return;
        const quality = valueOf(groups, YOUTUBE_QUALITY_SETTING_LABEL);
        settings.qualityMaxHeight = quality
            ? parseQuality(quality)
            : DEFAULTS.qualityMaxHeight;
        settings.autoplayNext =
            valueOf(groups, YOUTUBE_AUTOPLAY_SETTING_LABEL) !== "Off";
        settings.chaptersToScenes =
            valueOf(groups, YOUTUBE_CHAPTERS_TO_SCENES_SETTING_LABEL) !== "Off";
        settings.sponsorBlockEnabled =
            valueOf(groups, YOUTUBE_SPONSORBLOCK_SETTING_LABEL) !== "Off";
        const categories = valueOf(
            groups,
            YOUTUBE_SPONSORBLOCK_CATEGORIES_SETTING_LABEL,
        );
        settings.sponsorCategories = categories
            ? categories
                  .split(",")
                  .map((entry) => entry.trim())
                  .filter(Boolean)
            : [...DEFAULTS.sponsorCategories];
        settings.downloadDir =
            valueOf(groups, YOUTUBE_DOWNLOAD_DIR_SETTING_LABEL)?.trim() ?? "";
        const concurrency = Number.parseInt(
            valueOf(groups, YOUTUBE_DOWNLOAD_CONCURRENCY_SETTING_LABEL) ?? "",
            10,
        );
        settings.downloadConcurrency =
            Number.isFinite(concurrency) && concurrency >= 1 && concurrency <= 4
                ? concurrency
                : DEFAULTS.downloadConcurrency;
        const rate = Number.parseFloat(
            valueOf(groups, YOUTUBE_DOWNLOAD_RATE_LIMIT_SETTING_LABEL) ?? "",
        );
        settings.downloadRateLimit =
            Number.isFinite(rate) && rate > 0 ? rate : 0;
        const cookies = valueOf(groups, YTDL_COOKIES_FROM_BROWSER_SETTING_LABEL);
        settings.cookiesBrowser =
            cookies && cookies !== "Off" ? cookies : "";
    };

    const onSettingsUpdated = (event: Event) => {
        applyGroups(
            (event as CustomEvent<{ groups?: StoredGroup[] }>).detail?.groups,
        );
    };

    onMounted(async () => {
        const stored = await loadUiState<{
            settings?: { groups?: StoredGroup[] };
        }>();
        applyGroups(stored?.settings?.groups);
        window.addEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    onUnmounted(() => {
        window.removeEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    });

    return { settings };
};
