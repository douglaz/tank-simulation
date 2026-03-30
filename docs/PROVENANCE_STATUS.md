# Parameter Provenance Status

Developer-facing summary of parameter provenance annotations. Originally written for
tanksim-6e5.7.2, verified current as of tanksim-6e5.7.5. This document distinguishes
strong anchors from provisional approximations so future tuning and validation work
can focus on the weakest assumptions first.

## Coverage Summary

| Parameter family | File(s) | Annotated? | Notes |
|---|---|---|---|
| Nitrogen kinetics (Ks, Vmax, yields, decay) | `process/default.toml` | Yes (32 entries) | Mix of literature, expert, and heuristic |
| Denitrification | `process/default.toml` | Yes | 5 parameters |
| Carbonate solver constants | `chemistry.rs` (code) | Documented here + code comments | Not TOML-backed |
| Source water: hard_shrimp, moderate | `source_water/*.toml` | Yes (6 entries each) | Heuristic and expert |
| Source water: soft_acidic, ro_like | `source_water/*.toml` | **No** | Lower priority |
| Substrate: active_planted, coarse_porous | `substrate/*.toml` | Partial (colonizable_area_factor only) | Other indices pending |
| Substrate: inert_sand, inert_gravel | `substrate/*.toml` | **No** | |
| Shrimp biology | `shrimp/neocaridina_davidi.toml` | Yes (22 entries) | Literature anchors + heuristic thresholds |
| Plant presets | `plants/*.toml` | **No** | Growth rates, uptake biases |
| Process routing fractions | `process/default.toml` | **No** | Assimilation, respiration, excretion splits |
| Thermal transport | `process/default.toml` | **No** | k_surface, k_wall |

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

### Biomass-rate and turnover controls
These coefficients now carry `param_meta`, but they remain more model-structural than the
half-saturation anchors above because the biomass units and guild aggregation are simplified.
Future calibration should prioritize the heuristic rows first.

| Parameter | Value | Unit | Confidence | Notes |
|-----------|-------|------|------------|-------|
| `decomposer_vmax_per_hour` | 0.02 | per hour | heuristic | Biomass-normalized DOC-processing ceiling calibrated to aquarium DOC turnover rather than a portable literature constant |
| `decomposer_growth_yield` | 0.3 | g biomass / g DOC | expert | Lower-half heterotroph yield anchor for refractory aquarium DOC |
| `decomposer_decay_rate_per_hour` | 0.002 | per hour | expert | Starvation/persistence knob rather than a tightly measured aquarium constant |
| `aob_vmax_mg_n_per_g_per_hour` | 1.5 | mg N / g biomass / h | heuristic | Chosen to reproduce fishless-cycling TAN decline with the model's coarse nitrifier biomass units |
| `aob_growth_yield` | 0.05 | g biomass / mg N oxidized | literature | Standard autotrophic nitrifier yield envelope from ASM / biofilm texts |
| `aob_decay_rate_per_hour` | 0.003 | per hour | expert | First-pass nitrifier persistence anchor for aquarium biofilms |
| `nob_vmax_mg_n_per_g_per_hour` | 1.2 | mg N / g biomass / h | heuristic | Tuned so nitrite peaks remain transient instead of instantly cleared |
| `nob_growth_yield` | 0.04 | g biomass / mg N oxidized | literature | Slightly below the AOB yield, consistent with common NOB formulations |
| `nob_decay_rate_per_hour` | 0.003 | per hour | expert | Same order-of-magnitude persistence assumption as AOB |
| `comammox_vmax_fraction` | 0.4 | fraction of AOB vmax | heuristic | Relative-rate proxy used because comammox rides on the shared nitrifier biomass scaffold |
| `comammox_growth_yield` | 0.03 | g biomass / mg N oxidized | heuristic | Sparse direct measurements; held near the Nitrospira/NOB envelope |
| `comammox_decay_rate_per_hour` | 0.004 | per hour | heuristic | Slightly faster turnover prevents a permanent simplified late-cycle takeover |

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

### Denitrification Parameters (`process/default.toml`)
These fields now load through `ProcessParamsPreset`, carry `param_meta`, and map through
scenario materialization without falling back to runtime defaults.

