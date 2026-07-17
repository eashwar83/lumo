import { invoke } from "@tauri-apps/api/core";

// Reads a local image file and returns it as a base64 data: URL the webview can
// render. Returns null for missing/non-image files or on error.
export const readImageDataUrl = async (
    path: string,
): Promise<string | null> => {
    if (!path?.trim()) return null;
    try {
        return await invoke<string | null>("read_image_data_url", { path });
    } catch {
        return null;
    }
};
