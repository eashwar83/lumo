import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

// Small helpers for exporting/importing JSON to a user-chosen file. The native
// dialog picks the path; a tiny Rust command does the actual read/write (there
// is no filesystem plugin in this app).

/** Save `data` as pretty JSON to a user-chosen path. Returns the path, or null
 *  if the user cancelled. Throws on write failure. */
export const saveJsonFile = async (
    defaultName: string,
    data: unknown,
): Promise<string | null> => {
    const path = await saveDialog({
        defaultPath: defaultName,
        filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!path) return null;
    await invoke("write_text_file", {
        path,
        contents: JSON.stringify(data, null, 2),
    });
    return path;
};

/** Prompt for a JSON file and return its parsed contents, or null if cancelled.
 *  Throws on read/parse failure. */
export const openJsonFile = async (): Promise<unknown | null> => {
    const selected = await openDialog({
        multiple: false,
        directory: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
    });
    const path = Array.isArray(selected) ? selected[0] : selected;
    if (!path) return null;
    const text = await invoke<string>("read_text_file", { path });
    return JSON.parse(text);
};
