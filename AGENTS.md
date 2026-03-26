# AGENTS.md

## Issue Tracking

**Note:** `br` is non-invasive and never executes git commands. After `br sync --flush-only`, you must manually run `git add .beads/ && git commit`.

This repository uses `br` (beads_rust) and `bv` for issue tracking and backlog inspection.

- The authoritative backlog lives in `.beads/issues.jsonl`.
- Read task context from the beads themselves with `br show <id>`.
- Use `br ready` to find the next unblocked task.
- Use `br list --pretty` or `bv` to inspect the backlog more broadly.
- Use `br dep graph` when dependency shape matters.
- Keep task knowledge in the beads. Do not recreate parallel backlog scripts or mirror plan docs unless explicitly requested.

Common commands:

```bash
br ready
br show <id>
br list --pretty
br dep graph
bv
br sync --flush-only
git add .beads/
git commit -m "sync beads"
```

## Working Norms

- Follow the current bead descriptions and comments; they contain the roadmap, assumptions, testing expectations, and deferred-scope notes.
- Keep tests deterministic and add right-sized coverage for any code change: unit tests for local logic, integration tests for cross-system behavior, and scenario/e2e coverage when long-horizon behavior changes.
- Prefer explicit, inspectable behavior over hidden tuning. If diagnostics or tracing are needed, wire them in cleanly rather than relying on ad hoc prints.
- If a change affects persisted state or action semantics, update save/migration handling and the relevant bead context.
