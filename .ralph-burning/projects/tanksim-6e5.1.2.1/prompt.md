○ tanksim-6e5.1.2.1 · Implement per-tick mass budget tracking for N, C, and O2   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: instrumentation, phase-0, scientific-core, testing

Context:
- The core engine (engine.rs) steps through systems hourly. Each system modifies TankState fields (water pools, biology pools, detritus). There is currently no mechanism to verify that modifications within a closed tick are mass-conserving.

Deliverable:
- A BudgetLedger or equivalent that can record per-element (N, C, O2) sources and sinks during a tick.
- The ledger should capture: which system contributed what delta, and the net balance.
- Define canonical total_nitrogen() / total_carbon() helpers in one place so tests and diagnostics do not hand-roll partial pool lists.
- Focus on the major elements: total nitrogen must include every explicit N-bearing pool currently represented (including substrate nutrient stores), total carbon must include every explicit C-bearing pool currently represented, and both helpers must be extended when later phases add new explicit element-bearing state.
- Track oxygen demand in the same framework where feasible.

Likely touch points:
- crates/tank_core/src/engine.rs (step_hours, step loop)
- crates/tank_core/src/types/state.rs (new struct or field)
- crates/tank_core/src/invariants.rs (budget check after step)

## Acceptance Criteria


Acceptance Criteria:
- a test can run N ticks and inspect per-tick budget deltas
- closed-system ticks (no water change, no export) show near-zero net delta
- the instrumentation compiles out or is zero-cost when not in use

Dependencies:
  -> tanksim-6e5.1.2 (parent-child) - Add conservation/debug instrumentation and save-schema migration scaffolding

Dependents:
  <- tanksim-6e5.1.2.4 (blocks) - Add structured simulation tracing with per-system deltas and configurable verbosity
  <- tanksim-6e5.1.2.2 (blocks) - Add debug hooks and test helpers for budget inspection
  <- tanksim-6e5.4.7 (blocks) - Validate nitrification-driven alkalinity depletion with the carbonate solver, expose diagnostics, and prepare the denitrification return path
  <- tanksim-6e5.3.4 (blocks) - Audit feeding, detritus, DOC, and mineralization bookkeeping end to end

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Implementation guidance:
- The simplest approach: snapshot total-N and total-C before and after each tick, then assert delta ≈ 0 (within floating-point tolerance) for closed-system ticks.
- A more granular approach: each system function returns a BudgetDelta struct { n_in, n_out, c_in, c_out, o2_in, o2_out }, and the engine sums them.
- Start with the simple approach; upgrade to granular if debugging needs it.
- "Total N" means: ammonia_total + nitrite_total + nitrate_total + don_total + N_in_all_biomass_pools. Define this sum once as a helper.
  [2026-03-26 16:25 UTC] reviewer: Specific unit test requirements:

1. test_total_n_helper_sums_all_pools: total_nitrogen() = ammonia + nitrite + nitrate + DON + N_in(plants) + N_in(algae) + N_in(microbes) + N_in(shrimp_biomass) + N_in(detritus). Verify against hand-calculated sum for a known state.

2. test_total_c_helper_sums_all_pools: total_carbon() = DIC + DOC + C_in(plants) + C_in(algae) + C_in(microbes) + C_in(shrimp_biomass) + C_in(detritus). Same pattern.

3. test_closed_system_n_conservation: Run 24 ticks with no water change, no feed, no export. Assert |initial_total_N - final_total_N| < 1e-6 mg.

4. test_closed_system_c_conservation: Same for carbon.

5. test_water_change_n_export_tracked: Run a water change. Assert total_N decreased by the expected export amount. The budget ledger should show the water change as an explicit export, not as an unexplained loss.

6. test_budget_ledger_zero_cost_when_disabled: When budget tracking is off (normal gameplay), step_hours() has no additional allocations or I/O. Verify via #[cfg(test)] gating or runtime flag check.

NOTE: The total_nitrogen() and total_carbon() helper functions are themselves critical infrastructure. They must be defined in a single canonical location and include ALL pools. Missing a pool in these helpers would mask conservation bugs. Consider a compile-time or test-time check that all biomass-bearing state fields are included.

  [2026-03-26 16:40 UTC] reviewer: Correction on canonical conservation helpers:
- The reviewer test note was too narrow for nitrogen. The canonical total_nitrogen() helper must include every explicit N-bearing pool currently represented in TankState, including substrate_layers[*].nutrient_store_mg_n_total in addition to dissolved pools and any biomass/detritus terms modeled through shared composition helpers.
- Do not let tests hard-code a partial pool list that omits substrate N; rooted-plant and substrate/redox work would otherwise leak nitrogen without tripping the guardrail.
- For carbon, include every explicit C-bearing pool currently represented. If a later phase introduces an explicit substrate organic-C store, extend total_carbon() there too rather than leaving conservation logic frozen to today's state layout.

