use super::store::{InMemoryStore, ReconcileStore};
use crate::memo::constants::BASIS_POINTS;
use crate::memo::decode::decode_memo_v1;
use crate::types::{
    ExpectedPayment, MatchResult, MatchStatus, PaymentEvent, ReconcileReport, ReconcileSummary,
};
use crate::ReconcileError;

/// How basis-points tolerance is applied to partial payments.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ToleranceMode {
    /// Tolerance applies to the final cumulative total.
    #[default]
    Final,
    /// Tolerance applies per individual payment.
    Each,
}

/// Configuration for the Reconciler.
#[derive(Debug, Clone)]
pub struct ReconcilerOptions {
    /// Only accept memos whose issuer_tag matches this value.
    pub issuer_tag: Option<u64>,
    /// Require `event.from == expected.from` when `expected.from` is set.
    pub strict_sender: bool,
    /// Accept overpayments (amount > expected). Default: true.
    pub allow_overpayment: bool,
    /// Enable partial payment accumulation. Default: false.
    pub allow_partial: bool,
    /// Reject payments arriving after `due_at`. Default: false.
    pub reject_expired: bool,
    /// Tolerance in basis points (100 bps = 1%). Default: 0. Capped at 10_000 (100%).
    pub amount_tolerance_bps: u32,
    pub partial_tolerance_mode: ToleranceMode,
}

impl ReconcilerOptions {
    pub fn new() -> Self {
        Self {
            issuer_tag: None,
            strict_sender: false,
            allow_overpayment: true,
            allow_partial: false,
            reject_expired: false,
            amount_tolerance_bps: 0,
            partial_tolerance_mode: ToleranceMode::Final,
        }
    }
}

impl Default for ReconcilerOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Stateful matching engine that reconciles incoming payments against expected invoices.
///
/// # Example
/// ```
/// use tempo_reconcile::{Reconciler, ReconcilerOptions};
/// let mut r = Reconciler::new(ReconcilerOptions::new());
/// ```
pub struct Reconciler<S: ReconcileStore = InMemoryStore> {
    store: S,
    opts: ReconcilerOptions,
    /// Total number of payments registered via `expect()`, never decremented.
    expected_count: usize,
    /// Sum of all amounts registered via `expect()`, never decremented.
    expected_total_amount: u128,
}

impl Reconciler<InMemoryStore> {
    /// Create a Reconciler with an in-memory store.
    pub fn new(opts: ReconcilerOptions) -> Self {
        Self {
            store: InMemoryStore::new(),
            opts,
            expected_count: 0,
            expected_total_amount: 0,
        }
    }
}

impl<S: ReconcileStore> Reconciler<S> {
    /// Create a Reconciler with a custom store.
    pub fn with_store(store: S, opts: ReconcilerOptions) -> Self {
        Self {
            store,
            opts,
            expected_count: 0,
            expected_total_amount: 0,
        }
    }

    /// Register an expected payment.
    ///
    /// # Errors
    /// Returns [`ReconcileError::DuplicateExpected`] if `memo_raw` is already registered.
    pub fn expect(&mut self, payment: ExpectedPayment) -> Result<(), ReconcileError> {
        let amount = payment.amount;
        self.store.add_expected(payment)?;
        self.expected_count += 1;
        self.expected_total_amount += amount;
        Ok(())
    }

    /// Process one payment event. Returns a [`MatchResult`].
    ///
    /// Idempotent: the same `(tx_hash, log_index)` always returns the cached result.
    pub fn ingest(&mut self, event: PaymentEvent) -> MatchResult {
        let event_key = format!("{}:{}", event.tx_hash.to_ascii_lowercase(), event.log_index);

        // Idempotency: return cached result for duplicate events.
        if let Some(cached) = self.store.get_result(&event_key) {
            // Clone the cached result — requires MatchResult: Clone
            return cached.clone();
        }

        let result = self.match_event(&event);
        self.store.add_result(&event_key, result.clone());
        result
    }

    /// Process multiple events in batch order.
    pub fn ingest_many(&mut self, events: Vec<PaymentEvent>) -> Vec<MatchResult> {
        events.into_iter().map(|e| self.ingest(e)).collect()
    }

