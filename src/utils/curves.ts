// RGB tone-curve model + baking. A curve is a set of control points (x,y in
// 0..1) per channel; we interpolate a smooth, overshoot-free monotone-cubic
// spline through them and bake a 256-entry LUT that a GPU shader applies live.

export type CurvePoint = { x: number; y: number };
export type CurveChannel = "rgb" | "r" | "g" | "b";
export type Curves = Record<CurveChannel, CurvePoint[]>;

export const CURVE_CHANNELS: CurveChannel[] = ["rgb", "r", "g", "b"];

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

export const identityPoints = (): CurvePoint[] => [
    { x: 0, y: 0 },
    { x: 1, y: 1 },
];

export const defaultCurves = (): Curves => ({
    rgb: identityPoints(),
    r: identityPoints(),
    g: identityPoints(),
    b: identityPoints(),
});

const isIdentityChannel = (points: CurvePoint[]): boolean => {
    if (points.length !== 2) return false;
    const [a, b] = points;
    return (
        Math.abs(a.x) < 1e-4 &&
        Math.abs(a.y) < 1e-4 &&
        Math.abs(b.x - 1) < 1e-4 &&
        Math.abs(b.y - 1) < 1e-4
    );
};

export const isIdentityCurves = (curves: Curves): boolean =>
    CURVE_CHANNELS.every((ch) => isIdentityChannel(curves[ch]));

// --- Monotone cubic Hermite (Fritsch–Carlson) --------------------------------

type Spline = { xs: number[]; ys: number[]; t: number[] };

const buildSpline = (input: CurvePoint[]): Spline => {
    // Sort + dedupe by x so the spline is well-formed regardless of edit order.
    const points = [...input]
        .map((p) => ({ x: clamp01(p.x), y: clamp01(p.y) }))
        .sort((a, b) => a.x - b.x);
    const pts: CurvePoint[] = [];
    for (const p of points) {
        if (pts.length && Math.abs(p.x - pts[pts.length - 1].x) < 1e-5) {
            pts[pts.length - 1] = p; // last write wins for coincident x
        } else {
            pts.push(p);
        }
    }
    if (pts.length < 2) {
        return { xs: [0, 1], ys: [0, 1], t: [1, 1] };
    }

    const n = pts.length;
    const xs = pts.map((p) => p.x);
    const ys = pts.map((p) => p.y);
    const m: number[] = [];
    for (let i = 0; i < n - 1; i++) {
        const dx = xs[i + 1] - xs[i];
        m[i] = dx !== 0 ? (ys[i + 1] - ys[i]) / dx : 0;
    }
    const t: number[] = new Array(n);
    t[0] = m[0];
    t[n - 1] = m[n - 2];
    for (let i = 1; i < n - 1; i++) {
        t[i] = m[i - 1] * m[i] <= 0 ? 0 : (m[i - 1] + m[i]) / 2;
    }
    for (let i = 0; i < n - 1; i++) {
        if (m[i] === 0) {
            t[i] = 0;
            t[i + 1] = 0;
            continue;
        }
        const a = t[i] / m[i];
        const b = t[i + 1] / m[i];
        const h = Math.hypot(a, b);
        if (h > 3) {
            const s = 3 / h;
            t[i] = s * a * m[i];
            t[i + 1] = s * b * m[i];
        }
    }
    return { xs, ys, t };
};

const evalSpline = (spline: Spline, x: number): number => {
    const { xs, ys, t } = spline;
    const n = xs.length;
    if (x <= xs[0]) return clamp01(ys[0]);
    if (x >= xs[n - 1]) return clamp01(ys[n - 1]);
    let i = 0;
    while (i < n - 1 && x > xs[i + 1]) i++;
    const h = xs[i + 1] - xs[i];
    const s = h !== 0 ? (x - xs[i]) / h : 0;
    const s2 = s * s;
    const s3 = s2 * s;
    const h00 = 2 * s3 - 3 * s2 + 1;
    const h10 = s3 - 2 * s2 + s;
    const h01 = -2 * s3 + 3 * s2;
    const h11 = s3 - s2;
    const y =
        h00 * ys[i] + h10 * h * t[i] + h01 * ys[i + 1] + h11 * h * t[i + 1];
    return clamp01(y);
};

// A reusable evaluator for a single channel's curve (used by the editor to draw
// the line).
export const makeCurveEvaluator = (
    points: CurvePoint[],
): ((x: number) => number) => {
    const spline = buildSpline(points);
    return (x: number) => evalSpline(spline, x);
};

