//! Calibration mathematics. All formulas are documented in `docs/CALIBRATION_MATH.md`.

use serde::{Deserialize, Serialize};

/// Normalise an unsigned 8-bit axis (0..=255, centre ≈ 127.5) to `[-1, 1]`.
pub fn norm_u8(v: u8) -> f32 {
    ((f32::from(v) - 127.5) / 127.5).clamp(-1.0, 1.0)
}

/// Normalise an unsigned 16-bit axis (0..=65535) to `[-1, 1]`.
pub fn norm_u16(v: u16) -> f32 {
    ((f32::from(v) - 32767.5) / 32767.5).clamp(-1.0, 1.0)
}

/// Normalise a signed 16-bit axis to `[-1, 1]`.
pub fn norm_i16(v: i16) -> f32 {
    (f32::from(v) / 32767.0).clamp(-1.0, 1.0)
}

fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// Median of a slice (NaNs are dropped). Returns 0 for empty input.
pub fn median(data: &[f32]) -> f32 {
    let mut v: Vec<f32> = data.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.total_cmp(b));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

/// Median absolute deviation.
pub fn mad(data: &[f32]) -> f32 {
    let m = median(data);
    let dev: Vec<f32> = data.iter().map(|x| (x - m).abs()).collect();
    median(&dev)
}

/// Remove outliers with a modified z-score above `k` (default 3.5 → `k * 1.4826 * MAD`).
pub fn filter_outliers(data: &[f32], k: f32) -> Vec<f32> {
    let m = median(data);
    // Floor the scale so a perfectly flat signal (MAD = 0) still rejects obvious spikes.
    let s = (mad(data) * 1.4826).max(1e-3);
    data.iter().copied().filter(|x| x.is_finite() && (x - m).abs() <= k * s).collect()
}

/// Population standard deviation.
pub fn std_dev(data: &[f32]) -> f32 {
    if data.len() < 2 {
        return 0.0;
    }
    let n = data.len() as f32;
    let mean = data.iter().sum::<f32>() / n;
    finite((data.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n).sqrt())
}

/// Peak-to-peak range.
pub fn peak_to_peak(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let (mn, mx) = data.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    mx - mn
}

/// Result of a centre estimation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CenterEstimate {
    pub x: f32,
    pub y: f32,
    pub std_x: f32,
    pub std_y: f32,
    pub samples_used: usize,
    pub stable: bool,
}

/// Maximum jitter (std-dev in normalised units) for a centre capture to count as stable.
pub const STABLE_STD: f32 = 0.01;

/// Estimate the stick centre: MAD-filtered median over `samples`.
pub fn estimate_center(samples: &[(f32, f32)]) -> CenterEstimate {
    let xs: Vec<f32> = samples.iter().map(|s| s.0).collect();
    let ys: Vec<f32> = samples.iter().map(|s| s.1).collect();
    let fx = filter_outliers(&xs, 3.5);
    let fy = filter_outliers(&ys, 3.5);
    let (sx, sy) = (std_dev(&fx), std_dev(&fy));
    CenterEstimate {
        x: median(&fx),
        y: median(&fy),
        std_x: sx,
        std_y: sy,
        samples_used: fx.len().min(fy.len()),
        stable: samples.len() >= 10 && sx < STABLE_STD && sy < STABLE_STD,
    }
}

/// Verdict of a drift test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriftVerdict {
    Ok,
    Slight,
    Strong,
    HardwareWear,
}

/// Drift test result for one stick.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DriftResult {
    pub offset_x: f32,
    pub offset_y: f32,
    pub offset_radius: f32,
    pub noise: f32,
    pub jitter: f32,
    pub score: f32,
    pub verdict: DriftVerdict,
}

/// Compute the drift score (0–100) from offset radius, noise and jitter.
///
/// `score = 100 * clamp(0.6*off/0.12 + 0.25*noise/0.02 + 0.15*jitter/0.08, 0, 1)`
pub fn drift_score(offset_radius: f32, noise: f32, jitter: f32) -> f32 {
    let s = 0.6 * (offset_radius / 0.12) + 0.25 * (noise / 0.02) + 0.15 * (jitter / 0.08);
    (100.0 * finite(s)).clamp(0.0, 100.0)
}

/// Noise (std-dev) above which we suspect worn hardware.
pub const WEAR_NOISE: f32 = 0.02;
/// Jitter (peak-to-peak) above which we suspect worn hardware.
pub const WEAR_JITTER: f32 = 0.08;

