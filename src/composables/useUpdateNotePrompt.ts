import { computed, nextTick, ref, type Ref } from "vue";
import {
    parseUpdateNoteContent,
    type UpdateNotePrompt,
    type UpdateNoteBlock,
} from "../utils/parseUpdateNoteContent";
import { useUpdateSection } from "./settings-sections";

type UseUpdateNotePromptOptions<PanelId extends string> = {
    activePanel: Ref<PanelId>;
    hideHistory: Ref<boolean>;
    clearNavSelectionDuringLoad: Ref<boolean>;
    settingsPanelId: PanelId;
};

export const useUpdateNotePrompt = <PanelId extends string>({
    activePanel,
    hideHistory,
    clearNavSelectionDuringLoad,
    settingsPanelId,
}: UseUpdateNotePromptOptions<PanelId>) => {
    const updateSection = useUpdateSection();
    const updateNotePrompt = ref<UpdateNotePrompt | null>(null);
    const isUpdateNotePromptOpen = computed(() => updateNotePrompt.value !== null);

    const updateNotePromptTitle = computed(() => {
        const prompt = updateNotePrompt.value;
        return prompt
            ? parseUpdateNoteContent(prompt.version, prompt.note).title
            : "Update Available";
    });
    const updateNotePromptBlocks = computed(() => {
        const prompt = updateNotePrompt.value;
        if (!prompt) return [] as UpdateNoteBlock[];
        return parseUpdateNoteContent(prompt.version, prompt.note).blocks;
    });

    const showUpdateNotePrompt = (prompt: UpdateNotePrompt | null) => {
        const version = prompt?.version?.trim();
        const note = prompt?.note?.trim();
        if (!version || !note) return;
        updateNotePrompt.value = {
            version,
            note,
        };
    };

    const openSettingsAndInstallUpdate = () => {
        clearNavSelectionDuringLoad.value = false;
        activePanel.value = settingsPanelId;
        hideHistory.value = false;
        void nextTick(() => {
            void updateSection.installUpdate();
        });
    };

    const closeUpdateNotePrompt = () => {
        updateNotePrompt.value = null;
    };

    const onConfirmUpdateNotePrompt = () => {
        closeUpdateNotePrompt();
        openSettingsAndInstallUpdate();
    };

    return {
        isUpdateNotePromptOpen,
        updateNotePromptTitle,
        updateNotePromptBlocks,
        showUpdateNotePrompt,
        closeUpdateNotePrompt,
        onConfirmUpdateNotePrompt,
    };
};
