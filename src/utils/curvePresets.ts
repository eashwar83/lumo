// Built-in tone-curve presets. Each preset is a full set of control points per
// channel (rgb master + r/g/b). Unlike the video-menu "looks" (which are slider
// values), these are precise transfer functions — S-curves, split-toning, matte
// blacks, cross-process shifts — the kind of thing curves do well and sliders
// can't. B&W is intentionally absent: desaturation isn't a per-channel curve op
// (it needs an RGB→luma mix), so it stays in the video menu.

import { cloneCurves, identityPoints, type CurvePoint, type Curves } from "./curves";

export type CurvePresetCategory = "Tone" | "Warm" | "Cool" | "Cinematic";

export type CurvePreset = {
    id: string;
    name: string;
    category: CurvePresetCategory;
    curves: Curves;
};

// Convenience builder: any channel left out defaults to identity.
const make = (
    parts: Partial<Record<keyof Curves, CurvePoint[]>>,
): Curves => ({
    rgb: parts.rgb ?? identityPoints(),
    r: parts.r ?? identityPoints(),
    g: parts.g ?? identityPoints(),
    b: parts.b ?? identityPoints(),
});

const preset = (
    id: string,
    name: string,
    category: CurvePresetCategory,
    parts: Partial<Record<keyof Curves, CurvePoint[]>>,
): CurvePreset => ({ id, name, category, curves: make(parts) });