/// Classify a drift measurement.
pub fn classify(score: f32, noise: f32, jitter: f32) -> DriftVerdict {
    if noise >= WEAR_NOISE || jitter >= WEAR_JITTER || score >= 75.0 {
        DriftVerdict::HardwareWear
    } else if score >= 40.0 {
        DriftVerdict::Strong
    } else if score >= 12.0 {
        DriftVerdict::Slight
    } else {
        DriftVerdict::Ok
    }
}

/// Analyse resting samples of one stick.
pub fn analyze_drift(samples: &[(f32, f32)]) -> DriftResult {
    let c = estimate_center(samples);
    let noise = c.std_x.max(c.std_y);
    let xs: Vec<f32> = samples.iter().map(|s| s.0).collect();
    let ys: Vec<f32> = samples.iter().map(|s| s.1).collect();
    let jitter = peak_to_peak(&filter_outliers(&xs, 5.0)).max(peak_to_peak(&filter_outliers(&ys, 5.0)));
    let r = (c.x * c.x + c.y * c.y).sqrt();
    let score = drift_score(r, noise, jitter);
    DriftResult { offset_x: c.x, offset_y: c.y, offset_radius: r, noise, jitter, score, verdict: classify(score, noise, jitter) }
}

/// Number of angular segments for edge / circularity estimation.
pub const SEGMENTS: usize = 72;

/// Per-segment maximum radius (edge map).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeMap {
    pub radius: Vec<f32>,
}

impl Default for EdgeMap {
    fn default() -> Self {
        Self { radius: vec![0.0; SEGMENTS] }
    }
}

fn segment_of(x: f32, y: f32) -> usize {
    let a = y.atan2(x).rem_euclid(std::f32::consts::TAU);
    (((a / std::f32::consts::TAU) * SEGMENTS as f32) as usize).min(SEGMENTS - 1)
}

impl EdgeMap {
    /// Record a sample (relative to centre) — only keeps the maximum per segment.
    pub fn add(&mut self, x: f32, y: f32) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        let r = (x * x + y * y).sqrt();
        if r < 0.3 {
            return;
        }
        let s = segment_of(x, y);
        self.radius[s] = self.radius[s].max(r);
    }

    /// Fraction of segments with a plausible edge sample (`r >= 0.5`).
    pub fn coverage(&self) -> f32 {
        self.radius.iter().filter(|&&r| r >= 0.5).count() as f32 / SEGMENTS as f32
    }

    /// 3-tap circular smoothing of the segment radii (zero segments are left alone).
    pub fn smoothed(&self) -> Vec<f32> {
        let n = SEGMENTS;
        (0..n)
            .map(|i| {
                let (a, b, c) = (self.radius[(i + n - 1) % n], self.radius[i], self.radius[(i + 1) % n]);
                if b <= 0.0 {
                    0.0
                } else {
                    (a.max(0.0) * 0.25 + b * 0.5 + c.max(0.0) * 0.25).max(b * 0.9)
                }
            })
            .collect()
    }
}

/// Circularity analysis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Circularity {
    pub mean_error_pct: f32,
    pub max_error_pct: f32,
    pub weakest_segment: usize,
}

/// Mean / max deviation of the edge map from the unit circle, in percent.
pub fn circularity(map: &EdgeMap) -> Circularity {
    let sm = map.smoothed();
    let mut sum = 0.0;
    let mut n = 0.0;
    let mut max = 0.0f32;
    let mut weakest = 0;
    for (i, &r) in sm.iter().enumerate() {
        if r <= 0.0 {
            continue;
        }
        let e = (r - 1.0).abs() * 100.0;
        sum += e;
        n += 1.0;
        if e > max {
            max = e;
            weakest = i;
        }
    }
    Circularity { mean_error_pct: if n > 0.0 { sum / n } else { 0.0 }, max_error_pct: max, weakest_segment: weakest }
}

/// Deadzone shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeadzoneKind {
    Radial,
    Axial,
    ScaledRadial,
    Bowtie,
}

/// Response curve.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    Linear,
    /// `out = in^gamma`, gamma > 0.
    Exponential(f32),
    /// Smoothstep blend, `strength` in `[0, 1]`.
    SCurve(f32),
    /// Cubic Bézier through (0,0), (x1,y1), (x2,y2), (1,1); x1,x2 in `[0,1]`.
    Bezier {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
}

