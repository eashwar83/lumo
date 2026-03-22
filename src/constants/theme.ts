export type AppTheme = "system" | "light" | "dark" | "graphite";

type StoredSettingItemLike = { label?: string; value?: string };
type StoredSettingGroupLike = { title?: string; items?: StoredSettingItemLike[] };

export const THEME_SETTING_GROUP_TITLE = "Appearance";
export const THEME_SETTING_LABEL = "Theme";
export const THEME_SYSTEM_OPTION = "System (Auto)";
export const THEME_LIGHT_OPTION = "Light";
export const THEME_DARK_OPTION = "Dark";
export const THEME_GRAPHITE_OPTION = "Graphite";
export const THEME_OPTIONS = [
    THEME_SYSTEM_OPTION,
    THEME_LIGHT_OPTION,
    THEME_DARK_OPTION,
    THEME_GRAPHITE_OPTION,
];

const optionToTheme: Record<string, AppTheme> = {
    [THEME_SYSTEM_OPTION]: "system",
    [THEME_LIGHT_OPTION]: "light",
    [THEME_DARK_OPTION]: "dark",
    [THEME_GRAPHITE_OPTION]: "graphite",
};

const getThemeFromSettingValue = (value?: string): AppTheme =>
    optionToTheme[value ?? ""] ?? "graphite";

const extractThemeSettingValue = (
    groups?: StoredSettingGroupLike[],
): string | undefined => {
    if (!groups?.length) return undefined;
    const appearanceGroup = groups.find(
        (group) => group.title === THEME_SETTING_GROUP_TITLE,
    );
    const themeItem = appearanceGroup?.items?.find(
        (item) => item.label === THEME_SETTING_LABEL,
    );
    return themeItem?.value;
};

export const extractAppThemeFromSettingGroups = (
    groups?: StoredSettingGroupLike[],
): AppTheme => getThemeFromSettingValue(extractThemeSettingValue(groups));

export const applyAppTheme = (theme: AppTheme) => {
    if (typeof document === "undefined") return;
    const root = document.documentElement;
    if (theme === "system") {
        root.removeAttribute("data-theme");
        return;
    }
    root.setAttribute("data-theme", theme);
};

export const applyThemeFromSettingGroups = (
    groups?: StoredSettingGroupLike[],
) => {
    const theme = extractAppThemeFromSettingGroups(groups);
    applyAppTheme(theme);
};
