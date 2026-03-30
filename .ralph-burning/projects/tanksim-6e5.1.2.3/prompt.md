○ tanksim-6e5.1.2.3 · Add save-schema versioning and migration scaffolding   [● P0 · OPEN]
Owner: master · Type: task
Created: 2026-03-26 · Updated: 2026-03-27
Labels: migration, phase-0, save, scientific-core

Context:
- save.rs already serializes TankState inside SaveFile and tags the file with schema_version. The next phases will add fields (carbonate state in D2, habitat state in E1, richer shrimp state in F2) and rename fields (nitrogen pool names in B7).
- Without migration scaffolding, each change risks breaking saves or requiring manual re-creation.

Deliverable:
- A migration registry or strategy in save.rs that maps version N → version N+1 state transforms.
- At minimum: detect version mismatch, apply known migrations in order, fail gracefully for unknown future versions.
- Document the migration contract so each later phase knows how to register its state changes.

Likely touch points:
- crates/tank_core/src/save.rs
- crates/tank_core/src/types/state.rs (version constant)

## Acceptance Criteria


Acceptance Criteria:
- a test can save at version N, bump version, add a field, and load with the migration applied
- unknown version → clear error message (not silent data loss)
- the migration path is documented for later contributors

Dependencies:
  -> tanksim-6e5.1.2 (parent-child) - Add conservation/debug instrumentation and save-schema migration scaffolding
  -> tanksim-6e5.1.1 (blocks) - Inventory current scientific semantics, units, invariants, and shortcuts

Dependents:
  <- tanksim-6e5.4.2 (blocks) - Implement explicit carbonate equilibrium and pH solver
  <- tanksim-6e5.6.2 (blocks) - Implement stage- or size-structured shrimp population dynamics
  <- tanksim-6e5.5.1 (blocks) - Introduce habitat registry and colonizable-area model
  <- tanksim-6e5.3.5 (blocks) - Split plant trimming into export vs leave-cuttings actions

Comments:
  [2026-03-26 13:46 UTC] backlog-planner: Future-self notes:
- The migration system doesn't need to handle arbitrary schema evolution. It needs to handle the specific field additions and renames planned for phases 1-4.
- Consider serde's #[serde(default)] for new fields and #[serde(alias)] for renames as the simplest migration strategy.
- More complex migrations (splitting a field into multiple, changing units) may need explicit transform functions per version bump.
- Keep the migration code in save.rs close to the serialization logic it modifies.