export const BUILT_IN_CURVE_PRESETS: CurvePreset[] = [
    // --- Tone & contrast (neutral colour, master curve only) ---
    preset("soft-contrast", "Soft", "Tone", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.25, y: 0.22 },
            { x: 0.75, y: 0.78 },
            { x: 1, y: 1 },
        ],
    }),
    preset("medium-contrast", "Medium", "Tone", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.25, y: 0.18 },
            { x: 0.75, y: 0.82 },
            { x: 1, y: 1 },
        ],
    }),
    preset("strong-contrast", "Strong", "Tone", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.2, y: 0.1 },
            { x: 0.8, y: 0.9 },
            { x: 1, y: 1 },
        ],
    }),
    preset("matte", "Matte", "Tone", {
        rgb: [
            { x: 0, y: 0.09 },
            { x: 0.25, y: 0.28 },
            { x: 0.75, y: 0.78 },
            { x: 1, y: 0.93 },
        ],
    }),
    preset("crushed", "Crush", "Tone", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.2, y: 0.06 },
            { x: 0.55, y: 0.55 },
            { x: 1, y: 1 },
        ],
    }),
    preset("flat", "Flat", "Tone", {
        rgb: [
            { x: 0, y: 0.08 },
            { x: 0.25, y: 0.3 },
            { x: 0.75, y: 0.72 },
            { x: 1, y: 0.9 },
        ],
    }),
    preset("brighten", "Brighten", "Tone", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.25, y: 0.36 },
            { x: 0.5, y: 0.62 },
            { x: 1, y: 1 },
        ],
    }),

    // --- Warm ---
    preset("warm-film", "Warm Film", "Warm", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.25, y: 0.23 },
            { x: 0.75, y: 0.79 },
            { x: 1, y: 1 },
        ],
        r: [
            { x: 0, y: 0.03 },
            { x: 0.5, y: 0.55 },
            { x: 1, y: 1 },
        ],
        b: [
            { x: 0, y: 0 },
            { x: 0.5, y: 0.45 },
            { x: 1, y: 0.96 },
        ],
    }),
    preset("golden-hour", "Golden", "Warm", {
        rgb: [
            { x: 0, y: 0.02 },
            { x: 1, y: 1 },
        ],
        r: [
            { x: 0, y: 0.04 },
            { x: 0.5, y: 0.58 },
            { x: 1, y: 1 },
        ],
        b: [
            { x: 0, y: 0 },
            { x: 0.5, y: 0.42 },
            { x: 1, y: 0.9 },
        ],
    }),
    preset("sepia", "Sepia", "Warm", {
        rgb: [
            { x: 0, y: 0.02 },
            { x: 0.5, y: 0.52 },
            { x: 1, y: 0.98 },
        ],
        r: [
            { x: 0, y: 0.12 },
            { x: 0.5, y: 0.6 },
            { x: 1, y: 1 },
        ],
        g: [
            { x: 0, y: 0.06 },
            { x: 0.5, y: 0.48 },
            { x: 1, y: 0.92 },
        ],
        b: [
            { x: 0, y: 0.04 },
            { x: 0.5, y: 0.3 },
            { x: 1, y: 0.7 },
        ],
    }),
    preset("vintage-fade", "Vintage", "Warm", {
        rgb: [
            { x: 0, y: 0.1 },
            { x: 0.75, y: 0.8 },
            { x: 1, y: 0.92 },
        ],
        r: [
            { x: 0, y: 0.06 },
            { x: 1, y: 1 },
        ],
        g: [
            { x: 0, y: 0.04 },
            { x: 1, y: 0.98 },
        ],
        b: [
            { x: 0, y: 0.13 },
            { x: 0.5, y: 0.45 },
            { x: 1, y: 0.85 },
        ],
    }),

    // --- Cool ---
    preset("cool-film", "Cool Film", "Cool", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.25, y: 0.23 },
            { x: 0.75, y: 0.79 },
            { x: 1, y: 1 },
        ],
        r: [
            { x: 0, y: 0 },
            { x: 0.5, y: 0.46 },
            { x: 1, y: 0.97 },
        ],
        b: [
            { x: 0, y: 0.03 },
            { x: 0.5, y: 0.55 },
            { x: 1, y: 1 },
        ],
    }),
    preset("moonlight", "Moonlight", "Cool", {
        rgb: [
            { x: 0, y: 0.06 },
            { x: 0.25, y: 0.2 },
            { x: 0.75, y: 0.7 },
            { x: 1, y: 0.88 },
        ],
        r: [
            { x: 0, y: 0 },
            { x: 0.5, y: 0.44 },
            { x: 1, y: 0.94 },
        ],
        b: [
            { x: 0, y: 0.08 },
            { x: 0.5, y: 0.56 },
            { x: 1, y: 1 },
        ],
    }),
    preset("cyanotype", "Cyan", "Cool", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.5, y: 0.52 },
            { x: 1, y: 1 },
        ],
        r: [
            { x: 0, y: 0 },
            { x: 0.5, y: 0.32 },
            { x: 1, y: 0.72 },
        ],
        g: [
            { x: 0, y: 0.05 },
            { x: 0.5, y: 0.5 },
            { x: 1, y: 0.9 },
        ],
        b: [
            { x: 0, y: 0.15 },
            { x: 0.5, y: 0.62 },
            { x: 1, y: 1 },
        ],
    }),

    // --- Cinematic ---
    preset("teal-orange", "Teal & Orange", "Cinematic", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.25, y: 0.2 },
            { x: 0.75, y: 0.8 },
            { x: 1, y: 1 },
        ],
        r: [
            { x: 0, y: 0 },
            { x: 0.25, y: 0.2 },
            { x: 0.75, y: 0.8 },
            { x: 1, y: 1 },
        ],
        b: [
            { x: 0, y: 0.05 },
            { x: 0.25, y: 0.34 },
            { x: 0.75, y: 0.66 },
            { x: 1, y: 0.95 },
        ],
        g: [
            { x: 0, y: 0.02 },
            { x: 0.5, y: 0.5 },
            { x: 1, y: 0.99 },
        ],
    }),
    preset("bleach-bypass", "Bleach", "Cinematic", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.2, y: 0.1 },
            { x: 0.5, y: 0.52 },
            { x: 0.8, y: 0.9 },
            { x: 1, y: 1 },
        ],
        r: [
            { x: 0, y: 0.03 },
            { x: 1, y: 1 },
        ],
        b: [
            { x: 0, y: 0.03 },
            { x: 1, y: 0.99 },
        ],
    }),
    preset("cross-process", "Cross", "Cinematic", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.25, y: 0.2 },
            { x: 0.75, y: 0.82 },
            { x: 1, y: 1 },
        ],
        r: [
            { x: 0, y: 0.06 },
            { x: 0.5, y: 0.55 },
            { x: 1, y: 1 },
        ],
        g: [
            { x: 0, y: 0 },
            { x: 0.5, y: 0.52 },
            { x: 1, y: 1 },
        ],
        b: [
            { x: 0, y: 0.12 },
            { x: 0.5, y: 0.4 },
            { x: 1, y: 0.9 },
        ],
    }),
    preset("punch", "Punch", "Cinematic", {
        rgb: [
            { x: 0, y: 0 },
            { x: 0.22, y: 0.13 },
            { x: 0.78, y: 0.87 },
            { x: 1, y: 1 },
        ],
    }),
];

export const CURVE_PRESET_CATEGORY_ORDER: CurvePresetCategory[] = [
    "Tone",
    "Warm",
    "Cool",
    "Cinematic",
];

// Blend a preset's curves toward the identity diagonal (y = x) by `strength`
// (0..1). At 1 the preset is applied in full; at 0 it's a no-op. The blend keeps
// each control point's x and pulls its y toward the diagonal, so the effect
// scales smoothly and stays visible in the editor as the curve flattens.
export const blendCurvesTowardIdentity = (
    curves: Curves,
    strength: number,
): Curves => {
    const s = Math.min(1, Math.max(0, strength));
    if (s >= 1) return cloneCurves(curves);
    const blendChannel = (points: CurvePoint[]): CurvePoint[] =>
        points.map((p) => ({ x: p.x, y: p.x + s * (p.y - p.x) }));
    return {
        rgb: blendChannel(curves.rgb),
        r: blendChannel(curves.r),
        g: blendChannel(curves.g),
        b: blendChannel(curves.b),
    };
};