    /// Generate a full reconciliation report.
    pub fn report(&self) -> ReconcileReport {
        let all_results = self.store.get_all_results();
        let pending: Vec<ExpectedPayment> =
            self.store.get_all_expected().into_iter().cloned().collect();

        let mut matched = Vec::new();
        let mut issues = Vec::new();
        let mut summary = ReconcileSummary {
            // Use the monotonically increasing counters — correct even after remove_expected().
            total_expected: self.expected_count,
            total_expected_amount: self.expected_total_amount,
            total_received: all_results.len(),
            pending_count: pending.len(),
            ..ReconcileSummary::default()
        };

        for r in &all_results {
            summary.total_received_amount += r.payment.amount;
            match r.status {
                MatchStatus::Matched => {
                    summary.matched_count += 1;
                    summary.total_matched_amount += r
                        .expected
                        .as_ref()
                        .map(|e| e.amount)
                        .unwrap_or(r.payment.amount);
                    matched.push((*r).clone());
                }
                MatchStatus::Partial => {
                    summary.partial_count += 1;
                    issues.push((*r).clone());
                }
                MatchStatus::UnknownMemo => {
                    summary.unknown_memo_count += 1;
                    issues.push((*r).clone());
                }
                MatchStatus::NoMemo => {
                    summary.no_memo_count += 1;
                    issues.push((*r).clone());
                }
                MatchStatus::MismatchAmount => {
                    summary.mismatch_amount_count += 1;
                    issues.push((*r).clone());
                }
                MatchStatus::MismatchToken => {
                    summary.mismatch_token_count += 1;
                    issues.push((*r).clone());
                }
                MatchStatus::MismatchParty => {
                    summary.mismatch_party_count += 1;
                    issues.push((*r).clone());
                }
                MatchStatus::Expired => {
                    summary.expired_count += 1;
                    issues.push((*r).clone());
                }
            }
        }

        summary.issue_count = issues.len();

        ReconcileReport {
            matched,
            issues,
            pending,
            summary,
        }
    }

    /// Remove an expected payment (e.g. cancelled invoice).
    ///
    /// Also clears any accumulated partial payments for this memo,
    /// so re-registering it starts from zero.
    pub fn remove_expected(&mut self, memo_raw: &str) -> bool {
        self.store.remove_partial(memo_raw);
        self.store.remove_expected(memo_raw)
    }

    /// Clear all state: expected payments, match results, partials, and counters.
    pub fn reset(&mut self) {
        self.store.clear();
        self.expected_count = 0;
        self.expected_total_amount = 0;
    }

    /// Number of payments registered via `expect()` (monotonic, not decremented by remove).
    pub fn expected_count(&self) -> usize {
        self.expected_count
    }

    /// Sum of amounts registered via `expect()` (monotonic, not decremented by remove).
    pub fn expected_total_amount(&self) -> u128 {
        self.expected_total_amount
    }

    /// Number of cached match results (idempotency cache).
    pub fn result_count(&self) -> usize {
        self.store.get_all_results().len()
    }

    /// Number of expected payments still pending (not yet matched or removed).
    pub fn pending_count(&self) -> usize {
        self.store.get_all_expected().len()
    }

    /// Read-only access to the reconciler options.
    pub fn options(&self) -> &ReconcilerOptions {
        &self.opts
    }

    // --- internal ---

