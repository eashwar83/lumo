import { onMounted, onUnmounted, ref, watch } from "vue";
import { SETTINGS_UPDATED_EVENT } from "../mock/settings";
import { createDebouncedUiStateSaver, loadUiState } from "./useUiStateStore";
import {
    useGeneralSettingsSection,
    type StoredSettingGroup,
    useRenderingSettingsSection,
    type StoredRenderingState,
    useAboutSection,
    useUpdateSection,
    useMediaAssociationSection,
} from "./settings-sections";

export const useSettingsPanel = () => {
    const isMacOS =
        typeof navigator !== "undefined" && /mac|darwin/i.test(navigator.userAgent);
    const isWindowsPlatform =
        typeof navigator !== "undefined" && /\bwindows\b/i.test(navigator.userAgent);

    const isLoading = ref(true);
    const uiStateSaver = createDebouncedUiStateSaver(300);

    const general = useGeneralSettingsSection(isWindowsPlatform);
    const rendering = useRenderingSettingsSection();
    const about = useAboutSection();
    const update = useUpdateSection();
    const mediaAssociation = useMediaAssociationSection(isMacOS);

    const emitSettingsUpdated = () => {
        if (typeof window === "undefined") return;
        window.dispatchEvent(
            new CustomEvent(SETTINGS_UPDATED_EVENT, {
                detail: {
                    groups: general.toPersistedGroups(),
                },
            }),
        );
    };

    const saveState = () => {
        uiStateSaver.saveDebounced({
            settings: {
                groups: general.toPersistedGroups(),
            },
            rendering: rendering.toPersistedRendering(),
        });
    };

    const loadState = async () => {
        const stored = await loadUiState<{
            settings?: { groups?: StoredSettingGroup[] };
            rendering?: StoredRenderingState;
        }>();

        await general.loadGeneralSettings(stored?.settings?.groups);
        await rendering.loadRenderingSettings(stored?.rendering);
        isLoading.value = false;
    };

    const resetAllSettings = () => {
        general.resetGeneralSettings();
        rendering.resetRenderingSettings();
    };

    onMounted(() => {
        void loadState();
        void about.loadRuntimeVersions();
        void mediaAssociation.refreshMediaAssociationStatus();
        void update.loadUpdateAvailability();
        void update.setupUpdateAvailabilityListener();
    });

    onUnmounted(() => {
        general.dispose();
        rendering.dispose();
        update.dispose();
    });

    watch(
        general.settingGroups,
        () => {
            if (isLoading.value) return;
            general.applySectionSideEffects();
            saveState();
            emitSettingsUpdated();
        },
        { deep: true },
    );

    watch(
        [rendering.selectedShaderFiles, rendering.activeShaderFiles],
        () => {
            if (isLoading.value) return;
            rendering.scheduleApplyRenderingOptions();
            saveState();
        },
        { deep: true },
    );

    return {
        settingGroups: general.settingGroups,
        runtimeVersions: about.runtimeVersions,
        mediaAssociationStatus: mediaAssociation.mediaAssociationStatus,
        canManageMediaAssociation: mediaAssociation.canManageMediaAssociation,
        shouldShowSetDefaultMediaButton:
            mediaAssociation.shouldShowSetDefaultMediaButton,
        isSetDefaultButtonDisabled: mediaAssociation.isSetDefaultButtonDisabled,
        isSetDefaultButtonLoading: mediaAssociation.isSetDefaultButtonLoading,
        setDefaultButtonText: mediaAssociation.setDefaultButtonText,
        isSetDefaultSuccess: mediaAssociation.isSetDefaultSuccess,
        shouldShowUpdateButton: update.shouldShowUpdateButton,
        isUpdateButtonDisabled: update.isUpdateButtonDisabled,
        updateButtonText: update.updateButtonText,
        isUpdateRetry: update.isUpdateRetry,
        shouldShowUpdateStatus: update.shouldShowUpdateStatus,
        updateStatusText: update.updateStatusText,
        shouldShowUpdateHint: update.shouldShowUpdateHint,
        updateHintText: update.updateHintText,
        mediaAssociationSummary: mediaAssociation.mediaAssociationSummary,
        mediaAssociationMessage: mediaAssociation.mediaAssociationMessage,
        isCheckingMediaAssociation: mediaAssociation.isCheckingMediaAssociation,
        isApplyingMediaAssociation: mediaAssociation.isApplyingMediaAssociation,
        refreshMediaAssociationStatus: mediaAssociation.refreshMediaAssociationStatus,
        setMediaAssociationToSoia: mediaAssociation.setMediaAssociationToSoia,
        installUpdate: update.installUpdate,
        openProjectGithub: about.openProjectGithub,
        resetAllSettings,
        browseForPath: general.browseForPath,
        browseForCustomShaders: rendering.browseForCustomShaders,
        selectedShaderFiles: rendering.selectedShaderFiles,
        activeShaderFiles: rendering.activeShaderFiles,
        unavailableShaderFiles: rendering.unavailableShaderFiles,
        setShaderEnabled: rendering.setShaderEnabled,
        removeShaderFromList: rendering.removeShaderFromList,
        clearShaders: rendering.clearShaders,
        isFixedLogPathItem: general.isFixedLogPathItem,
        isLoading,
    };
};