impl Curve {
    /// Evaluate on `t ∈ [0,1]`; always returns a value in `[0,1]` and is monotone for valid parameters.
    pub fn eval(&self, t: f32) -> f32 {
        let t = finite(t).clamp(0.0, 1.0);
        let y = match *self {
            Curve::Linear => t,
            Curve::Exponential(g) => t.powf(g.clamp(0.1, 8.0)),
            Curve::SCurve(s) => {
                let s = s.clamp(0.0, 1.0);
                let sm = t * t * (3.0 - 2.0 * t);
                t + (sm - t) * s
            }
            Curve::Bezier { x1, y1, x2, y2 } => {
                let (x1, x2) = (x1.clamp(0.0, 1.0), x2.clamp(0.0, 1.0));
                let (y1, y2) = (y1.clamp(0.0, 1.0), y2.clamp(0.0, 1.0));
                // Solve x(u)=t by bisection (x is monotone for x1,x2 in [0,1]).
                let bez = |a: f32, b: f32, u: f32| {
                    let v = 1.0 - u;
                    3.0 * v * v * u * a + 3.0 * v * u * u * b + u * u * u
                };
                let (mut lo, mut hi) = (0.0f32, 1.0f32);
                for _ in 0..32 {
                    let mid = (lo + hi) / 2.0;
                    if bez(x1, x2, mid) < t {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                bez(y1, y2, (lo + hi) / 2.0)
            }
        };
        finite(y).clamp(0.0, 1.0)
    }
}

/// Complete software-correction configuration for one stick.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StickCorrection {
    pub center_x: f32,
    pub center_y: f32,
    pub kind: DeadzoneKind,
    pub inner_deadzone: f32,
    pub outer_deadzone: f32,
    pub anti_deadzone: f32,
    pub curve: Curve,
    /// Radial scale per circularity correction (1.0 = off).
    pub circularity_gain: f32,
}

impl Default for StickCorrection {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            kind: DeadzoneKind::ScaledRadial,
            inner_deadzone: 0.05,
            outer_deadzone: 0.0,
            anti_deadzone: 0.0,
            curve: Curve::Linear,
            circularity_gain: 1.0,
        }
    }
}

impl StickCorrection {
    /// Apply the correction to a raw normalised stick position. Output is in `[-1, 1]` per axis, never NaN.
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let (x, y) = (finite(x) - self.center_x, finite(y) - self.center_y);
        let dz = self.inner_deadzone.clamp(0.0, 0.95);
        let od = self.outer_deadzone.clamp(0.0, 0.95 - dz);
        let anti = self.anti_deadzone.clamp(0.0, 0.95);
        let map = |m: f32| -> f32 {
            // m in [0,∞): scaled deadzone → outer → curve → anti-deadzone
            if m <= dz {
                return 0.0;
            }
            let hi = 1.0 - od;
            let t = ((m - dz) / (hi - dz).max(1e-6)).clamp(0.0, 1.0);
            let c = self.curve.eval(t);
            (anti + (1.0 - anti) * c).clamp(0.0, 1.0)
        };
        let out = match self.kind {
            DeadzoneKind::ScaledRadial | DeadzoneKind::Radial => {
                let r = (x * x + y * y).sqrt();
                if r <= f32::EPSILON {
                    (0.0, 0.0)
                } else {
                    let m = if self.kind == DeadzoneKind::Radial {
                        // Unscaled: pass through r above deadzone, only cut inside.
                        if r <= dz {
                            0.0
                        } else {
                            self.curve.eval(r.min(1.0 - od) / (1.0 - od).max(1e-6))
                        }
                    } else {
                        map(r)
                    };
                    (x / r * m, y / r * m)
                }
            }
            DeadzoneKind::Axial => (x.signum() * map(x.abs()), y.signum() * map(y.abs())),
            DeadzoneKind::Bowtie => {
                // Narrow deadzone on the dominant axis, wide on the minor axis.
                let (ax, ay) = (x.abs(), y.abs());
                let ox =
                    if ay > ax { map_with(ax, dz * 1.5, od, anti, &self.curve) } else { map_with(ax, dz * 0.5, od, anti, &self.curve) };
                let oy =
                    if ax > ay { map_with(ay, dz * 1.5, od, anti, &self.curve) } else { map_with(ay, dz * 0.5, od, anti, &self.curve) };
                (x.signum() * ox, y.signum() * oy)
            }
        };
        let g = finite(self.circularity_gain).clamp(0.5, 2.0);
        (finite(out.0 * g).clamp(-1.0, 1.0), finite(out.1 * g).clamp(-1.0, 1.0))
    }
}

fn map_with(m: f32, dz: f32, od: f32, anti: f32, curve: &Curve) -> f32 {
    let dz = dz.clamp(0.0, 0.95);
    if m <= dz {
        return 0.0;
    }
    let hi = 1.0 - od;
    let t = ((m - dz) / (hi - dz).max(1e-6)).clamp(0.0, 1.0);
    (anti + (1.0 - anti) * curve.eval(t)).clamp(0.0, 1.0)
}

