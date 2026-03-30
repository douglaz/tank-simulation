# Validation Scenarios

Scientific accountability reference for the tank simulator. Each scenario
defines a qualitative expectation grounded in aquarium science literature or
established chemistry/biology principles, an acceptable envelope, and a
confidence rating.

**Purpose**: Provide a curated set of causal stories the simulator must
reproduce. Success is *directional plausibility*, not numerical prediction
of every real aquarium.

**Usage**: Run the validation suite with
`cargo test --test e2e_validation_suite -- --nocapture` to get a pass/fail
summary for each scenario. The calibration workflow (G4) can auto-populate
the report template at the end of this document.

---

## Confidence tiers

| Tier | Meaning | Implication |
|------|---------|-------------|
| **high** | Directionally validated — supported by peer-reviewed literature or stoichiometric necessity | Envelope violations are treated as model bugs |
| **medium** | Expert-consensus — widely accepted in the hobby/aquaculture but not from controlled studies | Envelope violations warrant investigation; tuning may shift bounds |
| **low** | Heuristic — calibrated to produce plausible behavior; limited empirical grounding | Envelope is indicative; may be revised as better data emerges |

---

## Scenario 1: Fishless cycling timeline

### Rationale

Fishless cycling establishes nitrifying bacteria by dosing ammonia in an
uninhabited tank. In well-buffered water at 20–28 °C, ammonia-oxidizing
bacteria (AOB) colonize first, converting NH₃/NH₄⁺ to NO₂⁻, followed by
nitrite-oxidizing bacteria (NOB) converting NO₂⁻ to NO₃⁻. The classic
succession produces a TAN peak (weeks 1–3), a nitrite peak (weeks 2–5),
and eventual clearing of both as the biofilter matures.

### Literature

- Hovanec, T.A. & DeLong, E.F. (1996). Comparative analysis of nitrifying
  bacteria associated with freshwater and marine aquaria.
  *Applied and Environmental Microbiology*, 62(8), 2888–2896.
- Timmons, M.B. & Ebeling, J.M. (2010). *Recirculating Aquaculture*, 2nd ed.
  Cayuga Aqua Ventures. Chapter 9: Biofiltration.
- Bower, C.E. & Turner, D.T. (1981). Accelerated nitrification in new
  seawater culture systems. *Aquaculture*, 24, 1–9.

### Expected qualitative outcome

In a moderately buffered tank (KH ≥ 4 dKH) at 24–26 °C with daily ammonia
dosing (~2 mg N/L/day equivalent):

1. TAN rises during weeks 1–2 as AOB lag.
2. TAN peaks and begins declining by weeks 2–4 as AOB biomass grows.
3. Nitrite appears as an intermediate, peaking after TAN begins to fall.
4. By weeks 4–8, both TAN and NO₂⁻ are near zero; NO₃⁻ accumulates.
5. Biofilter maturity index reaches ≥ 0.7 by week 6.

### Confidence: **high**

The nitrogen oxidation sequence is among the best-studied processes in
aquatic microbiology. The 4–8 week timeline is consistent across dozens of
studies and aquaculture texts.

### Envelope bounds

Note: The model's biofilter maturity index tracks a slow ramp (60-day
denitrifier-like scale) that does not directly correspond to nitrification
capacity. Cycling completion is validated by nitrification output (low TAN,
rising NO₃⁻) rather than the maturity index.

| Checkpoint | TAN (mg N/L) | NO₂⁻ (mg N/L) | NO₃⁻ (mg N/L) | pH |
|------------|-------------|---------------|---------------|-----|
| Week 2 | 0.0 – 15.0 | 0.0 – 5.0 | 0.0 – 15.0 | 6.5 – 8.5 |
| Week 4 | 0.0 – 10.0 | 0.0 – 10.0 | 0.5 – 25.0 | 6.5 – 8.5 |
| Week 6 | 0.0 – 3.0 | 0.0 – 3.0 | 2.0 – 40.0 | 6.5 – 8.5 |
| Week 8 | 0.0 – 2.0 | 0.0 – 2.0 | 5.0 – 60.0 | 6.5 – 8.5 |

