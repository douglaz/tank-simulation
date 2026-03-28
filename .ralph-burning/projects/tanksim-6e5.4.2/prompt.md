○ tanksim-6e5.4.2 · Implement explicit carbonate equilibrium and pH solver   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: carbonates, chemistry, pH, phase-2, scientific-core

Context:
- After D1 chooses the state contract, the engine needs a real equilibrium update instead of the current 6.3 + log10(alkalinity) - log10(DIC) shortcut.
- This is the core chemistry change that unlocks more believable pH differentiation and gas-exchange effects.

Deliverable:
- Implement the carbonate-state contract chosen in D1 around DIC, alkalinity, temperature, and any clearly-defined derived species outputs.
- Add the explicit equilibrium/pH solver with temperature-dependent constants and validate it against reference cases plus representative edge-condition sweeps.
- Replace the live gameplay pH shortcut in the chemistry step with the new solver.
- Update WaterState, snapshot/API outputs, and save/migration paths as needed so the new contract is inspectable and old saves still load cleanly.
- Keep the implementation numerically stable within the expected aquarium parameter ranges (roughly pH 5.5-8.5, alkalinity 0-10 meq/L, temperature 15-35 C).

Likely touch points:
- crates/tank_core/src/systems/chemistry.rs
- crates/tank_core/src/types/water.rs
- crates/tank_core/src/types/state.rs
- crates/tank_core/src/save.rs
- crates/tank_core/src/types/snapshot.rs

## Acceptance Criteria


Acceptance Criteria:
- WaterState/save/snapshot clearly distinguish stored carbonate inputs from derived outputs, and any required migration/default path loads old saves successfully.
- gameplay pH is computed from the new carbonate solver rather than the old shortcut, and shipped source waters now produce meaningfully differentiated pH values.
- the solver is deterministic and numerically stable across the reference cases and representative sweep/edge bounds (no NaN/Inf, reasonable pH bounds, directionally correct temperature response).
- snapshot/API outputs expose at least pH and CO2(aq) or an equivalent derived solver result so the new chemistry is inspectable by users and tests.

Dependencies:
  -> tanksim-6e5.4.1 (blocks) - Define carbonate-state contract and solver strategy
  -> tanksim-6e5.2.2 (blocks) - Implement canonical concentration and compartment helper APIs
  -> tanksim-6e5.1.2.3 (blocks) - Add save-schema versioning and migration scaffolding
  -> tanksim-6e5.4 (parent-child) - Phase 2 — carbonate chemistry, CO2 exchange, and pH realism

Dependents:
  <- tanksim-6e5.4.3 (blocks) - Integrate CO2 gas exchange with aeration, surface exchange, and ambient coupling
  <- tanksim-6e5.4.7 (blocks) - Validate nitrification-driven alkalinity depletion with the carbonate solver, expose diagnostics, and prepare the denitrification return path
  <- tanksim-6e5.7.2 (blocks) - Attach provenance and confidence metadata to high-value chemistry and ecology parameters
  <- tanksim-6e5.4.5 (blocks) - Upgrade source-water profiles to carry carbonate-relevant inputs and differentiated defaults
  <- tanksim-6e5.4.4 (blocks) - Connect photosynthesis and respiration to DIC / CO2 and day–night pH behavior

Comments:
  [2026-03-26 20:44 UTC] master: Canonicalization note: merged former D2a-D2d into canonical D2. The state contract, solver, live chemistry swap, and save/snapshot implications are one chemistry change; D1 remains the design spike and D6 remains downstream validation.
  [2026-03-26 13:46 UTC] backlog-planner: Future-self notes:
- The goal is a robust aquarium-focused solver, not a general geochemistry package.
- Preserve debuggability: intermediate values and assumptions should be inspectable in tests or dev snapshots.
- Do not bury important constants in magic numbers; keep them named and traceable.
- The existing dissolved_inorganic_carbon_mg_c_total field in WaterState can continue as the DIC state variable. The solver derives CO2(aq), HCO3-, CO3--, and pH from it + alkalinity + temperature.