// Bake the combined per-channel LUT: channel curve first, then the master (rgb)
// curve on top — the standard compositing order. Returns 256*4 RGBA8 bytes.
export const buildCurvesLut = (curves: Curves): Uint8Array => {
    const master = buildSpline(curves.rgb);
    const red = buildSpline(curves.r);
    const green = buildSpline(curves.g);
    const blue = buildSpline(curves.b);
    const lut = new Uint8Array(256 * 4);
    for (let i = 0; i < 256; i++) {
        const v = i / 255;
        const r = evalSpline(master, evalSpline(red, v));
        const g = evalSpline(master, evalSpline(green, v));
        const b = evalSpline(master, evalSpline(blue, v));
        lut[i * 4 + 0] = Math.round(r * 255);
        lut[i * 4 + 1] = Math.round(g * 255);
        lut[i * 4 + 2] = Math.round(b * 255);
        lut[i * 4 + 3] = 255;
    }
    return lut;
};

// --- Serialization -----------------------------------------------------------

export const serializeCurves = (curves: Curves): string => {
    if (isIdentityCurves(curves)) return "";
    // Compact: round to 4 decimals.
    const round = (p: CurvePoint) => ({
        x: Math.round(p.x * 10000) / 10000,
        y: Math.round(p.y * 10000) / 10000,
    });
    return JSON.stringify({
        rgb: curves.rgb.map(round),
        r: curves.r.map(round),
        g: curves.g.map(round),
        b: curves.b.map(round),
    });
};

const sanitizeChannel = (value: unknown): CurvePoint[] => {
    if (!Array.isArray(value)) return identityPoints();
    const points = value
        .filter(
            (p): p is CurvePoint =>
                !!p &&
                typeof (p as CurvePoint).x === "number" &&
                typeof (p as CurvePoint).y === "number",
        )
        .map((p) => ({ x: clamp01(p.x), y: clamp01(p.y) }));
    return points.length >= 2 ? points : identityPoints();
};

export const parseCurves = (json: string | undefined | null): Curves => {
    if (!json || !json.trim()) return defaultCurves();
    try {
        const raw = JSON.parse(json) as Record<CurveChannel, unknown>;
        return {
            rgb: sanitizeChannel(raw.rgb),
            r: sanitizeChannel(raw.r),
            g: sanitizeChannel(raw.g),
            b: sanitizeChannel(raw.b),
        };
    } catch {
        return defaultCurves();
    }
};

// --- Auto (from an aggregate histogram) -------------------------------------

export type ChannelHistograms = {
    r: number[];
    g: number[];
    b: number[];
    luma: number[];
};

// Black/white input points (0..1) by clipping a small % at each end. Bins may
// be peak-normalised; the CDF ratio is scale-invariant so that's fine.
const channelLevels = (
    bins: number[],
    clip = 0.0025,
): { black: number; white: number } => {
    const n = bins.length;
    const total = bins.reduce((a, b) => a + b, 0) || 1;
    let acc = 0;
    let black = 0;
    for (let i = 0; i < n; i++) {
        acc += bins[i];
        if (acc / total >= clip) {
            black = i / (n - 1);
            break;
        }
    }
    acc = 0;
    let white = 1;
    for (let i = n - 1; i >= 0; i--) {
        acc += bins[i];
        if (acc / total >= clip) {
            white = i / (n - 1);
            break;
        }
    }
    if (white <= black) white = Math.min(1, black + 0.05);
    return { black, white };
};

// Mean of a histogram (0..1). Peak-normalisation cancels in the ratio.
const histogramMean = (bins: number[]): number => {
    const n = bins.length;
    let sum = 0;
    let weight = 0;
    for (let i = 0; i < n; i++) {
        sum += bins[i] * i;
        weight += bins[i];
    }
    return weight > 0 ? sum / weight / (n - 1) : 0.5;
};

const median01 = (bins: number[]): number => {
    const n = bins.length;
    const total = bins.reduce((a, b) => a + b, 0) || 1;
    let acc = 0;
    for (let i = 0; i < n; i++) {
        acc += bins[i];
        if (acc / total >= 0.5) return i / (n - 1);
    }
    return 0.5;
};

