import { ref } from "vue";

export const usePlaybackLoadingState = () => ({
    isLoading: ref(false),
    loadingUrl: ref(""),
});
