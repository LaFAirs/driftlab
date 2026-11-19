# Calibration mathematics

All formulas live in [`crates/driftlab-core/src/math.rs`](../crates/driftlab-core/src/math.rs) and are covered by unit and property tests (`proptest`).

## 1. Normalisation

| Raw type | Formula |
|---|---|
| `u8` (DS4/DualSense sticks) | `(v − 127.5) / 127.5`, clamped to `[−1, 1]` |
| `u16` | `(v − 32767.5) / 32767.5` |
| `i16` (gyro, XInput) | `v / 32767` |

PlayStation Y axes are down-positive and are flipped so that *up = +1*.

## 2. Centre estimation

For N samples per axis:

1. `m = median(x)`, `MAD = median(|x − m|)`.
2. Keep samples with `|x − m| ≤ k · s`, where `s = max(1.4826 · MAD, 10⁻³)` and `k = 3.5` (modified z-score; the floor lets a perfectly flat signal still reject spikes).
3. Centre = median of the kept samples; noise = population standard deviation of the kept samples.
4. **Stable** if `N ≥ 10` and `σx, σy < 0.01` (1 % of full travel). Moving sticks fail this check and the wizard restarts the round.

## 3. Drift metrics

* Offset radius `r = √(cx² + cy²)`
* Noise `σ = max(σx, σy)`
* Jitter `J = max(peak-to-peak(x), peak-to-peak(y))` after a wider (k = 5) outlier filter

### Drift score

```
score = 100 · clamp( 0.6·r/0.12 + 0.25·σ/0.02 + 0.15·J/0.08 , 0, 1 )
```

Rationale: a centre offset is the most visible symptom in games (weight 0.6, saturates at 12 % – well beyond any usual deadzone); noise (0.25, saturates at 2 %) and jitter (0.15, saturates at 8 %) describe an unstable potentiometer.

### Classification

| Verdict | Condition |
|---|---|
| Hardware wear likely | `σ ≥ 0.02` **or** `J ≥ 0.08` **or** `score ≥ 75` |
| Strong drift | `score ≥ 40` |
| Slight drift | `score ≥ 12` |
| OK | otherwise |

A healthy potentiometer is stable: noise above 2 % or peak-to-peak jitter above 8 % cannot be removed by recentring, which is why those two conditions override the score. The thresholds are heuristics chosen to be conservative about claiming "hardware wear"; they are not manufacturer specifications.

## 4. Range / edge estimation

The plane is divided into 72 angular segments of 5°. For every sample with `r ≥ 0.3` the maximum radius per segment is kept (`EdgeMap`). Coverage is the fraction of segments with `r ≥ 0.5`; the wizard continues at **≥ 95 %**. A 3-tap circular filter `0.25·a + 0.5·b + 0.25·c` (never below `0.9·b`) smooths the map.

## 5. Circularity

For smoothed radii `rᵢ` of non-empty segments: `eᵢ = |rᵢ − 1| · 100 %`. Mean error = mean(eᵢ), maximum error = max(eᵢ), weakest direction = argmax(eᵢ). The software circularity gain is `1 / mean(rᵢ)`, clamped to `[0.9, 1.3]`.

## 6. Deadzones, outer zone, anti-deadzone and curves

With `d` = inner deadzone, `o` = outer deadzone, `a` = anti-deadzone and `c(t)` the response curve on `[0, 1]`:

* **Scaled radial** (default): `r = |p − centre|`; for `r > d`, `t = (r − d)/((1 − o) − d)` (clamped), output magnitude `a + (1 − a)·c(t)` along the original direction; otherwise 0. With `o = a = 0` and a linear curve this is `out = ((r − d)/(1 − d))·dir`.
* **Radial**: pass through above `d`, no rescale below the outer zone.
* **Axial**: the scaled mapping applied per axis.
* **Bowtie**: dominant axis uses `0.5·d`, the minor axis `1.5·d`.

Curves: linear; exponential `t^γ` (γ ∈ [0.1, 8]); S-curve `t + (smoothstep(t) − t)·s`; cubic Bézier through (0,0), (x₁,y₁), (x₂,y₂), (1,1) with x₁,x₂ ∈ [0,1] (x(u) is monotone, solved by bisection).

All outputs are clamped to `[−1, 1]`, NaN/∞ inputs are treated as 0.

### Property tests

* output always in `[−1, 1]`, never NaN, for random parameters and inputs (including NaN input)
* curves are monotone and bounded
* continuity of the scaled-radial mapping at the deadzone boundary
* drift score bounded to `[0, 100]`

## 7. Automatic deadzone

```
dz = clamp( r + 3σ + J/2 , 0, 0.35 )   rounded up to the next 0.5 %
```

The centre is compensated separately in the wizard, so there only `3σ + J/2` remains (residual noise).

## 8. Trigger statistics

Rest = minimum, maximum = maximum, linearity = `1 − KS`, where KS is the Kolmogorov–Smirnov distance between the empirical distribution of normalised samples and the uniform distribution – a slow, even press covers the travel uniformly.
