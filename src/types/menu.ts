// Model for the application menu bar. The tree is built in `useAppMenu` and
// rendered by `MenuBar` / `MenuList`, which stay entirely presentational.
//
// Everything is evaluated fresh each time a menu opens, so `disabled` and
// `checked` can just be plain booleans computed at build time rather than
// reactive getters.

export type MenuAction = {
    kind: "action";
    label: string;
    /** Pre-formatted accelerator, e.g. "Ctrl + S". */
    shortcut?: string;
    disabled?: boolean;
    /** Draws a tick — use for toggles and for the active item of a group. */
    checked?: boolean;
    run: () => void | Promise<void>;
};

export type MenuSubmenu = {
    kind: "submenu";
    label: string;
    disabled?: boolean;
    children: MenuNode[];
};

export type MenuSeparator = { kind: "separator" };

export type MenuNode = MenuAction | MenuSubmenu | MenuSeparator;

export type MenuTopLevel = {
    label: string;
    children: MenuNode[];
};
