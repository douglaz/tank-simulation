# Release Narrative: Scientific-Core Upgrade (v0.2 - v0.6)

## What this release is

This release completes the scientific-core overhaul that began after the v0.1
gap analysis (`docs/aquarium_sim_review_vnext.md`). The original simulator had
a solid architectural foundation — deterministic stepping, clean workspace
separation, first-class geometry — but its scientific core had five structural
weaknesses that limited its fidelity as an ecosystem model. This release
resolves all five and adds an accountability layer so the simulator can
demonstrate what it claims.

## What changed, and why it matters

### 1. Concentration-normalized kinetics (v0.2, tanksim-6e5.2.x)

**Problem**: All Monod half-saturation constants were stored as whole-tank
totals and divided by a hardcoded 20 L reference volume at runtime. A 100 L
tank with the same nutrient concentration appeared less limited than a 20 L
tank — the opposite of reality.

**Fix**: Every kinetic parameter now operates in concentration units (mg/L).
Half-saturation constants sit within published literature ranges. Tank volume
affects total mass pools but not the limitation dynamics.

**New behavior**: Two tanks with the same concentration but different volumes
now produce identical Monod limitation factors. Cycling timelines scale with
biofilter capacity, not tank size.

### 2. Closed mass loops (v0.2, tanksim-6e5.2.x + v0.5, tanksim-6e5.6.x)

**Problem**: Shrimp grazing and microfauna consumption deleted biomass without
returning waste products. The nitrogen and carbon budgets had silent leaks.

**Fix**: Shrimp now route consumed mass through assimilation, feces, dissolved
excretion (TAN + DOC), and respiration (O2 demand + DIC production). Budget
helpers audit mass conservation every tick. Trimming distinguishes "trim and
remove" (exports mass) from "trim and leave cuttings" (detritus input).

**New behavior**: Feeding rate directly affects TAN load through the excretion
pathway. Overfeeding produces measurable nutrient spikes rather than vanishing
into a sink.

### 3. Carbonate equilibrium and pH realism (v0.3, tanksim-6e5.4.x)

**Problem**: pH was computed from a log-linear shortcut that collapsed all
source waters to nearly the same value (~6.3). Different KH and DIC levels
produced no meaningful pH differentiation.

**Fix**: A closed-form quadratic carbonate equilibrium solver now derives
CO2(aq), HCO3-, CO3--, and pH from DIC, alkalinity, and temperature each
hour. pKa1 uses the temperature-corrected Harned & Davis (1943) formula.
Aeration strips CO2, raising pH. Plant photosynthesis consumes DIC during the
day, producing measurable diurnal pH swings.

**New behavior**: Hard shrimp water (KH 8) equilibrates near pH 8.3; RO-like
water near pH 6.7 — a 1.5 unit gap. Planted tanks show 0.7-0.9 unit day/night
pH swings. Aeration produces a 1.6 unit pH difference versus passive gas exchange.

### 4. Habitat-aware biofilm ecology (v0.4, tanksim-6e5.5.x)

**Problem**: Biofilter carrying capacity was a single fixed constant (0.5 g)
regardless of tank geometry, media volume, or available colonization surfaces.

**Fix**: A five-zone habitat registry (filter media, glass/hardscape, plant
surfaces, substrate surface, substrate deep) computes colonizable area from
geometry and derives flow, oxygen, and light exposure modifiers for each zone.
Nitrifier biomass scales with available attachment area. Denitrification occurs
in suboxic substrate zones with maturation dynamics.

**New behavior**: Bigger filters and porous substrates produce faster cycling.
Mature planted tanks develop measurable denitrification capacity (87 mg N
exported as N2 over 120 days in the validation scenario).

### 5. Stage-structured shrimp population (v0.5, tanksim-6e5.6.x)

**Problem**: Shrimp were a simple count with uniform biology. No life stages,
molt mechanics, or mineral requirements.

**Fix**: Three life stages (juvenile, sub-adult, adult) each carry per-stage
reserves and condition indices. Molt cycle depends on temperature and mineral
availability (Ca, Mg). Reproduction requires thermal window (22-28C),
adequate condition, and low toxicant exposure. Chloride protects against
nitrite toxicity.

**New behavior**: Cool-water tanks produce thriving colonies (260 population
after 60 days); warm tanks suppress reproduction entirely. Mineral-deficient
water triggers molt stress and population decline.

### 6. Scientific accountability layer (v0.6, tanksim-6e5.7.x)