### Test cross-reference

`e2e_validation_suite::vs01_cycling_timeline`

---

## Scenario 2: Aeration effects on dissolved oxygen and pH

### Rationale

Mechanical aeration increases the gas-liquid transfer coefficient (KLA),
accelerating both O₂ dissolution and CO₂ stripping. An aerated tank should
therefore show (a) higher dissolved oxygen, especially when DO is below
saturation, and (b) higher pH due to CO₂ removal shifting the carbonate
equilibrium toward bicarbonate.

### Literature

- Colt, J. (2006). Water quality requirements for reuse systems.
  *Aquacultural Engineering*, 34(3), 143–156.
- Boyd, C.E. & Tucker, C.S. (1998). *Pond Aquaculture Water Quality
  Management*. Springer. Chapters 5–6.
- Summerfelt, S.T. et al. (2000). Controlled-atmosphere stripping columns
  for CO₂ and TSS removal. *Aquacultural Engineering*, 22, 57–69.

### Expected qualitative outcome

Two identical tanks with biological oxygen demand (moderate bioload, active
nitrification). One receives strong aeration; the other is passive (surface
exchange only). After 24 hours:

1. Aerated tank has higher DO than passive tank.
2. Aerated tank has higher pH than passive tank (CO₂ stripping).
3. DO difference ≥ 0.5 mg/L; pH difference ≥ 0.2 units.

### Confidence: **high**

Gas transfer theory (KLA-driven reaeration) is well-established in
environmental engineering. The direction of the effect is physically
necessary; only the magnitude depends on model parameters.

### Envelope bounds

The scenario starts with depressed DO (3 mg/L) and elevated DIC (40 mg C/L)
with reduced surface reaeration so the aeration hardware is the dominant
gas-transfer pathway.

| Metric | Direction |
|--------|-----------|
| DO | aerated > passive |
| pH | aerated > passive |

### Test cross-reference

`e2e_validation_suite::vs02_aeration_effects` (DO + pH combined),
`e2e_carbonate_probes::probe_2_aeration_driven_co2_stripping` (pH only)

---

## Scenario 3: Planted tank day/night pH swing

### Rationale

In a densely planted tank, photosynthesis during lit hours consumes dissolved
CO₂, reducing DIC and raising pH. At night, plant and animal respiration
releases CO₂, increasing DIC and lowering pH. The resulting diurnal pH swing
is a well-documented phenomenon in planted aquaria.

### Literature

- Brewer, P.G. & Goldman, J.C. (1976). Alkalinity changes generated by
  phytoplankton growth. *Limnology and Oceanography*, 21(1), 108–117.
- Wetzel, R.G. (2001). *Limnology: Lake and River Ecosystems*, 3rd ed.
  Academic Press. Chapter 11: CO₂ and pH dynamics.
- Stumm, W. & Morgan, J.J. (1996). *Aquatic Chemistry*, 3rd ed.
  Wiley-Interscience. Chapter 4: Carbonate system.

### Expected qualitative outcome

A 30 L tank with 25 g total plant biomass, 12h/12h photoperiod, moderate
DIC buffering, and no mechanical aeration:

1. End-of-light pH exceeds end-of-dark pH every cycle.
2. pH swing amplitude (max light pH − min dark pH) is 0.2–1.0 units.
3. Swing is larger in low-KH water (less buffering).

### Confidence: **high**

The photosynthesis/respiration CO₂ flux is a direct consequence of plant
metabolism and the carbonate equilibrium. The 0.2–1.0 range encompasses
lightly to heavily planted tanks in typical hobby conditions.

### Envelope bounds

| Metric | Bound | Justification |
|--------|-------|---------------|
| Light pH > dark pH | every cycle from day 2 onward | photosynthetic CO₂ draw is obligate |
| Swing amplitude | 0.1 – 2.0 units | 0.1 lower bound is conservative for 25 g in 30 L; 2.0 upper bound allows extreme soft water |

### Test cross-reference

`e2e_validation_suite::vs03_day_night_ph_swing`,
`e2e_carbonate_probes::probe_3_day_night_ph_drift`