| Parameter | Value | Unit | Confidence | Notes |
|-----------|-------|------|------------|-------|
| `denitrification_vmax_mg_n_per_l_per_hour` | 0.1 | mg N/L/h | literature | Conservative mid-range freshwater sediment anchor for mature suboxic pore water |
| `denitrification_k_no3_mg_n_per_l` | 2.0 | mg N/L | literature | Middle-of-range Monod constant to avoid nitrate-unlimited denitrification at trace NO₃ |
| `denitrification_k_doc_mg_c_per_l` | 5.0 | mg C/L | literature | Assumes mixed aquarium DOC quality rather than acetate-like lab substrates |
| `denitrification_pore_water_mixing_factor` | 0.5 | fraction | heuristic | Geometry/flow proxy for pore-water access, not a directly measured constant |
| `denitrification_activity_maturation_days` | 60.0 | days | expert | Encodes the slower establishment of denitrifiers versus nitrifiers in new tanks |

---

## Carbonate Chemistry

### Source-water presets (`hard_shrimp.toml`, `moderate.toml`)
| Parameter | hard_shrimp | moderate | Confidence | Notes |
|-----------|-------------|----------|------------|-------|
| `dic_mg_c_per_l` | 36.0 | 20.0 | heuristic | Derived from target KH plus carbonate equilibrium preview |
| `alkalinity_meq_per_l` | 2.857 | 1.429 | heuristic | KH 8.0 / KH 4.0 respectively |
| `bicarbonate_mg_per_l` | 174.3 | 87.2 | heuristic | Explicit reservoir view: alkalinity × 61.02 |
| `calcium_mg_per_l` | 42.0 | 25.0 | expert | Mineral targets chosen to clear shrimp molt minima with explicit margin |
| `magnesium_mg_per_l` | 15.0 | 10.0 | expert | Paired with Ca to keep shrimp-safe Ca:Mg ratios |
| `chloride_mg_per_l` | 20.0 | 18.0 | heuristic | Sets the Cl:NO₂ protection baseline |

Interpretation:
- `dic_mg_c_per_l`, `alkalinity_meq_per_l`, `bicarbonate_mg_per_l`, and `chloride_mg_per_l`
  are preset heuristics. They are derived from target KH/GH bands and utility-water style
  assumptions rather than measured from a named source.
- `calcium_mg_per_l` and `magnesium_mg_per_l` are stronger expert-curated anchors. They are
  still preset defaults, but they now explicitly stand apart from the heuristic carbonate values.

### Code-resident solver and boundary constants (`crates/tank_core/src/systems/chemistry.rs`)

These coefficients stay in code rather than TOML `param_meta` because every source-water
preset shares the same carbonate solver. Canonical provenance currently lives in the
doc comments in `chemistry.rs` and in this table until a dedicated code-constant metadata
registry exists.

| Constant | Value | Confidence | Source | Notes |
|----------|-------|------------|--------|-------|
| `ATMOSPHERIC_CO2_PPM` | `410 ppm` | expert | Rounded NOAA background-atmosphere mean from the late-2010s / ~2020 envelope | Fixed boundary condition for current scope, not a live yearly climate input |
| `KH_CO2_25C_MOL_PER_L_ATM` | `3.4e-2 mol/(L·atm)` | literature | Stumm & Morgan 1996 | 25°C Henry-law anchor for atmospheric CO₂ coupling |
| `KH_TEMP_FACTOR_K` | `2400 K` | expert | First-pass van 't Hoff fit for the 15-35°C aquarium envelope | Revisit if salinity or temperature envelope expands |
| `pKa1(T)` | `3404.71/T + 0.032786*T - 14.8435` (`T` in K); `pKa1(25°C) ≈ 6.35` | literature | Harned & Davis 1943 | Full temperature correction across the current freshwater range |
| `PKA2` | `10.33` | expert | 25°C freshwater carbonate tables; `docs/carbonate_state_contract.md` | Literature-consistent anchor, but the fixed-temperature use is still a first-pass simplification |

### Not Yet Annotated
- `soft_acidic.toml` and `ro_like.toml` source water profiles do not yet carry `param_meta`.
  These are lower-priority because their extreme values (very low KH/GH) are less likely
  to be cited as representative defaults.

---

## Habitat / Biofilter Scaling (substrate/*.toml)

