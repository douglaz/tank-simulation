# Parameter Provenance Status

Developer-facing summary of parameter provenance annotations as of tanksim-6e5.7.2.
This document distinguishes strong anchors from provisional approximations so future
tuning and validation work can focus on the weakest assumptions first.

## Confidence Tiers

| Tier | Meaning | Action needed |
|------|---------|---------------|
| **literature** | Peer-reviewed source or stoichiometric derivation | None — strong anchor |
| **expert** | Domain-expert recommendation, widely cited but not from a single controlled study | Confirm with literature review if parameter becomes load-bearing for a claim |
| **heuristic** | Calibration fit, rule of thumb, or analogy | Priority target for sensitivity analysis and literature search |
| **placeholder** | Temporary stand-in awaiting better data | Must be resolved before any scientific claim depends on it |

---

## Nitrogen Kinetics (process/default.toml)

### Strong Anchors (literature)
| Parameter | Value | Unit | Source |
|-----------|-------|------|--------|
| `aob_k_tan_mg_n_per_l` | 1.0 | mg N/L | Prosser 1990; Rittmann & McCarty 2001 |
| `aob_k_do_mg_per_l` | 0.5 | mg O₂/L | Rittmann & McCarty 2001; Henze et al. 2008 |
| `nob_k_nitrite_mg_n_per_l` | 0.5 | mg N/L | Blackburne et al. 2007; Henze et al. 2008 |
| `nob_k_do_mg_per_l` | 0.8 | mg O₂/L | Rittmann & McCarty 2001; Blackburne et al. 2007 |
| `comammox_k_tan_mg_n_per_l` | 0.2 | mg N/L | Kits et al. 2017; Daims et al. 2015 |
| `decomposer_k_doc_mg_c_per_l` | 3.0 | mg C/L | Wetzel 2001; Boulton et al. 1998 |
| `decomposer_k_do_mg_per_l` | 0.5 | mg O₂/L | Rittmann & McCarty 2001 |
| `o2_per_mg_n_nitrified` | 4.57 | mg O₂/mg N | Stoichiometric (64/14) |
| `alkalinity_meq_per_mg_n_nitrified` | 0.1428 | meq/mg N | Stoichiometric (2/14.007) |

### Provisional (heuristic)
| Parameter | Value | Unit | Why provisional |
|-----------|-------|------|-----------------|
| `comammox_k_do_mg_per_l` | 0.6 | mg O₂/L | Analogy with Nitrospira; limited direct measurements |
| `plant_max_growth_rate_fast_stem_per_day` | 0.08 | per day | Calibrated to Hygrophila observations, not from controlled studies |
| `plant_max_growth_rate_root_rosette_per_day` | 0.06 | per day | Calibrated to Cryptocoryne/Echinodorus, limited quantitative data |
| `periphyton_max_growth_rate_per_day` | 0.15 | per day | Set lower than planktonic by analogy; boundary-layer assumption |

### Literature-backed but broad range
| Parameter | Value | Unit | Range | Notes |
|-----------|-------|------|-------|-------|
| `algae_max_growth_rate_per_day` | 0.2 | per day | 0.1–0.5 | Conservative within Eppley curve predictions |

### Not Yet in Preset Layer (TODO)
The following denitrification parameters are present in `default.toml` but are not carried by
`ProcessParamsPreset` — the runtime reads code defaults from `tank_core::types::process`.
Their provenance cannot be attached via `param_meta` until the preset struct is extended.

| Parameter | Value | Unit | Literature range | Confidence estimate |
|-----------|-------|------|-----------------|---------------------|
| `denitrification_vmax_mg_n_per_l_per_hour` | 0.1 | mg N/L/h | 0.01–0.5 | literature (mid-range) |
| `denitrification_k_no3_mg_n_per_l` | 2.0 | mg N/L | 0.5–5.0 | literature |
| `denitrification_k_doc_mg_c_per_l` | 5.0 | mg C/L | 1.0–10.0 | literature |
| `denitrification_pore_water_mixing_factor` | 0.5 | fraction | 0.3–0.7 | heuristic |
| `denitrification_activity_maturation_days` | 60.0 | days | 30–90 | expert |

**Action**: Extend `ProcessParamsPreset` to include denitrification fields, then attach
formal `param_meta` entries. The literature ranges above are documented in the TOML comments
and in `tank_core::types::process` default-value doc comments.

---

## Carbonate Chemistry (source_water/*.toml)