---

## Scenario 4: Source-water differentiation (RO vs hard tap)

### Rationale

Water with near-zero alkalinity and DIC (reverse-osmosis permeate) produces
a fundamentally different carbonate equilibrium than water with substantial
alkalinity and DIC (hard tap water). This difference drives a pH gap of
at least 1.0 unit under identical biological conditions.

### Literature

- Standard carbonate chemistry (Stumm & Morgan 1996; Snoeyink & Jenkins 1980).
- Practical aquarium context: Walstad, D. (2003). *Ecology of the Planted
  Aquarium*, 2nd ed. Echinodorus Publishing. Chapter 5: Water chemistry.

### Expected qualitative outcome

Two biology-free equilibrium tanks with identical geometry:

1. Hard shrimp water (KH ~8.0, DIC ~36 mg C/L): pH 7.0–8.5.
2. RO-like water (KH ~0.1, DIC ~1.0 mg C/L): pH 5.5–7.0.
3. pH gap ≥ 1.0 unit.

### Confidence: **high**

This is a direct consequence of carbonate equilibrium thermodynamics. The
direction and approximate magnitude are analytically derivable.

### Envelope bounds

| Water type | pH range | Justification |
|------------|----------|---------------|
| Hard shrimp (KH 8) | 7.0 – 8.5 | Analytical carbonate equilibrium at KH 8 / DIC 36 |
| RO-like (KH 0.1) | 5.5 – 7.0 | Near solver floor with minimal buffering |
| Gap | ≥ 1.0 | 80× KH difference drives ≥ 1 pH unit separation |

### Test cross-reference

`e2e_validation_suite::vs04_source_water_differentiation`,
`e2e_carbonate_probes::probe_1_source_water_ph_differentiation`

---

## Scenario 5: Neocaridina breeding thermal window

### Rationale

*Neocaridina davidi* breeds readily at 22–28 °C but shows reproductive
suppression above 30 °C. At elevated temperature, egg development
accelerates but spawning frequency drops, egg viability declines, and
egg-dropping events increase. Below ~18 °C, metabolic rates slow and
reproduction is sluggish.

### Literature

- Tropea, C., Stumpf, L., & López Greco, L.S. (2015). Effect of temperature
  on biochemical composition, growth and reproduction of the ornamental red
  cherry shrimp *Neocaridina heteropoda heteropoda*.
  *PLOS ONE*, 10(3), e0119468.
- Pantaleão, J.A.F., Barros-Alves, S.P., et al. (2015). Reproductive
  performance of the ornamental shrimp *Neocaridina davidi* under different
  temperatures. *Aquaculture Research*, 48(12), 6124–6132.
- Anger, K. (2001). *The Biology of Decapod Crustacean Larvae*. Crustacean
  Issues, 14. Balkema. (General crustacean thermal biology.)

### Expected qualitative outcome

Two identical colonies of 10 adult *Neocaridina* in well-maintained tanks:

1. **Cool arm (25 °C)**: population grows over 60 days; at least 2 complete
   berried → hatch cycles; juveniles and sub-adults present.
2. **Warm arm (31 °C)**: reproductive readiness index depressed; fewer
   juveniles than cool arm; egg-dropping events observed after heat shock.

### Confidence: **high**

The thermal window for Neocaridina reproduction is well-documented. The
direction of the effect (suppression above 30 °C) is consistent across
multiple studies.

### Envelope bounds

| Metric | Cool (25 °C) | Warm (31 °C) | Directional |
|--------|-------------|-------------|-------------|
| Final population | > initial (10) | any | cool > warm offspring |
| Complete repro cycles | ≥ 2 | any | — |
| Juveniles present | yes | — | cool > warm juvenile count |
| Egg-dropping events | low | elevated | warm ≥ cool |
| Reproductive readiness | 0.3 – 1.0 | < cool | warm < cool |

### Test cross-reference

`e2e_validation_suite::vs05_shrimp_breeding_thermal_window`,
`e2e_shrimp_scenario_probes::probe_successful_breeding`,
`e2e_shrimp_scenario_probes::probe_thermal_suppression`

