//! Debug hooks and test helpers for budget inspection.
//!
//! This module provides ergonomic utilities for tests and interactive debugging
//! of the per-tick mass budget tracking system. It bridges the raw budget ledger
//! (see [`BudgetLedger`]) and higher-level assertions that test authors need.
//!
//! # Quick start
//!
//! ```rust,ignore
//! use tank_core::budget_helpers::{step_and_inspect, assert_n_conserved, assert_c_conserved};
//!
//! let mut engine = Engine::from_parts(state, vec![]);
//! let result = step_and_inspect(&mut engine, 24)?;
//!
//! // One-liner conservation checks:
//! assert_n_conserved(&result.budget, 1e-6);
//! assert_c_conserved(&result.budget, 1e-6);
//!
//! // Inspect which system moved nitrogen:
//! for entry in result.budget.system_deltas(Element::Nitrogen) {
//!     println!("{}: {:+.6} mg N", entry.label, entry.net_mg);
//! }
//! ```
//!
//! # Debug output
//!
//! Set `TANK_BUDGET_DEBUG=1` to print per-tick budget summaries to stderr
//! without modifying test code or assertions. The output identifies the system
//! responsible for the largest budget movement per element per tick.

use crate::{
    engine::Engine,
    types::{
        BudgetDelta, BudgetEntry, BudgetLedger, BudgetTotals, ElementBudget, SimError,
        TankSnapshot, TankState, TickBudgetRecord,
    },
    SimulationEngine,
};

/// Which element to query or assert on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Element {
    Nitrogen,
    Carbon,
    Oxygen,
}

/// Per-system delta for a single element, returned by
/// [`InspectionResult::system_deltas`].
#[derive(Debug, Clone)]
pub struct SystemElementDelta {
    pub label: String,
    pub in_mg: f64,
    pub out_mg: f64,
    pub net_mg: f64,
}

/// Result of [`step_and_inspect`]: the final state plus the budget ledger
/// covering every tick that was stepped.
pub struct InspectionResult {
    pub snapshot: TankSnapshot,
    pub budget: BudgetInspector,
}

/// Wrapper around a [`BudgetLedger`] providing ergonomic query methods.
pub struct BudgetInspector {
    pub ledger: BudgetLedger,
    pub before_totals: BudgetTotals,
    pub after_totals: BudgetTotals,
}

impl BudgetInspector {
    /// Overall net change for a given element across all inspected ticks.
    pub fn net_delta(&self, element: Element) -> f64 {
        match element {
            Element::Nitrogen => self.after_totals.nitrogen_mg - self.before_totals.nitrogen_mg,
            Element::Carbon => self.after_totals.carbon_mg - self.before_totals.carbon_mg,
            Element::Oxygen => self.after_totals.oxygen_mg - self.before_totals.oxygen_mg,
        }
    }

    /// Per-system deltas for a given element, aggregated across all ticks.
    ///
    /// Returns one entry per unique system/action label with totalled in/out/net.
    pub fn system_deltas(&self, element: Element) -> Vec<SystemElementDelta> {
        let mut map: std::collections::BTreeMap<String, (f64, f64)> =
            std::collections::BTreeMap::new();
        for tick in &self.ledger.ticks {
            for entry in &tick.entries {
                let eb = select_element(&entry.delta, element);
                let acc = map.entry(entry.label.clone()).or_insert((0.0, 0.0));
                acc.0 += eb.in_mg;
                acc.1 += eb.out_mg;
            }
        }
        map.into_iter()
            .map(|(label, (in_mg, out_mg))| SystemElementDelta {
                label,
                in_mg,
                out_mg,
                net_mg: in_mg - out_mg,
            })
            .collect()
    }

    /// Per-tick net deltas for a given element.
    pub fn per_tick_net(&self, element: Element) -> Vec<f64> {
        self.ledger
            .ticks
            .iter()
            .map(|tick| select_element(&tick.net_delta, element).net_mg())
            .collect()
    }

    /// Find the system with the largest absolute net delta for a given element
    /// across all ticks. Useful for locating the source of a conservation bug.
    pub fn largest_mover(&self, element: Element) -> Option<SystemElementDelta> {
        self.system_deltas(element)
            .into_iter()
            .max_by(|a, b| a.net_mg.abs().partial_cmp(&b.net_mg.abs()).unwrap())
    }

    /// Access the underlying tick records directly.
    pub fn ticks(&self) -> &[TickBudgetRecord] {
        &self.ledger.ticks
    }