### Annotated in hard_shrimp.toml and moderate.toml (heuristic)
| Parameter | hard_shrimp | moderate | Unit | Notes |
|-----------|-------------|----------|------|-------|
| `dic_mg_c_per_l` | 36.0 | 20.0 | mg C/L | Derived from target KH via carbonate equilibrium |
| `alkalinity_meq_per_l` | 2.857 | 1.429 | meq/L | KH 8.0 / KH 4.0 respectively |
| `bicarbonate_mg_per_l` | 174.3 | 87.2 | mg/L | = alkalinity × 61.02 |
| `calcium_mg_per_l` | 42.0 | 25.0 | mg/L | Set for molt success margin |
| `magnesium_mg_per_l` | 15.0 | 10.0 | mg/L | Ca:Mg ratio 2.8:1 / 2.5:1 |
| `chloride_mg_per_l` | 20.0 | 18.0 | mg/L | Sets Cl:NO₂ protection baseline |

All carbonate-related source water parameters are **heuristic** — derived from target
KH/GH values and equilibrium math rather than measured from specific water supplies.
This is appropriate for preset defaults but means the exact values are not scientifically
citable. The equilibrium *relationships* between DIC, alkalinity, and bicarbonate are
literature-backed (Stumm & Morgan 1996).

### Not Yet Annotated
- `soft_acidic.toml` and `ro_like.toml` source water profiles do not yet carry `param_meta`.
  These are lower-priority because their extreme values (very low KH/GH) are less likely
  to be cited as representative defaults.

---

## Shrimp Biology (shrimp/neocaridina_davidi.toml)

### Strong Anchors (literature)
| Parameter | Value | Unit | Source |
|-----------|-------|------|--------|
| `body_nitrogen_mg_per_g_wet_mass` | 27.6 | mg N/g | Anger 2001 (crustacean stoichiometry) |
| `body_carbon_mg_per_g_wet_mass` | 172.4 | mg C/g | Anger 2001 |

### Expert-level (well-supported but not quantitatively precise)
| Parameter | Value | Unit | Notes |
|-----------|-------|------|-------|
| `high_temp_repro_penalty_start_c` | 28.0 | °C | Widely cited hobbyist + Tropea 2015 |
| `ca_min_mg_per_l` | 20.0 | mg/L | Crustacean mineralization literature |
| `mg_min_mg_per_l` | 5.0 | mg/L | Less studied than Ca for dwarf shrimp |
| `tan_repro_threshold_mg_n_per_l` | 1.0 | mg N/L | EPA criteria + invertebrate sensitivity |
| `no2_repro_threshold_mg_n_per_l` | 0.5 | mg N/L | Precautionary; species data sparse |

### Provisional (heuristic / placeholder)
| Parameter | Value | Confidence | Why provisional |
|-----------|-------|------------|-----------------|
| `high_temp_repro_penalty_full_c` | 33.0 | heuristic | Near lethal limit; exact cessation point unknown |
| `low_temp_repro_ramp_width_c` | 4.0 | **placeholder** | Pure tuning parameter; no literature support |
| `chloride_protection_factor` | 0.5 | heuristic | Mechanism well-established in fish; transfer to Neocaridina is medium-confidence |
| `molt_stress_mineral_*_weight` | 0.5/0.3/0.2 | heuristic | Calibration weights, not derived from data |

---

## Parameters Not Yet Annotated

The following parameter families have no `param_meta` entries yet and are candidates for
future annotation beads:

- **Substrate presets** (inert_sand, inert_gravel, active_planted, coarse_porous): colonizable
  area factors, detritus trapping indices, nutrient charges
- **Plant presets** (fast_stem, root_rosette): growth_rate_index, uptake bias weights
- **Remaining source water** (soft_acidic, ro_like): full chemistry profiles
- **Process routing fractions**: shrimp/microfauna assimilation, respiration, excretion splits
- **Process thermal transport**: k_surface_w_per_m2_k, k_wall_w_per_m2_k

---

## How to Use This Summary

1. **Before making a scientific claim**: check that the parameter's confidence is `literature`.
   If it's `heuristic` or `placeholder`, the claim needs qualification.
2. **Before tuning a parameter**: check its `valid_range` in the TOML `param_meta`. Tuning
   outside the range will produce a loader warning.
3. **When adding new parameters**: add a `[param_meta.name]` entry in the same TOML file.
   Use the `ConfidenceLevel` enum: `literature`, `expert`, `heuristic`, or `placeholder`.
4. **When a parameter graduates**: update its `confidence` and `source` fields in the TOML.
   Update this summary doc if it was listed as provisional.