---

## Scenario 6: Algae–plant competition under high light and low nutrients

### Rationale

Under high light intensity with limited dissolved nutrients (N, P), fast-
growing algae (both suspended and periphyton) tend to outcompete slow-growing
vascular plants. Plants with larger nutrient storage and root access can
buffer short-term limitation, but sustained nutrient scarcity in the water
column favors algae, which have higher surface-area-to-volume ratios and
faster nutrient uptake kinetics per unit biomass.

### Literature

- Tilman, D. (1982). *Resource Competition and Community Structure*.
  Princeton University Press. (R* theory: the species that can grow at the
  lowest resource concentration wins.)
- Stevenson, R.J. (1996). An introduction to algal ecology in freshwater
  benthic habitats. In *Algal Ecology: Freshwater Benthic Ecosystems*,
  Academic Press.
- Walstad, D. (2003). *Ecology of the Planted Aquarium*, 2nd ed.
  Chapter 13: Algae control.

### Expected qualitative outcome

A 30 L tank with moderate initial plant biomass (10 g fast stem), high light
(intensity 1.0, 14 h photoperiod), very low dissolved N and P (near
detection limits), no fertilization, and no shrimp:

1. Algae nuisance index rises over 60 days.
2. Plant biomass declines or stagnates (nutrient-limited).
3. Final algae nuisance > initial algae nuisance.
4. Final plant health index < initial plant health index.

### Confidence: **medium**

The direction is well-supported by resource competition theory and
aquarium practice. The magnitude depends on the model's nutrient-uptake
kinetics, which carry `heuristic` confidence for specific Ks values.

### Envelope bounds

The scenario uses rosette plants only (slower growth), a trickle feed
(0.02 g/day), and no microfauna grazing pressure. Moderate initial algae
biomass (2.5 g periphyton + suspended) provides a realistic colonization seed.

| Metric | Direction | Justification |
|--------|-----------|---------------|
| Algae total biomass | does not collapse (>50% of initial) | sufficient light and trickle nutrients sustain algae |
| Plant biomass (final vs initial) | decreases or stagnates (<120% of initial) | nutrient limitation caps growth |
| Plant health index | declines | sustained N/P deprivation under algae competition |

### Test cross-reference

`e2e_validation_suite::vs06_algae_plant_competition`

---

## Scenario 7: Nitrate removal via substrate denitrification

### Rationale

In tanks with deep or porous substrates, suboxic zones below the O₂
penetration depth support facultative denitrifiers that reduce NO₃⁻ to N₂
gas. This pathway removes fixed nitrogen from the system entirely (exported
as gas). A tank with a mature planted substrate should therefore show lower
steady-state NO₃⁻ than an otherwise identical bare-bottom tank where
denitrification cannot occur.

### Literature

- Seitzinger, S.P. (1988). Denitrification in freshwater and coastal marine
  ecosystems: Ecological and geochemical significance. *Limnology and
  Oceanography*, 33(4), 702–724.
- Vymazal, J. (2007). Removal of nutrients in various types of constructed
  wetlands. *Science of the Total Environment*, 380(1-3), 48–65.
- Aquarium context: substrate-driven denitrification is a known mechanism
  in deep sand beds and planted substrate systems (Walstad 2003; Brock 2017
  hobby literature).

### Expected qualitative outcome

Two 60 L tanks with identical bioload and feeding (0.1 g/day, 56 days),
weekly 20% water changes:

1. **Planted + deep substrate**: active substrate with coarse porous layer;
   mature denitrifier community (activity index > 0). Shows measurable
   cumulative N₂ export and lower final NO₃⁻.
2. **Bare-bottom**: no substrate, no denitrification pathway. All
   nitrification end-product accumulates as NO₃⁻.
3. Planted-substrate NO₃⁻ < bare-bottom NO₃⁻ at end of run.
4. Planted-substrate cumulative N₂ export > 0.

### Confidence: **medium**