    /// Look up a specific system's delta for a given element in a specific tick.
    pub fn system_delta_in_tick(
        &self,
        tick_index: usize,
        label: &str,
        element: Element,
    ) -> Option<SystemElementDelta> {
        self.ledger.ticks.get(tick_index).and_then(|tick| {
            tick.entries
                .iter()
                .find(|e| e.label == label)
                .map(|entry| {
                    let eb = select_element(&entry.delta, element);
                    SystemElementDelta {
                        label: entry.label.clone(),
                        in_mg: eb.in_mg,
                        out_mg: eb.out_mg,
                        net_mg: eb.net_mg(),
                    }
                })
        })
    }
}

// ---------------------------------------------------------------------------
// Core step-and-inspect function
// ---------------------------------------------------------------------------

/// Step the engine for `hours` ticks with budget tracking enabled, returning
/// both the final state snapshot and a [`BudgetInspector`] covering those ticks.
///
/// Budget tracking is enabled before stepping and left enabled afterward so
/// callers can continue inspecting if needed.
pub fn step_and_inspect(
    engine: &mut Engine,
    hours: u32,
) -> Result<InspectionResult, SimError> {
    engine.enable_budget_tracking();
    let before_totals = engine.full_state().budget_totals();
    let tick_offset = engine
        .budget_ledger()
        .map_or(0, |l| l.ticks.len());

    engine.step_hours(hours)?;

    let after_totals = engine.full_state().budget_totals();
    let ledger = engine.budget_ledger().expect("budget tracking was enabled");
    let new_ticks = ledger.ticks[tick_offset..].to_vec();

    maybe_print_debug(&new_ticks);

    Ok(InspectionResult {
        snapshot: engine.snapshot(),
        budget: BudgetInspector {
            ledger: BudgetLedger { ticks: new_ticks },
            before_totals,
            after_totals,
        },
    })
}

// ---------------------------------------------------------------------------
// Assertion API — single-call conservation checks
// ---------------------------------------------------------------------------

/// Assert that total nitrogen is conserved (net delta within `tolerance_mg`).
///
/// Panics with a diagnostic message identifying the largest mover if the
/// assertion fails.
pub fn assert_n_conserved(budget: &BudgetInspector, tolerance_mg: f64) {
    assert_budget_balanced(budget, Element::Nitrogen, tolerance_mg);
}

/// Assert that total carbon is conserved (net delta within `tolerance_mg`).
///
/// Panics with a diagnostic message identifying the largest mover if the
/// assertion fails.
pub fn assert_c_conserved(budget: &BudgetInspector, tolerance_mg: f64) {
    assert_budget_balanced(budget, Element::Carbon, tolerance_mg);
}

/// Assert that the O2 demand balance is within `tolerance_mg`.
///
/// This checks that the net oxygen delta across all ticks stays within the
/// given tolerance. For closed systems (no aeration exchange), this should
/// be near zero.
pub fn assert_o2_balanced(budget: &BudgetInspector, tolerance_mg: f64) {
    assert_budget_balanced(budget, Element::Oxygen, tolerance_mg);
}

/// Generic per-element conservation assertion.
///
/// Checks that the absolute net delta for `element` across all inspected ticks
/// is within `tolerance_mg`. On failure, the panic message includes:
/// - the actual net delta
/// - the system with the largest absolute contribution
/// - per-tick breakdown
pub fn assert_budget_balanced(budget: &BudgetInspector, element: Element, tolerance_mg: f64) {
    let net = budget.net_delta(element);
    if net.abs() <= tolerance_mg {
        return;
    }
    let largest = budget.largest_mover(element);
    let per_tick = budget.per_tick_net(element);
    let worst_tick = per_tick
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap());
    let element_name = match element {
        Element::Nitrogen => "nitrogen",
        Element::Carbon => "carbon",
        Element::Oxygen => "oxygen",
    };

    let mut msg = format!(
        "{element_name} budget imbalance: net delta = {net:+.9} mg (tolerance = {tolerance_mg:.9} mg)\n"
    );
    if let Some(mover) = &largest {
        msg.push_str(&format!(
            "  largest mover: {} ({:+.9} mg net, in={:.9} out={:.9})\n",
            mover.label, mover.net_mg, mover.in_mg, mover.out_mg
        ));
    }
    if let Some((idx, val)) = worst_tick {
        msg.push_str(&format!("  worst tick: index {idx} (net {val:+.9} mg)\n"));
    }
    msg.push_str(&format!(
        "  before: {:.6} mg, after: {:.6} mg\n",
        match element {
            Element::Nitrogen => budget.before_totals.nitrogen_mg,
            Element::Carbon => budget.before_totals.carbon_mg,
            Element::Oxygen => budget.before_totals.oxygen_mg,
        },
        match element {
            Element::Nitrogen => budget.after_totals.nitrogen_mg,
            Element::Carbon => budget.after_totals.carbon_mg,
            Element::Oxygen => budget.after_totals.oxygen_mg,
        }
    ));
    msg.push_str("  all systems:\n");
    for sd in budget.system_deltas(element) {
        msg.push_str(&format!(
            "    {}: {:+.9} mg (in={:.9} out={:.9})\n",
            sd.label, sd.net_mg, sd.in_mg, sd.out_mg
        ));
    }
    panic!("{msg}");
}