### Representative annotated presets
| Preset | Parameter | Value | Confidence | Notes |
|--------|-----------|-------|------------|-------|
| `active_planted.toml` | `colonizable_area_factor` | 0.8 | heuristic | Rooted aquasoil sits above inert gravel but below dedicated porous media for attachment area |
| `coarse_porous.toml` | `colonizable_area_factor` | 0.9 | heuristic | Highest shipped factor; directly signals this preset's role as the strongest nitrifier habitat substrate, with >1.0 still physically meaningful for pore-wall surface area beyond projected footprint |

`colonizable_area_factor` is the current load-bearing substrate scaling knob because
`footprint × colonizable_area_factor` feeds colonizable area and downstream biofilm/periphyton
capacity. Other substrate indices and nutrient-charge fields are still pending annotation.

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
| `nh3_stress_threshold_mg_n_per_l` | 0.02 | mg NH3-N/L | Conservative unionized-ammonia onset anchor; species-specific Neocaridina data remain sparse |

### Provisional (heuristic / placeholder)
| Parameter | Value | Confidence | Why provisional |
|-----------|-------|------------|-----------------|
| `high_temp_repro_penalty_full_c` | 33.0 | heuristic | Near lethal limit; exact cessation point unknown |
| `low_temp_repro_ramp_width_c` | 4.0 | **placeholder** | Pure tuning parameter; no literature support |
| `temp_condition_low_divisor_c` / `temp_condition_high_divisor_c` | 10.0 / 8.0 | heuristic | Broad condition-envelope shape controls; calibrated for gradual cold/hot taper rather than measured physiology slopes |
| `temp_condition_min_factor` | 0.2 | heuristic | Floor that keeps temperature stress from forcing condition fully to zero by itself |
| `molt_stress_condition_midpoint` | 0.5 | heuristic | Model threshold that turns low condition into daily molt-stress pressure |
| `molt_stress_thermal_cap` | 0.5 | heuristic | Blend cap preventing heat alone from saturating daily molt stress |
| `tan_repro_full_suppression_mg_n_per_l` | 3.0 | heuristic | Derived from the corrected TAN onset threshold plus the legacy decline-span formula |
| `no2_repro_full_suppression_mg_n_per_l` | 1.5 | heuristic | Derived from the corrected NO₂ onset threshold plus the legacy decline-span formula |
| `chloride_protection_factor` | 0.5 | heuristic | Mechanism well-established in fish; transfer to Neocaridina is medium-confidence |
| `nh3_stress_response_scale` | 2.0 | heuristic | Converts NH3 excess into hourly stress; curve-shape calibration, not a toxicology datum |
| `molt_stress_mineral_*_weight` | 0.5/0.3/0.2 | heuristic | Calibration weights, not derived from data |

---

## Parameters Not Yet Annotated

The following parameter families still need additional `param_meta` coverage and remain
candidates for future annotation beads:

- **Substrate presets** (`inert_sand`, `inert_gravel`, plus non-`colonizable_area_factor`
  fields in `active_planted` / `coarse_porous`): detritus trapping indices, nutrient charges,
  and secondary habitat modifiers
- **Plant presets** (fast_stem, root_rosette): growth_rate_index, uptake bias weights
- **Remaining source water** (soft_acidic, ro_like): full chemistry profiles
- **Process routing fractions**: shrimp/microfauna assimilation, respiration, excretion splits
- **Process thermal transport**: k_surface_w_per_m2_k, k_wall_w_per_m2_k

---

## How to Use This Summary

1. **Before making a scientific claim**: check that the parameter's confidence is `literature`.
   If it's `heuristic` or `placeholder`, the claim needs qualification.
2. **Before tuning a parameter**: check its `valid_range` in the TOML `param_meta`. For
   code-resident constants such as the carbonate solver coefficients, check the doc comments
   in `crates/tank_core/src/systems/chemistry.rs` and the table above instead.
3. **When adding new parameters**: add a `[param_meta.name]` entry in the same TOML file.
   Use the `ConfidenceLevel` enum: `literature`, `expert`, `heuristic`, or `placeholder`.
4. **When a parameter graduates**: update its `confidence` and `source` fields in the TOML.
   Update this summary doc if it was listed as provisional.