/// Smallest inner deadzone that suppresses the measured drift with a safety margin.
///
/// `dz = clamp(offset_radius + 3*noise + jitter/2, 0, 0.35)`, rounded up to 0.5 %.
pub fn auto_deadzone(d: &DriftResult) -> f32 {
    let raw = d.offset_radius + 3.0 * d.noise + d.jitter / 2.0;
    ((raw.clamp(0.0, 0.35) * 200.0).ceil() / 200.0).clamp(0.0, 0.35)
}

/// Trigger statistics from a slow full press/release.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TriggerStats {
    pub rest: f32,
    pub max: f32,
    /// 1.0 = perfectly even coverage of the travel (empirical CDF vs uniform), 0 = very uneven.
    pub linearity: f32,
}

/// Rest = minimum, max = maximum; linearity = `1 - KS(F_emp, uniform)` over the normalised values.
pub fn trigger_stats(samples: &[f32]) -> TriggerStats {
    let v: Vec<f32> = samples.iter().copied().filter(|x| x.is_finite()).collect();
    if v.len() < 2 {
        return TriggerStats { rest: 0.0, max: 0.0, linearity: 0.0 };
    }
    let (mn, mx) = v.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    if mx - mn < 0.2 {
        return TriggerStats { rest: mn, max: mx, linearity: 0.0 };
    }
    let mut s: Vec<f32> = v.iter().map(|x| (x - mn) / (mx - mn)).collect();
    s.sort_by(|a, b| a.total_cmp(b));
    let n = s.len() as f32;
    let ks = s.iter().enumerate().map(|(i, &x)| ((i as f32 + 1.0) / n - x).abs().max((i as f32 / n - x).abs())).fold(0.0f32, f32::max);
    TriggerStats { rest: mn, max: mx, linearity: (1.0 - ks).clamp(0.0, 1.0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn trigger_linearity() {
        let even: Vec<f32> = (0..200).map(|i| i as f32 / 199.0).collect();
        let s = trigger_stats(&even);
        assert!(s.linearity > 0.95 && s.rest == 0.0 && (s.max - 1.0).abs() < 1e-6);
        let uneven: Vec<f32> = (0..200).map(|i| (i as f32 / 199.0).powi(4)).collect();
        assert!(trigger_stats(&uneven).linearity < s.linearity);
        assert_eq!(trigger_stats(&[0.5, 0.5]).linearity, 0.0);
    }

    #[test]
    fn normalisation_bounds() {
        assert!((norm_u8(0) + 1.0).abs() < 1e-6);
        assert!((norm_u8(255) - 1.0).abs() < 1e-6);
        assert!(norm_u8(128).abs() < 0.01);
        assert!((norm_u16(65535) - 1.0).abs() < 1e-6);
        assert!((norm_i16(i16::MIN) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn median_and_mad() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
        assert_eq!(median(&[]), 0.0);
        assert_eq!(mad(&[1.0, 1.0, 2.0, 2.0, 4.0, 6.0, 9.0]), 1.0);
    }

    #[test]
    fn outliers_removed() {
        let mut d = vec![0.01f32; 50];
        d.push(0.9);
        let f = filter_outliers(&d, 3.5);
        assert!(f.iter().all(|&x| x < 0.5));
    }

    #[test]
    fn center_estimate_finds_offset() {
        let s: Vec<(f32, f32)> = (0..100).map(|i| (0.05 + (i % 3) as f32 * 0.0005, -0.03)).collect();
        let c = estimate_center(&s);
        assert!((c.x - 0.05).abs() < 0.002 && (c.y + 0.03).abs() < 0.002);
        assert!(c.stable);
    }

    #[test]
    fn unstable_when_moving() {
        let s: Vec<(f32, f32)> = (0..100).map(|i| ((i as f32 / 10.0).sin() * 0.3, 0.0)).collect();
        assert!(!estimate_center(&s).stable);
    }

    #[test]
    fn drift_classification() {
        let clean: Vec<(f32, f32)> = vec![(0.0, 0.0); 100];
        assert_eq!(analyze_drift(&clean).verdict, DriftVerdict::Ok);
        let drifty: Vec<(f32, f32)> = vec![(0.1, 0.05); 100];
        let r = analyze_drift(&drifty);
        assert!(r.score > 40.0);
        let noisy: Vec<(f32, f32)> = (0..100).map(|i| (if i % 2 == 0 { 0.05 } else { -0.05 }, 0.0)).collect();
        assert_eq!(analyze_drift(&noisy).verdict, DriftVerdict::HardwareWear);
    }

    #[test]
    fn edge_map_coverage_and_circularity() {
        let mut m = EdgeMap::default();
        for i in 0..720 {
            let a = i as f32 / 720.0 * std::f32::consts::TAU;
            m.add(a.cos(), a.sin());
        }
        assert!(m.coverage() > 0.99);
        assert!(circularity(&m).max_error_pct < 5.0);
        let mut sq = EdgeMap::default();
        for i in 0..720 {
            let a = i as f32 / 720.0 * std::f32::consts::TAU;
            let k = a.cos().abs().max(a.sin().abs());
            sq.add(a.cos() / k, a.sin() / k);
        }
        assert!(circularity(&sq).max_error_pct > 20.0);
    }

    #[test]
    fn scaled_radial_formula() {
        let c = StickCorrection { inner_deadzone: 0.2, ..Default::default() };
        assert_eq!(c.apply(0.1, 0.0), (0.0, 0.0));
        let (x, _) = c.apply(0.6, 0.0);
        assert!((x - 0.5).abs() < 1e-5);
        let (x, _) = c.apply(1.0, 0.0);
        assert!((x - 1.0).abs() < 1e-5);
    }

    #[test]
    fn auto_deadzone_covers_drift() {
        let d = analyze_drift(&vec![(0.08, 0.0); 100]);
        assert!(auto_deadzone(&d) >= 0.08);
    }

    fn arb_curve() -> impl Strategy<Value = Curve> {
        prop_oneof![
            Just(Curve::Linear),
            (0.1f32..8.0).prop_map(Curve::Exponential),
            (0.0f32..1.0).prop_map(Curve::SCurve),
            (0.0f32..1.0, 0.0f32..1.0, 0.0f32..1.0, 0.0f32..1.0).prop_map(|(x1, y1, x2, y2)| Curve::Bezier { x1, y1, x2, y2 }),
        ]
    }

    fn arb_kind() -> impl Strategy<Value = DeadzoneKind> {
        prop_oneof![Just(DeadzoneKind::Radial), Just(DeadzoneKind::Axial), Just(DeadzoneKind::ScaledRadial), Just(DeadzoneKind::Bowtie)]
    }

    proptest! {
        #[test]
        fn output_in_range_no_nan(
            x in -2.0f32..2.0, y in -2.0f32..2.0,
            dz in 0.0f32..0.9, od in 0.0f32..0.5, anti in 0.0f32..0.9,
            cx in -0.2f32..0.2, cy in -0.2f32..0.2,
            kind in arb_kind(), curve in arb_curve(), gain in 0.5f32..2.0,
        ) {
            let c = StickCorrection { center_x: cx, center_y: cy, kind, inner_deadzone: dz, outer_deadzone: od, anti_deadzone: anti, curve, circularity_gain: gain };
            let (ox, oy) = c.apply(x, y);
            prop_assert!(ox.is_finite() && oy.is_finite());
            prop_assert!((-1.0..=1.0).contains(&ox) && (-1.0..=1.0).contains(&oy));
        }

        #[test]
        fn nan_input_safe(kind in arb_kind()) {
            let c = StickCorrection { kind, ..Default::default() };
            let (ox, oy) = c.apply(f32::NAN, f32::INFINITY);
            prop_assert!(ox.is_finite() && oy.is_finite());
        }

        #[test]
        fn curves_monotone(curve in arb_curve(), a in 0.0f32..1.0, b in 0.0f32..1.0) {
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            prop_assert!(curve.eval(lo) <= curve.eval(hi) + 1e-4);
            prop_assert!((0.0..=1.0).contains(&curve.eval(a)));
        }

        #[test]
        fn scaled_radial_continuous_at_deadzone(dz in 0.01f32..0.9, ang in 0.0f32..std::f32::consts::TAU) {
            let c = StickCorrection { inner_deadzone: dz, ..Default::default() };
            let (x, y) = c.apply((dz + 1e-3) * ang.cos(), (dz + 1e-3) * ang.sin());
            prop_assert!((x * x + y * y).sqrt() < 0.05 + 2e-3 / (1.0 - dz));
        }

        #[test]
        fn score_bounded(o in 0.0f32..2.0, n in 0.0f32..2.0, j in 0.0f32..2.0) {
            let s = drift_score(o, n, j);
            prop_assert!((0.0..=100.0).contains(&s));
        }
    }
}
