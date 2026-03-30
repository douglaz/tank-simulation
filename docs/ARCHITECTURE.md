# Architecture Overview

This document gives a new contributor a conceptual map of the simulator and a reading guide to the existing documentation.

## What the simulator models

`tank-simulation` is a deterministic, mechanistic freshwater aquarium simulator. It models a planted *Neocaridina davidi* shrimp tank as an ecosystem: chemistry, biology, and hardware interact each hour through mass-balance rules and guild-based ecology.

The model is **guild-based, not molecule-level**. It tracks conserved pools (nitrogen, carbon, phosphorus, oxygen, major ions) as whole-tank totals and derives concentrations for kinetics and display. Biology is represented as functional guilds (nitrifiers, decomposers, plants, algae, microfauna, shrimp stages) rather than individual organisms or species-level taxonomy.

## Key abstractions

### Units system

All conserved chemistry is stored as whole-tank element-basis totals (e.g., `ammonia_total_mg_n_total`, `dissolved_inorganic_carbon_mg_c_total`). Source-water inputs are per-liter. Snapshot/API display projects totals to scientific concentrations (mg N/L, mg P/L, mg C/L). Ion-style display (mg NO3/L) is opt-in. See [UNITS.md](UNITS.md) for the full policy.

### Carbonate equilibrium

Canonical state is DIC (mg C total) + alkalinity (meq total) + temperature. A closed-form quadratic solver derives CO2(aq), HCO3-, CO3--, and pH each chemistry step using temperature-corrected pKa1 (Harned & Davis 1943) and fixed pKa2. See [carbonate_state_contract.md](carbonate_state_contract.md) for the solver contract.

### Habitat registry

Five ecological zones, each with colonizable area and normalized exposure modifiers (flow, oxygen, light):

| Zone | Description |
|------|-------------|
| FilterMedia | Biological filter media inside the filter housing |
| GlassHardscape | Glass walls and hardscape surfaces |
| PlantSurfaces | Living plant leaf and stem surfaces |
| SubstrateSurface | Top surface of the substrate bed (oxic zone) |
| SubstrateDeep | Internal grain surfaces within the substrate column (suboxic) |

Areas are derived from geometry, hardware, and plant biomass. Exposure modifiers respond to filter flow, aeration, clogging, light intensity, and substrate properties. Implemented in `crates/tank_core/src/types/habitat.rs`.

### Stage-structured shrimp

Three life stages: Juvenile (0-45 days), Sub-adult (15-30 days), Adult (indefinite). Each stage carries its own count, reserve, and condition index. Population-wide state includes molt cycle, reproduction readiness, egg cohorts, and stress accumulators. See [shrimp_state_contract.md](shrimp_state_contract.md) for the design contract.

### Parameter provenance

Every tunable parameter can carry `ParamMeta` with confidence tier (`literature`, `expert`, `heuristic`, `placeholder`), source citation, valid range, and notes. Provenance lives in TOML preset files alongside the values. See [PROVENANCE_STATUS.md](PROVENANCE_STATUS.md) for current coverage.

## Workspace layout

| Crate | Role |
|-------|------|
| `tank_core` | Simulation engine: types, systems, save/load, events, invariants |
| `tank_data` | Packaged TOML presets (scenarios, source water, plants, substrate, shrimp, process) |
| `tank_scenarios` | Scenario materialization and startup overrides |
| `tank_api` | Axum HTTP server: snapshots, actions, stepping, save/load |
| `tank_tui` | Terminal UI client connecting to `tank_api` |
| `tank_harness` | Calibration harness and validation suite |

## Reading guide

Recommended reading order depends on your goal:

### Understanding the model (start here)

1. This file (ARCHITECTURE.md) — conceptual overview
2. [UNITS.md](UNITS.md) — unit policy and display conventions
3. [PROVENANCE_STATUS.md](PROVENANCE_STATUS.md) — parameter confidence tiers
4. [validation_scenarios.md](validation_scenarios.md) — what the model can reproduce

### Understanding chemistry

1. [carbonate_state_contract.md](carbonate_state_contract.md) — DIC/alkalinity solver
2. [MASS_FLOW.md](MASS_FLOW.md) — nitrogen and carbon flow paths
3. [ROUTING.md](ROUTING.md) — material routing through consumers and maintenance

### Understanding ecology

1. [shrimp_state_contract.md](shrimp_state_contract.md) — stage-structured population model
2. `crates/tank_core/src/types/habitat.rs` — habitat registry implementation
3. `crates/tank_core/src/systems/shrimp.rs` — shrimp system logic

### Extending the model

1. [UNITS.md](UNITS.md) — naming rules for new parameters and fields
2. [ROUTING.md](ROUTING.md) — how to close mass loops for new consumers
3. [PROVENANCE_STATUS.md](PROVENANCE_STATUS.md) — how to annotate new parameters

### Historical context

- [scientific_inventory.md](scientific_inventory.md) — forensic catalog of the v0.1 state
- [aquarium_sim_review_vnext.md](aquarium_sim_review_vnext.md) — original gap analysis that motivated the scientific-core upgrade
- [scientific_specs.md](scientific_specs.md) — condensed review gateway
- `spec.md` (root) — original v0.1 implementation specification

## Extension points

### Adding a new organism guild

1. Define the guild's state struct in `crates/tank_core/src/types/biology.rs`
2. Add a system update function in `crates/tank_core/src/systems/`
3. Close the mass loop per [ROUTING.md](ROUTING.md): ingested mass must return as assimilated reserve, feces, excreted N (TAN), and respired C (DIC/O2)
4. Add snapshot fields and TUI/API display
5. Add provenance annotations for all new parameters

### Adding a new habitat kind

1. Add a variant to `HabitatKind` in `crates/tank_core/src/types/habitat.rs`
2. Add a compute helper and wire it into `compute_habitat_registry()`
3. Update tests in `crates/tank_core/tests/` and `crates/tank_harness/`

### Adding a new chemical species

1. Add a `_mg_X_total` field to `WaterState` in `crates/tank_core/src/types/water.rs`
2. Add concentration helpers following the naming policy in [UNITS.md](UNITS.md)
3. Wire into relevant systems (nitrogen_cycle, chemistry, etc.)
4. Add snapshot field, API semantics entry, and TUI label with correct basis

### Adding a new process parameter

1. Add the field to the appropriate params struct with a basis-explicit name
2. Add a `[param_meta.<name>]` entry in the TOML preset file
3. Set confidence level, source, valid_range, and unit
4. Update [PROVENANCE_STATUS.md](PROVENANCE_STATUS.md)
