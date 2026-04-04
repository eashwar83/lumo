import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";

type MediaAssociationStatus = {
    supported: boolean;
    targetBundleId: string;
    isDefaultForAll: boolean;
    missingExtensions: string[];
    checkedExtensions: string[];
};

type MediaAssociationApplyResult = {
    success: boolean;
    failedExtensions: string[];
    status: MediaAssociationStatus;
};

type SetDefaultButtonPhase = "idle" | "checking" | "success" | "failed";

export const useMediaAssociationSection = (isMacOS: boolean) => {
    const mediaAssociationStatus = ref<MediaAssociationStatus | null>(null);
    const isCheckingMediaAssociation = ref(false);
    const isApplyingMediaAssociation = ref(false);
    const mediaAssociationMessage = ref("");
    const setDefaultButtonPhase = ref<SetDefaultButtonPhase>("idle");

    const refreshMediaAssociationStatus = async () => {
        isCheckingMediaAssociation.value = true;
        mediaAssociationMessage.value = "";
        try {
            mediaAssociationStatus.value = await invoke<MediaAssociationStatus>(
                "get_media_association_status",
            );
        } catch {
            mediaAssociationStatus.value = null;
            mediaAssociationMessage.value =
                "Unable to read current default app associations.";
        } finally {
            isCheckingMediaAssociation.value = false;
        }
    };

    const setMediaAssociationToSoia = async () => {
        if (setDefaultButtonPhase.value === "checking") return;
        isApplyingMediaAssociation.value = true;
        setDefaultButtonPhase.value = "checking";
        mediaAssociationMessage.value = "";
        try {
            const result = await invoke<MediaAssociationApplyResult>(
                "set_media_association_to_soia",
            );
            mediaAssociationStatus.value = result.status;
            if (result.status.isDefaultForAll) {
                setDefaultButtonPhase.value = "success";
                mediaAssociationMessage.value =
                    "Soia is now the default app for checked media extensions.";
            } else if (result.failedExtensions.length) {
                setDefaultButtonPhase.value = "failed";
                mediaAssociationMessage.value =
                    "Some extensions could not be updated automatically.";
            } else {
                setDefaultButtonPhase.value = "failed";
                mediaAssociationMessage.value =
                    "Default app was not updated for all checked extensions.";
            }
        } catch {
            setDefaultButtonPhase.value = "failed";
            mediaAssociationMessage.value =
                "Failed to set Soia as the default media app.";
        } finally {
            isApplyingMediaAssociation.value = false;
        }
    };

    const canManageMediaAssociation = computed(() => isMacOS);

    const shouldShowSetDefaultMediaButton = computed(() => {
        const status = mediaAssociationStatus.value;
        const needsAction = Boolean(
            isMacOS && status?.supported && !status.isDefaultForAll,
        );
        return needsAction || setDefaultButtonPhase.value !== "idle";
    });

    const isSetDefaultButtonDisabled = computed(
        () =>
            setDefaultButtonPhase.value === "checking" ||
            setDefaultButtonPhase.value === "success" ||
            setDefaultButtonPhase.value === "failed",
    );

    const isSetDefaultButtonLoading = computed(
        () => setDefaultButtonPhase.value === "checking",
    );

    const setDefaultButtonText = computed(() => {
        if (setDefaultButtonPhase.value === "success") {
            return "Success";
        }
        if (setDefaultButtonPhase.value === "failed") {
            return "Failed";
        }
        return "Set as default";
    });

    const isSetDefaultSuccess = computed(
        () => setDefaultButtonPhase.value === "success",
    );

    const mediaAssociationSummary = computed(() => {
        if (!isMacOS) {
            return "Default media app detection is only available on macOS.";
        }
        const status = mediaAssociationStatus.value;
        if (!status) {
            return "Unable to detect current default app associations.";
        }
        if (!status.supported) {
            return "Default media app detection is unavailable on this system.";
        }
        if (status.isDefaultForAll) {
            return "Soia is already the default app for checked media extensions.";
        }
        if (!status.missingExtensions.length) {
            return "Some media extensions are not currently handled by Soia.";
        }
        const previewLimit = 8;
        const preview = status.missingExtensions.slice(0, previewLimit).join(", ");
        const remaining = status.missingExtensions.length - previewLimit;
        if (remaining > 0) {
            return `Not default for: ${preview}, +${remaining} more.`;
        }
        return `Not default for: ${preview}.`;
    });

    return {
        mediaAssociationStatus,
        canManageMediaAssociation,
        shouldShowSetDefaultMediaButton,
        isSetDefaultButtonDisabled,
        isSetDefaultButtonLoading,
        setDefaultButtonText,
        isSetDefaultSuccess,
        mediaAssociationSummary,
        mediaAssociationMessage,
        isCheckingMediaAssociation,
        isApplyingMediaAssociation,
        refreshMediaAssociationStatus,
        setMediaAssociationToSoia,
    };
};