    fn match_event(&mut self, event: &PaymentEvent) -> MatchResult {
        // No memo → no_memo immediately.
        let memo_raw = match &event.memo_raw {
            Some(m) => m.clone(),
            None => {
                return MatchResult {
                    status: MatchStatus::NoMemo,
                    payment: event.clone(),
                    expected: None,
                    reason: Some("no memo field on transfer".to_string()),
                    overpaid_by: None,
                    remaining_amount: None,
                    is_late: None,
                };
            }
        };

        // issuerTag filter: decode v1 memo, check tag if filter is set.
        if let Some(filter_tag) = self.opts.issuer_tag {
            match decode_memo_v1(&memo_raw) {
                Some(m) if m.issuer_tag == filter_tag => {}
                _ => {
                    return MatchResult {
                        status: MatchStatus::UnknownMemo,
                        payment: event.clone(),
                        expected: None,
                        reason: Some("memo issuerTag does not match filter".to_string()),
                        overpaid_by: None,
                        remaining_amount: None,
                        is_late: None,
                    };
                }
            }
        }

        // Look up expected payment by memo_raw.
        let expected = match self.store.get_expected(&memo_raw).cloned() {
            Some(e) => e,
            None => {
                return MatchResult {
                    status: MatchStatus::UnknownMemo,
                    payment: event.clone(),
                    expected: None,
                    reason: Some("memo not in expected list".to_string()),
                    overpaid_by: None,
                    remaining_amount: None,
                    is_late: None,
                };
            }
        };

        // Token check.
        if !event.token.eq_ignore_ascii_case(&expected.token) {
            return MatchResult {
                status: MatchStatus::MismatchToken,
                payment: event.clone(),
                expected: Some(expected),
                reason: Some("token address mismatch".to_string()),
                overpaid_by: None,
                remaining_amount: None,
                is_late: None,
            };
        }

        // Recipient check.
        if !event.to.eq_ignore_ascii_case(&expected.to) {
            return MatchResult {
                status: MatchStatus::MismatchParty,
                payment: event.clone(),
                expected: Some(expected),
                reason: Some("recipient address mismatch".to_string()),
                overpaid_by: None,
                remaining_amount: None,
                is_late: None,
            };
        }

        // Sender check (if strictSender and expected.from is set).
        if self.opts.strict_sender {
            if let Some(ref exp_from) = expected.from {
                if !event.from.eq_ignore_ascii_case(exp_from) {
                    return MatchResult {
                        status: MatchStatus::MismatchParty,
                        payment: event.clone(),
                        expected: Some(expected),
                        reason: Some("sender address mismatch (strictSender)".to_string()),
                        overpaid_by: None,
                        remaining_amount: None,
                        is_late: None,
                    };
                }
            }
        }

        // Expiry check.
        let is_late = if let (Some(ts), Some(due)) = (event.timestamp, expected.due_at) {
            Some(ts > due)
        } else {
            None
        };

        if self.opts.reject_expired && is_late == Some(true) {
            return MatchResult {
                status: MatchStatus::Expired,
                payment: event.clone(),
                expected: Some(expected),
                reason: Some("payment arrived after due_at".to_string()),
                overpaid_by: None,
                remaining_amount: None,
                is_late,
            };
        }

        // Amount check.
        let bps = self.opts.amount_tolerance_bps.min(10_000) as u128;
        let tolerance = tolerance_from_bps(expected.amount, bps);
        let min_acceptable = expected.amount.saturating_sub(tolerance);

        if event.amount < expected.amount {
            let underpaid_by = expected.amount - event.amount;

            if self.opts.allow_partial && self.opts.partial_tolerance_mode == ToleranceMode::Each {
                // "each" mode: tolerance applies per individual payment.
                // Beyond tolerance → mismatch_amount. Within tolerance → fall through to matched.
                if underpaid_by > tolerance {
                    let reason = format!(
                        "underpaid: got {}, expected {}",
                        event.amount, expected.amount
                    );
                    return MatchResult {
                        status: MatchStatus::MismatchAmount,
                        payment: event.clone(),
                        expected: Some(expected),
                        reason: Some(reason),
                        overpaid_by: None,
                        remaining_amount: Some(underpaid_by),
                        is_late,
                    };
                }
                // Within tolerance in each-mode: treat as exact match, fall through to matched
            } else if event.amount < min_acceptable {
                // Below tolerance floor.
                if self.opts.allow_partial {
                    // "final" mode: accumulate partials, tolerance on cumulative total.
                    let cumulative = self.store.add_partial(&memo_raw, event.amount);
                    if cumulative >= min_acceptable {
                        // Accumulated enough — mark matched.
                        self.store.remove_expected(&memo_raw);
                        self.store.remove_partial(&memo_raw);
                        let overpaid_by = if cumulative > expected.amount {
                            Some(cumulative - expected.amount)
                        } else {
                            None
                        };
                        return MatchResult {
                            status: MatchStatus::Matched,
                            payment: event.clone(),
                            expected: Some(expected),
                            reason: None,
                            overpaid_by,
                            remaining_amount: None,
                            is_late,
                        };
                    }
                    // Still partial.
                    let remaining = expected.amount.saturating_sub(cumulative);
                    let reason = format!(
                        "partial: accumulated {}, need {}",
                        cumulative, expected.amount
                    );
                    return MatchResult {
                        status: MatchStatus::Partial,
                        payment: event.clone(),
                        expected: Some(expected),
                        reason: Some(reason),
                        overpaid_by: None,
                        remaining_amount: Some(remaining),
                        is_late,
                    };
                }
                // No partial: mismatch_amount
                let reason = format!(
                    "underpaid: got {}, expected {}",
                    event.amount, expected.amount
                );
                return MatchResult {
                    status: MatchStatus::MismatchAmount,
                    payment: event.clone(),
                    expected: Some(expected),
                    reason: Some(reason),
                    overpaid_by: None,
                    remaining_amount: Some(underpaid_by),
                    is_late,
                };
            }
            // Within tolerance (non-partial case or each-mode within tolerance)
            // → fall through to matched path below
        }

        // Overpayment check.
        let overpaid_by = if event.amount > expected.amount {
            Some(event.amount - expected.amount)
        } else {
            None
        };

        if overpaid_by.is_some() && !self.opts.allow_overpayment {
            let reason = format!(
                "overpaid: got {}, expected {}",
                event.amount, expected.amount
            );
            return MatchResult {
                status: MatchStatus::MismatchAmount,
                payment: event.clone(),
                expected: Some(expected),
                reason: Some(reason),
                overpaid_by,
                remaining_amount: None,
                is_late,
            };
        }

        // Matched!
        self.store.remove_expected(&memo_raw);
        MatchResult {
            status: MatchStatus::Matched,
            payment: event.clone(),
            expected: Some(expected),
            reason: None,
            overpaid_by,
            remaining_amount: None,
            is_late,
        }
    }
}

/// Compute basis-point tolerance without intermediate overflow.
///
/// Equivalent to `⌈amount * bps / 10_000⌉` but avoids overflow when `amount`
/// is near `u128::MAX` by decomposing the multiplication:
///   a * b / c == (a / c) * b + (a % c) * b / c
/// where c = BASIS_POINTS = 10_000.
fn tolerance_from_bps(amount: u128, bps: u128) -> u128 {
    if bps == 0 {
        return 0;
    }
    let q = amount / BASIS_POINTS;
    let r = amount % BASIS_POINTS;
    // r < 10_000, bps ≤ 10_000 → r * bps ≤ 99_990_000 (no overflow)
    let tol = q * bps + r * bps / BASIS_POINTS;
    let rem = (r * bps) % BASIS_POINTS;
    tol + (rem != 0) as u128
}