// A conservative "auto" grade built on the principle: only fix a genuine
// problem, and never invent one. On already-decent footage it is near-identity.
//
//   1. LEVELS  — pull black/white to the real clip points. Only ever expands
//      headroom that already exists, so it can't wash out or crush anything.
//   2. MIDTONE — a gentle lift, but ONLY when the footage is truly under-
//      exposed after the level stretch. A merely "dark"/moody grade is left
//      alone (mistaking dark for underexposed is what flattened films before).
//   3. WHITE BALANCE — NOT gray-world (which fights every warm/cool artistic
//      grade and adds a cast of its own). We only touch colour when there's a
//      *strong* global cast — the kind a broken/aged source has — and even then
//      only at low strength and tightly capped. A normal graded film's channel
//      imbalance is below threshold and is left untouched.
export const autoCurvesFromHistogram = (hist: ChannelHistograms): Curves => {
    const LEVEL_CLIP = 0.0025; // fraction clipped at each end for levels
    const LEVEL_STRENGTH = 0.9; // how far to pull black/white to the clip point
    const DARK_MEDIAN = 0.18; // post-stretch median below this = underexposed
    const MIDTONE_TARGET = 0.3; // comfortable midtone to lift a dark image toward
    const MIDTONE_STRENGTH = 0.3; // partial lift, so it never overshoots
    const WB_CAST_THRESHOLD = 0.08; // only correct a cast bigger than 8%
    const WB_STRENGTH = 0.25; // fraction of the cast to remove
    const WB_CAP = 0.06; // hard cap on per-channel correction (±6%)

    // --- Master: auto-levels on luma ---
    const lum = channelLevels(hist.luma, LEVEL_CLIP);
    const black = lum.black * LEVEL_STRENGTH;
    const white = 1 - (1 - lum.white) * LEVEL_STRENGTH;
    const span = Math.max(0.05, white - black);

    const master: CurvePoint[] = [{ x: 0, y: 0 }];
    if (black > 0.02) master.push({ x: Math.min(black, 0.4), y: 0 });

    // --- Midtone lift: only for genuinely underexposed footage ---
    const median = median01(hist.luma);
    const stretchedMedian = clamp01((median - black) / span);
    if (median > 0.03 && stretchedMedian < DARK_MEDIAN) {
        const targetY =
            stretchedMedian +
            (MIDTONE_TARGET - stretchedMedian) * MIDTONE_STRENGTH;
        const midX = median;
        const prevX = master[master.length - 1].x;
        if (
            midX > prevX + 0.04 &&
            midX < white - 0.04 &&
            targetY - stretchedMedian > 0.01
        ) {
            master.push({
                x: midX,
                y: Math.min(0.97, Math.max(0.03, targetY)),
            });
        }
    }

    if (white < 0.985) master.push({ x: Math.max(white, 0.6), y: 1 });
    master.push({ x: 1, y: 1 });

    // --- White balance: only remove a strong global cast ---
    const meanR = histogramMean(hist.r);
    const meanG = histogramMean(hist.g);
    const meanB = histogramMean(hist.b);
    const avg = (meanR + meanG + meanB) / 3 || 0.5;
    const devR = meanR / avg - 1;
    const devG = meanG / avg - 1;
    const devB = meanB / avg - 1;
    const maxDev = Math.max(Math.abs(devR), Math.abs(devG), Math.abs(devB));

    const gainFor = (dev: number): number => {
        if (maxDev <= WB_CAST_THRESHOLD) return 1; // no meaningful cast
        const g = 1 - dev * WB_STRENGTH;
        return Math.min(1 + WB_CAP, Math.max(1 - WB_CAP, g));
    };
    const gainCurve = (k: number): CurvePoint[] => {
        if (Math.abs(k - 1) < 0.015) return identityPoints();
        if (k > 1) {
            const x1 = Math.min(0.995, 1 / k);
            return [
                { x: 0, y: 0 },
                { x: x1, y: 1 },
                { x: 1, y: 1 },
            ];
        }
        return [
            { x: 0, y: 0 },
            { x: 1, y: k },
        ];
    };

    return {
        rgb: master.length >= 2 ? master : identityPoints(),
        r: gainCurve(gainFor(devR)),
        g: gainCurve(gainFor(devG)),
        b: gainCurve(gainFor(devB)),
    };
};

export const cloneCurves = (curves: Curves): Curves => ({
    rgb: curves.rgb.map((p) => ({ ...p })),
    r: curves.r.map((p) => ({ ...p })),
    g: curves.g.map((p) => ({ ...p })),
    b: curves.b.map((p) => ({ ...p })),
});
