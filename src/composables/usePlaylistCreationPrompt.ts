import { computed, ref } from "vue";

export type PlaylistCreationPromptRequest = {
    defaultName: string;
    itemCount: number;
    sourceLabel?: string;
};

export type PlaylistCreationPromptResult = {
    shouldCreate: boolean;
    name: string;
};

type PendingPrompt = {
    resolve: (result: PlaylistCreationPromptResult) => void;
};

export const usePlaylistCreationPrompt = () => {
    const isOpen = ref(false);
    const defaultName = ref("");
    const itemCount = ref(0);
    const nameDraft = ref("");
    const sourceLabel = ref("");
    let pendingPrompt: PendingPrompt | null = null;

    const message = computed(() => {
        const name = nameDraft.value.trim() || defaultName.value || "Playlist";
        const itemLabel = itemCount.value === 1 ? "item" : "items";
        const source = sourceLabel.value.trim();
        if (source) {
            return `${source}\ncontains ${itemCount.value} ${itemLabel}. A playlist named "${name}" will be created.`;
        }
        return `Contains ${itemCount.value} ${itemLabel}. A playlist named "${name}" will be created.`;
    });

    const resolvePrompt = (shouldCreate: boolean) => {
        const pending = pendingPrompt;
        if (!pending) return;

        const fallbackName = defaultName.value || "Playlist";
        const name = nameDraft.value.trim() || fallbackName;
        pendingPrompt = null;
        isOpen.value = false;
        pending.resolve({ shouldCreate, name });
    };

    const requestPlaylistCreation = (
        request: PlaylistCreationPromptRequest,
    ): Promise<PlaylistCreationPromptResult> => {
        if (pendingPrompt) {
            resolvePrompt(false);
        }

        defaultName.value = request.defaultName.trim() || "Playlist";
        itemCount.value = Math.max(0, request.itemCount);
        nameDraft.value = defaultName.value;
        sourceLabel.value = request.sourceLabel?.trim() ?? "";
        isOpen.value = true;

        return new Promise((resolve) => {
            pendingPrompt = { resolve };
        });
    };

    const cancelPlaylistCreation = () => {
        resolvePrompt(false);
    };

    const confirmPlaylistCreation = () => {
        resolvePrompt(true);
    };

    return {
        isOpen,
        defaultName,
        itemCount,
        nameDraft,
        sourceLabel,
        message,
        requestPlaylistCreation,
        cancelPlaylistCreation,
        confirmPlaylistCreation,
    };
};