**Problem**: No systematic way to verify the simulator's claims or track which
parameters were well-founded versus provisional.

**Fix**: Three interlocking subsystems:
- **Parameter provenance** (tanksim-6e5.7.2): Every high-value parameter
  carries a confidence tier (literature, expert, heuristic, placeholder) with
  source citation and valid range.
- **Validation scenarios** (tanksim-6e5.7.3): Eight curated scenarios grounded
  in aquarium science literature with expected envelopes and confidence ratings.
- **Calibration report workflow** (tanksim-6e5.7.4): Machine-readable reports
  comparing simulated output to target envelopes with pass/marginal/fail
  classification and cross-run comparison.

**New behavior**: `cargo run -p tank_harness --bin calibration_report` produces
a full calibration snapshot. Marginal findings (within 10% of envelope
boundaries) surface early warnings without blocking CI.

## Current calibration state

As of 2026-03-30 with default parameters:

- **8 scenarios**: 6 pass, 2 marginal, 0 fail
- **Marginal findings**: Both are boundary-detection artifacts (pH hitting the
  8.5 gameplay clamp; zero-bounded envelopes), not parameter tuning problems.
  Documented in `docs/traces/calibration_snapshot_v0.6.md`.
- **96+ tests** pass across all crates; zero clippy warnings; clean formatting.

## What the simulator does NOT yet claim

Transparency about scope is part of the scientific posture:

- **Not predictive**: The simulator reproduces directional plausibility, not
  numerical predictions of specific aquaria. Parameters are calibrated to
  literature envelopes, not fitted to time-series data from real tanks.
- **Well-mixed column**: No vertical stratification, thermal layering, or
  localized flow patterns.
- **Guild-based biology**: Plants, algae, and microbes are represented as
  functional guilds, not species-level populations.
- **Fixed atmospheric CO2**: No seasonal or ventilation-driven variation.
- **Simplified microfauna**: Protozoan/microfauna routing uses a simpler
  approximation than the full shrimp mass-routing model.
- **Provisional parameters remain**: Plant growth rates, process routing
  fractions, thermal transport coefficients, and several shrimp stress
  thresholds are heuristic-tier. See `docs/PROVENANCE_STATUS.md` for the
  complete inventory.

## Follow-on backlog candidates

Discovered during the rebalance pass, captured here for future roadmap planning:

1. **Envelope refinement for zero-bounded fields**: The 10% marginal heuristic
   produces false positives when applied to fields with a zero lower bound.
   Options: asymmetric threshold, absolute-distance fallback, or per-field
   override.

2. **pH clamp interaction with validation**: The gameplay pH clamp at 8.5
   creates a structural ceiling that interacts with envelope boundaries in
   high-buffer scenarios. Consider either raising the clamp (requires
   scientific justification for pH > 8.5 freshwater) or annotating it in
   scenario definitions.

3. **Plant preset provenance**: `fast_stem` and `root_rosette` growth rates
   and uptake biases lack `param_meta` annotations. These are load-bearing
   for the algae-plant competition scenario.

4. **Remaining source water annotations**: `soft_acidic.toml` and `ro_like.toml`
   do not carry `param_meta`. Lower priority because they represent extreme
   conditions, but completeness matters for the provenance story.

5. **Process routing fraction provenance**: Shrimp assimilation, respiration,
   and excretion splits (0.50/0.70/0.10/0.20) are heuristic-tier with no
   cited source. These directly affect nutrient loading predictions.

6. **Thermal transport provenance**: `k_surface_w_per_m2_k` (10.0) and
   `k_wall_w_per_m2_k` (5.0) are heuristic calibration fits. Important for
   the warm_room scenario's thermal equilibration story.

7. **Microfauna mass-routing parity**: Microfauna still use a simpler
   approximation than the closed shrimp routing model. Closing this gap would
   strengthen the mass-conservation guarantees.

8. **Substrate index provenance**: Detritus trapping indices, nutrient charges,
   and secondary habitat modifiers in substrate presets lack annotation.

9. **Equipment auto-scaling with geometry**: Filter flow, heater power, initial
   plant mass, and initial stock do not automatically scale when tank geometry
   scales. This limits the meaningfulness of geometry-scaling validation.

10. **Sensitivity analysis tooling**: With provenance tiers in place, the next
    logical step is automated sensitivity analysis targeting heuristic-tier
    parameters to quantify which ones most affect validation scenario outcomes.