The denitrification pathway direction is well-established in aquatic
ecology. The magnitude of NO₃⁻ reduction in a small aquarium substrate
is less certain — hobby tanks often have thin substrates with limited
suboxic volume. The model's `denitrification_pore_water_mixing_factor`
and `denitrification_maturation_days` carry `heuristic` confidence.

### Envelope bounds

The planted arm's NO₃⁻ may not be lower than the bare arm in absolute
terms, because plants contribute additional organic matter that decomposes
to TAN → NO₃⁻. The core validation is therefore on N₂ export (gas
pathway) rather than steady-state NO₃⁻ concentration.

The scenario pre-seeds the denitrifier activity index at 0.8 (simulating
an established tank past the 60-day maturation ramp) and uses deep
substrate with reduced porosity (0.35) to maximize suboxic pore volume.

| Metric | Planted+substrate | Bare-bottom | Validation |
|--------|-------------------|-------------|------------|
| Cumulative N₂ export (mg N) | > 1.0 | ≤ 0.1 | planted has active denitrification |
| Denitrifier activity index | > 0.1 | 0.0 | community is established |

### Test cross-reference

`e2e_validation_suite::vs07_nitrate_removal_denitrification`

---

## Scenario 8: Stocking density crash (overcrowding + overfeeding)

### Rationale

Overcrowding and overfeeding without adequate water changes produces a
predictable failure cascade: excess feed → ammonia accumulation →
nitrification can't keep pace → TAN and NO₂⁻ spike → shrimp
toxicity → mortality → further ammonia from decomposition → crash.
This is the canonical "new tank syndrome" or "crash" failure mode.

### Literature

- Timmons, M.B. & Ebeling, J.M. (2010). *Recirculating Aquaculture*,
  2nd ed. Chapter 3: Water quality and its management. (Carrying capacity
  and ammonia toxicity thresholds.)
- Colt, J. & Armstrong, D.A. (1981). Nitrogen toxicity to crustaceans,
  fish, and molluscs. *Bio-Engineering Symposium for Fish Culture*,
  FCS Pub. 1, 34–47.
- Aquarium context: overfeeding → ammonia → crash is the most common
  failure mode in hobby tanks and is universally described in beginner
  guides (Walstad 2003; Axelrod & Vorderwinkler 1995).

### Expected qualitative outcome

A 30 L tank with 20 shrimp (overcrowded), 0.5 g/day feeding (excessive),
no water changes, moderate initial biofilter:

1. TAN rises sharply within the first week.
2. Shrimp begin dying within 1–2 weeks (TAN/NO₂⁻ toxicity).
3. By week 4, most or all shrimp are dead.
4. Chemistry continues to deteriorate (positive feedback from corpse
   decomposition).

### Confidence: **high**

The ammonia toxicity → mortality → decomposition feedback loop is
universally observed and mechanistically simple. The qualitative outcome
(population collapse) is among the most predictable phenomena in aquarium
husbandry.

### Envelope bounds

| Checkpoint | TAN (mg N/L) | Shrimp count | pH |
|------------|-------------|-------------|-----|
| Week 1 | 1.0 – 30.0 | 5 – 20 | 5.0 – 8.5 |
| Week 2 | 3.0 – 60.0 | 0 – 15 | 4.5 – 8.5 |
| Week 4 | 5.0 – 100.0 | 0 – 5 | 4.5 – 8.5 |
| Week 8 | 10.0 – 400.0 | 0 – 0 | 4.5 – 8.5 |

Note: The week-8 TAN ceiling is wide because 8 weeks of 0.5 g/day overfeeding
with dead shrimp decomposing and no water changes can produce extreme
ammonia levels (> 200 mg N/L observed in model runs).

### Test cross-reference

`e2e_validation_suite::vs08_stocking_density_crash`,
`e2e_shrimp_scenario_probes::probe_crash_mode`

---

## Scenario confidence summary

