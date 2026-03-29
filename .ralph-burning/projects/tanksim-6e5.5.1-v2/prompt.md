○ tanksim-6e5.5.1 · Introduce habitat registry and colonizable-area model   [● P1 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: architecture, geometry, habitats, phase-3, scientific-core

Context:
- The current simulation tracks a tank, substrate, and filter, but not a general habitat model for surfaces and zones.
- To make biofilms, periphyton, redox, and geometry matter for the right reasons, the engine needs a first-class habitat registry.

Deliverable:
- Define a compact habitat registry in engine state/save data covering at least filter media, glass/hardscape, plant surfaces, substrate surface, and deeper substrate.
- Compute colonizable area for each habitat from existing geometry, hardware, and plant state (including configurable hardscape/media inputs) instead of hardcoding a single generic surface.
- Assign flow, oxygen, and light exposure modifiers in the habitat registry so downstream systems have one canonical place to read these mechanics.
- Make the modifiers responsive to relevant scenario inputs where appropriate (for example filter flow, depth/light context, rooted plant presence) while keeping the model ecological rather than graphical.
- Keep the abstraction small and inspectable so later habitatized systems can build on it without redesigning state yet again.

Target habitats (minimal set):
  FilterMedia
  GlassHardscape
  PlantSurfaces
  SubstrateSurface
  SubstrateDeep

Likely touch points:
- crates/tank_core/src/types/state.rs
- crates/tank_core/src/types/geometry.rs
- crates/tank_core/src/types/substrate.rs
- supporting system modules

## Acceptance Criteria


Acceptance Criteria:
- habitats are explicit serialized engine concepts with documented purpose and minimal required metadata.
- colonizable areas are derived from geometry, hardware, and plant state rather than hardcoded constants, and plant-driven areas update when biomass changes.
- flow, oxygen, and light exposure modifiers are documented, bounded in [0,1], and respond to the relevant scenario inputs for each habitat.
- representative tests cover registry serialization, area computation, and modifier sensitivity/bounds so later habitatized systems can build on this without redefining the abstraction.

Dependencies:
  -> tanksim-6e5.2.1 (blocks) - Decide internal unit taxonomy and display policy
  -> tanksim-6e5.1.2.3 (blocks) - Add save-schema versioning and migration scaffolding
  -> tanksim-6e5.1.1 (blocks) - Inventory current scientific semantics, units, invariants, and shortcuts
  -> tanksim-6e5.5 (parent-child) - Phase 3 — habitatized ecology, substrate redox, and geometry-aware scaling

Dependents:
  <- tanksim-6e5.5.6 (blocks) - Scale equipment, plant mass, and stocking defaults with tank geometry
  <- tanksim-6e5.5.5 (blocks) - Add depth/turbidity light attenuation and habitat-specific light exposure
  <- tanksim-6e5.5.4.3 (blocks) - Add root-zone oxygenation hooks for rooted plants
  <- tanksim-6e5.5.4 (blocks) - Implement substrate oxic/suboxic layers, denitrification, and root-zone redox hooks
  <- tanksim-6e5.5.3 (blocks) - Split periphyton and decomposer pools by habitat
  <- tanksim-6e5.5.2 (blocks) - Scale biofilter carrying capacity with habitat, media, flow, and oxygen

Comments:
  [2026-03-26 20:44 UTC] master: Canonicalization note: merged former E1a-E1c into E1. Habitat types, area computation, and exposure modifiers form one reusable registry contract, while the downstream habitat consumers remain separate.
  [2026-03-26 13:46 UTC] backlog-planner: Rationale:
- "Where things live" is the key missing abstraction for the next scientific step.
- Keep habitats ecological rather than graphical. The goal is not spatial rendering; it is mechanistic differentiation.
- Start with a small fixed set of habitats and only generalize further if later work truly needs it.

Design principle:
- A habitat is a surface or zone where biofilm can colonize, decomposition can occur, or redox conditions can differ from the bulk water column.
- Each habitat knows: its colonizable area (cm²), its flow exposure (normalized 0-1), its oxygen exposure (normalized 0-1), and optionally its light exposure.
- Biomass pools (nitrifiers, decomposers, periphyton) will later be indexed by habitat. For now, the registry just provides the geometry and modifiers.
