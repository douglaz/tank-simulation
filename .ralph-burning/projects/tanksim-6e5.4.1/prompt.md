○ tanksim-6e5.4.1 · Define carbonate-state contract and solver strategy   [● P0 · OPEN]
Owner: master · Type: spike
Created: 2026-03-26 · Updated: 2026-03-27
Labels: carbonates, chemistry, design, phase-2, scientific-core

Context:
- The current pH shortcut (chemistry.rs:18) is too compressed for the simulator's long-term goals, but the project still needs a pragmatic rather than maximal carbonate model.
- Before implementation, the team should agree on which carbonate variables are explicit state, which are derived, and what numerical strategy is acceptable.

Deliverable:
- A design note describing the chosen carbonate state contract. Recommended minimal state:
  - Explicit: total DIC (already exists), alkalinity (already exists), temperature (already exists)
  - Derived: CO2(aq), HCO3-, CO3--, pH (from equilibrium solver)
  - Or alternative: explicit CO2(aq) + alkalinity, derive the rest
- The intended solver approach, update order, and stability/precision expectations.
- Clear notes on what is intentionally excluded in the first pass.

Key design choices:
  1. What is state vs derived?
     → Option A: DIC + alkalinity are state; CO2, HCO3, CO3, pH are derived each tick via equilibrium.
     → Option B: CO2(aq) + alkalinity are state; DIC and pH are derived.
     → Recommendation: Option A (DIC + alk as state) because DIC is already a state variable and alkalinity is already tracked.
  2. Solver approach?
     → Iterative: Newton-Raphson on the charge-balance equation.
     → Analytical: closed-form for CO2-only system (no other weak acids).
     → Recommendation: analytical closed-form using DIC, alkalinity, and temperature-adjusted pKa values. Sufficient for freshwater aquarium range.
  3. Temperature dependence of equilibrium constants?
     → Yes, at least for pKa1 (CO2 ↔ HCO3-). pKa2 matters less in the 6-8 pH range.

Likely touch points:
- crates/tank_core/src/systems/chemistry.rs
- crates/tank_core/src/types/water.rs
- crates/tank_core/src/types/source_water.rs

## Acceptance Criteria


Acceptance Criteria:
- the design is specific enough that implementation work can proceed without re-litigating fundamentals
- update order and ownership boundaries are clear
- simplifications are explicit rather than hidden

Dependencies:
  -> tanksim-6e5.2.1 (blocks) - Decide internal unit taxonomy and display policy
  -> tanksim-6e5.1.1 (blocks) - Inventory current scientific semantics, units, invariants, and shortcuts
  -> tanksim-6e5.4 (parent-child) - Phase 2 — carbonate chemistry, CO2 exchange, and pH realism

Dependents:
  <- tanksim-6e5.4.2 (blocks) - Implement explicit carbonate equilibrium and pH solver

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- Carbonate chemistry can sprawl. This bead keeps the project honest about scope while still replacing the current oversimplification.
- Decide the minimum viable mechanistic model that gives better source-water differentiation and day/night pH behavior.
- Write down numeric expectations early so later tuning/debugging has a reference.

Scientific reference:
- Carbonate equilibrium in freshwater:
  CO2(aq) + H2O ↔ H2CO3 ↔ H+ + HCO3-    (pKa1 ≈ 6.35 at 25°C)
  HCO3- ↔ H+ + CO3--                       (pKa2 ≈ 10.33 at 25°C)
- In the pH 6-8 range typical of aquaria, HCO3- dominates. CO3-- is negligible below pH 8.
- Alkalinity ≈ [HCO3-] + 2[CO3--] - [H+] + [OH-]. For aquarium pH ranges, alkalinity ≈ [HCO3-].
- Given DIC and alkalinity, pH can be solved analytically from the charge balance.
- Temperature corrections: pKa1(T) = 3404.71/(T+273.15) + 0.032786*(T+273.15) - 14.8435 (Harned & Davis 1943). Linear approximation: pKa1(T) ≈ 6.352 - 0.0055*(T-25), good to ~±0.03 pK units across 15-35°C.
  [2026-03-26 17:06 UTC] reviewer: Priority fix: P1 → P0.

Rationale: D1 (this spike) blocks D2 (implement solver) which is P0. A P1 spike blocking P0 implementation is a priority inversion — the spike must complete before the solver can be designed. Promoting to P0 makes the priority chain consistent.