/// Assert that every tick individually has its element net delta within
/// `tolerance_mg`. Useful for catching single-tick spikes that cancel out
/// across a multi-tick run.
pub fn assert_per_tick_balanced(
    budget: &BudgetInspector,
    element: Element,
    tolerance_mg: f64,
) {
    for (i, net) in budget.per_tick_net(element).iter().enumerate() {
        if net.abs() > tolerance_mg {
            let element_name = match element {
                Element::Nitrogen => "nitrogen",
                Element::Carbon => "carbon",
                Element::Oxygen => "oxygen",
            };
            let tick = &budget.ledger.ticks[i];
            let mut msg = format!(
                "{element_name} per-tick imbalance at tick {i} (day {}, hour {}): \
                 net = {net:+.9} mg (tolerance = {tolerance_mg:.9} mg)\n  stages:\n",
                tick.day, tick.hour
            );
            for entry in &tick.entries {
                let eb = select_element(&entry.delta, element);
                msg.push_str(&format!(
                    "    {}: {:+.9} mg (in={:.9} out={:.9})\n",
                    entry.label,
                    eb.net_mg(),
                    eb.in_mg,
                    eb.out_mg
                ));
            }
            panic!("{msg}");
        }
    }
}

// ---------------------------------------------------------------------------
// Debug output (TANK_BUDGET_DEBUG=1)
// ---------------------------------------------------------------------------

fn debug_enabled() -> bool {
    std::env::var("TANK_BUDGET_DEBUG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn maybe_print_debug(ticks: &[TickBudgetRecord]) {
    if !debug_enabled() {
        return;
    }
    for tick in ticks {
        print_tick_debug(tick);
    }
}

fn print_tick_debug(tick: &TickBudgetRecord) {
    eprintln!(
        "--- BUDGET tick={} day={} hour={} ---",
        tick.tick_index, tick.day, tick.hour
    );
    eprintln!(
        "  totals before: N={:.6} C={:.6} O2={:.6}",
        tick.before.nitrogen_mg, tick.before.carbon_mg, tick.before.oxygen_mg
    );
    eprintln!(
        "  totals after:  N={:.6} C={:.6} O2={:.6}",
        tick.after.nitrogen_mg, tick.after.carbon_mg, tick.after.oxygen_mg
    );
    eprintln!(
        "  net delta:     N={:+.6} C={:+.6} O2={:+.6}",
        tick.net_delta.nitrogen.net_mg(),
        tick.net_delta.carbon.net_mg(),
        tick.net_delta.oxygen.net_mg()
    );
    for entry in &tick.entries {
        let n = &entry.delta.nitrogen;
        let c = &entry.delta.carbon;
        let o = &entry.delta.oxygen;
        if n.net_mg().abs() > 1e-12 || c.net_mg().abs() > 1e-12 || o.net_mg().abs() > 1e-12 {
            eprintln!(
                "  {:<35} N={:+.6} C={:+.6} O2={:+.6}",
                entry.label,
                n.net_mg(),
                c.net_mg(),
                o.net_mg()
            );
        }
    }
    // Identify largest movers per element
    if let Some(n_max) = largest_mover_in_tick(tick, Element::Nitrogen) {
        eprintln!("  >> largest N mover: {} ({:+.6} mg)", n_max.0, n_max.1);
    }
    if let Some(c_max) = largest_mover_in_tick(tick, Element::Carbon) {
        eprintln!("  >> largest C mover: {} ({:+.6} mg)", c_max.0, c_max.1);
    }
    if let Some(o_max) = largest_mover_in_tick(tick, Element::Oxygen) {
        eprintln!("  >> largest O2 mover: {} ({:+.6} mg)", o_max.0, o_max.1);
    }
}

fn largest_mover_in_tick(tick: &TickBudgetRecord, element: Element) -> Option<(String, f64)> {
    tick.entries
        .iter()
        .map(|e| {
            let net = select_element(&e.delta, element).net_mg();
            (e.label.clone(), net)
        })
        .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
        .filter(|(_, net)| net.abs() > 1e-12)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn select_element(delta: &BudgetDelta, element: Element) -> ElementBudget {
    match element {
        Element::Nitrogen => delta.nitrogen,
        Element::Carbon => delta.carbon,
        Element::Oxygen => delta.oxygen,
    }
}
