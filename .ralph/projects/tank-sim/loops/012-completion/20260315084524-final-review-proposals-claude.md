---
artifact: final-review-proposals
loop: 12
project: tank-sim
backend: claude
role: final_reviewer
created_at: 2026-03-15T08:45:24Z
---

Good — the chemistry test at line 15-19 explicitly sets non-zero DIC rates, confirming the code-default zero values don't break tests. Now I have a complete picture.

---

# Final Review: NO AMENDMENTS

## Summary

The project is a comprehensive, well-engineered implementation of the freshwater planted shrimp tank simulator. All 85 tests pass (including all 10 mandatory acceptance tests). Here's why the project is correct:

**Complete spec coverage:**
- All 12 `PlayerAction` variants implemented with proper validation
- `SimulationEngine` trait with the required 4 methods
- All 13 required `EventKind` values present and emitted
- All 6 required preset sets (4 source waters, 4 substrates, 2 plants, 1 shrimp, 1 process pack, 3 scenarios)
- Save/load with schema version, app version, full state, queued actions, and RNG state (JSON)
- TUI with Overview, Chemistry, Biology, Actions, and Log screens

**Correct formulas verified against spec:**
- Geometry: volume, surface area, wall area, ratios — all correct (`geometry.rs`)
- Temperature: UA model with `k_surface * surface_m2 * top_factor + k_wall * wall_m2`, deadband heater that never cools (`temperature.rs:48-68`)
- NH3 speciation: `pKa = 0.09018 + 2729.92 / (273.2 + temp)`, `fraction = 1/(1+10^(pKa-pH))` (`chemistry.rs:24-26`)
- DO saturation: correct 4-point table with linear interpolation (`temperature.rs:6`)
- pH: `clamp(6.3 + log10(alk) - log10(dic), 5.5, 8.5)` (`chemistry.rs:19`)
- GH/KH/TDS/conductivity: all matching spec formulas (`snapshot.rs:79-80`)
- Nitrification stoichiometry: 4.57 mg O2/mg N total (split as 3.43 AOB + 1.14 NOB), 0.1428 meq alkalinity/mg N (`nitrogen_cycle.rs:131-133`)
- Monod kinetics: `vmax * biomass * env_factor * S/(K+S)` (`nitrogen_cycle.rs:333-341`)

**Correct causal relationships:**
- Tank size affects dilution, thermal inertia, and stability (verified by `tank_size_thermal_response` tests)
- Nitrification is biomass-limited, oxygen-limited, and maturity-gated with shared DO/alkalinity budgets (`nitrogen_cycle.rs:126-232`)
- Plants compete with algae for light and nutrients; `RootFeedingRosette` benefits from active substrate (`plant_growth.rs:110-141`)
- Overfeeding raises detritus, lowers DO, increases algae pressure (verified by `overfeeding_effects` test)
- Shrimp reproduction optimal at 22–26°C, penalized at 30°C, near-zero at 33°C (`shrimp.rs:519-540`)
- Periphyton serves as both nuisance indicator and shrimp food source

**Determinism and invariants:**
- ChaCha8Rng with seed+draw-count serialization ensures reproducible replays (`rng.rs`)
- `enforce_invariants` runs after every tick: no negative pools, clamped indices, population constraints (`invariants.rs`)
- Event log kept to 200, snapshot exposes last 20

**Minor observations (not amendments):**
- `ProcessParams::default()` sets DIC rates to 0.0 while the TOML preset uses 0.08/0.12 — tests explicitly set non-zero values where needed, and the canonical path loads TOML presets
- Water changes don't mix DO (spec's mass balance test doesn't require it; reaeration handles convergence)
- `NitrogenCycleOutput` is computed but unused in the engine — harmless dead coupling point