| # | Scenario | Domain | Confidence | Status |
|---|----------|--------|------------|--------|
| 1 | Fishless cycling timeline | Chemistry / Microbiology | high | validated directionally |
| 2 | Aeration effects (DO + pH) | Physics / Chemistry | high | validated directionally |
| 3 | Day/night pH swing | Chemistry / Plant biology | high | validated directionally |
| 4 | Source-water differentiation | Chemistry | high | validated directionally |
| 5 | Shrimp breeding thermal window | Animal behavior | high | validated directionally |
| 6 | Algae–plant competition | Ecology | medium | still heuristic |
| 7 | Nitrate removal (denitrification) | Microbiology / Chemistry | medium | still heuristic |
| 8 | Stocking density crash | Toxicology / Husbandry | high | validated directionally |

**Validated directionally**: The model reproduces the correct qualitative
direction with high confidence. Envelope violations should be treated as
bugs.

**Still heuristic**: The model produces qualitatively reasonable behavior,
but key parameters (growth kinetics Ks values, denitrification mixing
factors) carry `heuristic` or `expert` confidence. Envelope bounds may shift
as better calibration data becomes available.

---

## Validation report template

The calibration workflow (G4) can auto-populate this template by running
each scenario and recording observed values against the envelope bounds.

```
============================================================
  VALIDATION REPORT — Tank Simulator vX.Y
  Generated: YYYY-MM-DD HH:MM:SS UTC
  Seed: <seed>
============================================================

SCENARIO 1: Fishless cycling timeline
  Status: PASS / FAIL
  Checkpoints:
    Week 2: TAN=X.XX NO2=X.XX NO3=X.XX maturity=X.XX pH=X.XX  [PASS/FAIL]
    Week 4: TAN=X.XX NO2=X.XX NO3=X.XX maturity=X.XX pH=X.XX  [PASS/FAIL]
    Week 6: TAN=X.XX NO2=X.XX NO3=X.XX maturity=X.XX pH=X.XX  [PASS/FAIL]
    Week 8: TAN=X.XX NO2=X.XX NO3=X.XX maturity=X.XX pH=X.XX  [PASS/FAIL]
  Violations: <list or "none">

SCENARIO 2: Aeration effects
  Status: PASS / FAIL
  Observed:
    Aerated: DO=X.XX pH=X.XXX
    Passive: DO=X.XX pH=X.XXX
    DO gap=X.XX  pH gap=X.XXX
  Violations: <list or "none">

SCENARIO 3: Day/night pH swing
  Status: PASS / FAIL
  Observed:
    Light max pH=X.XXX  Dark min pH=X.XXX  Swing=X.XXX
  Violations: <list or "none">

SCENARIO 4: Source-water differentiation
  Status: PASS / FAIL
  Observed:
    Hard shrimp: pH=X.XXX  RO-like: pH=X.XXX  Gap=X.XXX
  Violations: <list or "none">

SCENARIO 5: Shrimp breeding thermal window
  Status: PASS / FAIL
  Observed:
    Cool (25C): final_pop=XX cycles=X juveniles=XX
    Warm (31C): final_pop=XX cycles=X juveniles=XX
  Violations: <list or "none">

SCENARIO 6: Algae–plant competition
  Status: PASS / FAIL
  Observed:
    Algae nuisance: initial=X.XXX final=X.XXX
    Plant biomass: initial=X.XXX final=X.XXX
    Plant health:  initial=X.XXX final=X.XXX
  Violations: <list or "none">

SCENARIO 7: Nitrate removal (denitrification)
  Status: PASS / FAIL
  Observed:
    Planted: NO3=X.XX  N2_export=X.XX mg N
    Bare:    NO3=X.XX  N2_export=X.XX mg N
  Violations: <list or "none">

SCENARIO 8: Stocking density crash
  Status: PASS / FAIL
  Checkpoints:
    Week 1: TAN=X.XX shrimp=XX pH=X.XX  [PASS/FAIL]
    Week 2: TAN=X.XX shrimp=XX pH=X.XX  [PASS/FAIL]
    Week 4: TAN=X.XX shrimp=XX pH=X.XX  [PASS/FAIL]
    Week 8: TAN=X.XX shrimp=XX pH=X.XX  [PASS/FAIL]
  Violations: <list or "none">

============================================================
  SUMMARY: X/8 scenarios passed
  High-confidence:  X/6 passed
  Medium-confidence: X/2 passed
============================================================
```
