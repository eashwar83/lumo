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

// A gentle "auto" grade: contrast + brightness via a luma curve on the MASTER
// channel (preserves hue), plus a soft gray-world white balance on R/G/B. Every
// adjustment is applied partway and clamped, so it lifts a flat/cast video
// without overcooking. Videos already well-graded barely change.
export const autoCurvesFromHistogram = (hist: ChannelHistograms): Curves => {
    const CONTRAST_STRENGTH = 0.6; // how far to pull black/white toward the clip
    const GAMMA_STRENGTH = 0.3; // how far to nudge the midtone toward 0.5
    const WB_STRENGTH = 0.5; // how much of a colour cast to remove
    const WB_MIN = 0.86;
    const WB_MAX = 1.16;

    // --- Master: luma contrast + midtone lift ---
    const lum = channelLevels(hist.luma);
    const black = lum.black * CONTRAST_STRENGTH;
    const white = 1 - (1 - lum.white) * CONTRAST_STRENGTH;
    const span = Math.max(0.05, white - black);

    const master: CurvePoint[] = [{ x: 0, y: 0 }];
    if (black > 0.015) master.push({ x: Math.min(black, 0.4), y: 0 });

    const median = median01(hist.luma);
    if (median > 0.05 && median < 0.95) {
        const mapped = Math.min(1, Math.max(0, (median - black) / span));
        const targetY = mapped + (0.5 - mapped) * GAMMA_STRENGTH;
        const midX = median;
        const prevX = master[master.length - 1].x;
        if (
            midX > prevX + 0.04 &&
            midX < white - 0.04 &&
            Math.abs(targetY - mapped) > 0.015
        ) {
            master.push({
                x: midX,
                y: Math.min(0.97, Math.max(0.03, targetY)),
            });
        }
    }

    if (white < 0.985) master.push({ x: Math.max(white, 0.6), y: 1 });
    master.push({ x: 1, y: 1 });

    // --- Per-channel: gentle gray-world white balance (gain) ---
    const meanR = histogramMean(hist.r);
    const meanG = histogramMean(hist.g);
    const meanB = histogramMean(hist.b);
    const target = (meanR + meanG + meanB) / 3 || 0.5;

    const gain = (mean: number): number => {
        if (mean <= 0.02) return 1;
        const factor = 1 + WB_STRENGTH * (target / mean - 1);
        return Math.min(WB_MAX, Math.max(WB_MIN, factor));
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
        r: gainCurve(gain(meanR)),
        g: gainCurve(gain(meanG)),
        b: gainCurve(gain(meanB)),
    };
};

export const cloneCurves = (curves: Curves): Curves => ({
    rgb: curves.rgb.map((p) => ({ ...p })),
    r: curves.r.map((p) => ({ ...p })),
    g: curves.g.map((p) => ({ ...p })),
    b: curves.b.map((p) => ({ ...p })),
});
