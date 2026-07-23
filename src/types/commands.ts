// The command registry that the shortcut builder binds against.
//
// Two shapes cover almost everything the app can do:
//
//   * `action`  — a discrete thing: open a panel, apply a preset, toggle a mode.
//   * `adjust`  — a numeric setting, which becomes three bindable commands
//                 (increase by N / decrease by N / set to N) from one entry.
//
// A third kind of binding, a raw mpv command, isn't in the registry at all: it
// has no fixed identity, the user types it. See `CustomShortcut`.

export type AdjustSpec = {
    min: number;
    max: number;
    /** Default increment offered in the builder. */
    step: number;
    unit?: string;
    /** Decimal places for the OSD readout. */
    precision?: number;
};

export type ActionCommand = {
    kind: "action";
    /** Stable across releases — persisted in user settings. */
    id: string;
    label: string;
    group: string;
    run: () => void | Promise<void>;
};

export type AdjustCommand = {
    kind: "adjust";
    id: string;
    label: string;
    group: string;
    spec: AdjustSpec;
    get: () => number;
    set: (value: number) => void | Promise<void>;
};

export type CommandDef = ActionCommand | AdjustCommand;

export type AdjustMode = "increase" | "decrease" | "set";

export type CustomShortcut = {
    /** Local identity for list operations; not meaningful across machines. */
    id: string;
    chord: string;
    kind: "action" | "adjust" | "mpv";
    /** For action/adjust: the registry command id. */
    commandId?: string;
    /** For adjust. */
    mode?: AdjustMode;
    amount?: number;
    /** For mpv: the raw command line, e.g. "cycle deinterlace". */
    mpvCommand?: string;
};

/** How a bound custom shortcut reads in the settings list. */
export const describeCustomShortcut = (
    entry: CustomShortcut,
    command?: CommandDef,
): string => {
    if (entry.kind === "mpv") {
        return entry.mpvCommand?.trim() || "mpv command";
    }
    if (!command) return "Unavailable command";
    if (command.kind === "action" || entry.kind === "action") {
        return command.label;
    }
    const unit = command.kind === "adjust" ? (command.spec.unit ?? "") : "";
    const amount = entry.amount ?? 0;
    if (entry.mode === "set") return `${command.label} → ${amount}${unit}`;
    const sign = entry.mode === "decrease" ? "−" : "+";
    return `${command.label} ${sign}${Math.abs(amount)}${unit}`;
};
