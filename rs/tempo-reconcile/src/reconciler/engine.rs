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
    /// Tolerance in basis points (100 bps = 1%). Default: 0. Must be <= 10_000 (100%).
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
/// let mut r = Reconciler::new(ReconcilerOptions::new()).unwrap();
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
    ///
    /// # Errors
    /// Returns [`ReconcileError::InvalidToleranceBps`] if `amount_tolerance_bps > 10_000`.
    pub fn new(opts: ReconcilerOptions) -> Result<Self, ReconcileError> {
        if opts.amount_tolerance_bps > 10_000 {
            return Err(ReconcileError::InvalidToleranceBps(
                opts.amount_tolerance_bps,
            ));
        }
        Ok(Self {
            store: InMemoryStore::new(),
            opts,
            expected_count: 0,
            expected_total_amount: 0,
        })
    }
}

impl<S: ReconcileStore> Reconciler<S> {
    /// Create a Reconciler with a custom store.
    ///
    /// # Errors
    /// Returns [`ReconcileError::InvalidToleranceBps`] if `amount_tolerance_bps > 10_000`.
    pub fn with_store(store: S, opts: ReconcilerOptions) -> Result<Self, ReconcileError> {
        if opts.amount_tolerance_bps > 10_000 {
            return Err(ReconcileError::InvalidToleranceBps(
                opts.amount_tolerance_bps,
            ));
        }
        Ok(Self {
            store,
            opts,
            expected_count: 0,
            expected_total_amount: 0,
        })
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

        for r in all_results {
            summary.total_received_amount += r.payment.amount;
            match r.status {
                MatchStatus::Matched => {
                    summary.matched_count += 1;
                    summary.total_matched_amount += r
                        .expected
                        .as_ref()
                        .map(|e| e.amount)
                        .unwrap_or(r.payment.amount);
                    matched.push(r.clone());
                }
                MatchStatus::Partial => {
                    summary.partial_count += 1;
                    issues.push(r.clone());
                }
                MatchStatus::UnknownMemo => {
                    summary.unknown_memo_count += 1;
                    issues.push(r.clone());
                }
                MatchStatus::NoMemo => {
                    summary.no_memo_count += 1;
                    issues.push(r.clone());
                }
                MatchStatus::MismatchAmount => {
                    summary.mismatch_amount_count += 1;
                    issues.push(r.clone());
                }
                MatchStatus::MismatchToken => {
                    summary.mismatch_token_count += 1;
                    issues.push(r.clone());
                }
                MatchStatus::MismatchParty => {
                    summary.mismatch_party_count += 1;
                    issues.push(r.clone());
                }
                MatchStatus::Expired => {
                    summary.expired_count += 1;
                    issues.push(r.clone());
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

    fn match_event(&mut self, event: &PaymentEvent) -> MatchResult {
        let memo_raw = match self.check_memo(event) {
            Ok(m) => m,
            Err(result) => return result,
        };

        let expected = match self.store.get_expected(&memo_raw).cloned() {
            Some(e) => e,
            None => {
                return self.result(
                    event,
                    MatchStatus::UnknownMemo,
                    None,
                    "memo not in expected list",
                )
            }
        };

        if let Some(result) = self.check_counterparties(event, &expected) {
            return result;
        }

        let is_late = match (event.timestamp, expected.due_at) {
            (Some(ts), Some(due)) => Some(ts > due),
            _ => None,
        };
        if self.opts.reject_expired && is_late == Some(true) {
            return self.result_with(
                event,
                MatchStatus::Expired,
                Some(expected),
                "payment arrived after due_at",
                None,
                None,
                is_late,
            );
        }

        self.match_amount(event, expected, &memo_raw, is_late)
    }

    /// Validate memo presence and issuer tag filter. Returns memo_raw or early MatchResult.
    #[allow(clippy::result_large_err)]
    fn check_memo(&self, event: &PaymentEvent) -> Result<String, MatchResult> {
        let memo_raw = match &event.memo_raw {
            Some(m) => m.clone(),
            None => {
                return Err(self.result(
                    event,
                    MatchStatus::NoMemo,
                    None,
                    "no memo field on transfer",
                ))
            }
        };

        if let Some(filter_tag) = self.opts.issuer_tag {
            match decode_memo_v1(&memo_raw) {
                Some(m) if m.issuer_tag == filter_tag => {}
                _ => {
                    return Err(self.result(
                        event,
                        MatchStatus::UnknownMemo,
                        None,
                        "memo issuerTag does not match filter",
                    ))
                }
            }
        }

        Ok(memo_raw)
    }

    /// Verify token, recipient, and optional sender match. Returns Some(result) on mismatch.
    fn check_counterparties(
        &self,
        event: &PaymentEvent,
        expected: &ExpectedPayment,
    ) -> Option<MatchResult> {
        if !event.token.eq_ignore_ascii_case(&expected.token) {
            return Some(self.result(
                event,
                MatchStatus::MismatchToken,
                Some(expected.clone()),
                "token address mismatch",
            ));
        }

        if !event.to.eq_ignore_ascii_case(&expected.to) {
            return Some(self.result(
                event,
                MatchStatus::MismatchParty,
                Some(expected.clone()),
                "recipient address mismatch",
            ));
        }

        if self.opts.strict_sender {
            if let Some(ref exp_from) = expected.from {
                if !event.from.eq_ignore_ascii_case(exp_from) {
                    return Some(self.result(
                        event,
                        MatchStatus::MismatchParty,
                        Some(expected.clone()),
                        "sender address mismatch (strictSender)",
                    ));
                }
            }
        }

        None
    }

    /// Amount matching: tolerance, partial payments, overpayment.
    fn match_amount(
        &mut self,
        event: &PaymentEvent,
        expected: ExpectedPayment,
        memo_raw: &str,
        is_late: Option<bool>,
    ) -> MatchResult {
        if event.amount < expected.amount {
            if let Some(result) = self.handle_underpayment(event, &expected, memo_raw, is_late) {
                return result;
            }
            // Within tolerance → fall through to matched
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
            return self.result_with(
                event,
                MatchStatus::MismatchAmount,
                Some(expected),
                &reason,
                overpaid_by,
                None,
                is_late,
            );
        }

        // Matched!
        self.store.remove_expected(memo_raw);
        self.result_with(
            event,
            MatchStatus::Matched,
            Some(expected),
            "",
            overpaid_by,
            None,
            is_late,
        )
    }

    /// Handle underpaid amounts. Returns Some(result) if resolved, None to fall through to matched.
    fn handle_underpayment(
        &mut self,
        event: &PaymentEvent,
        expected: &ExpectedPayment,
        memo_raw: &str,
        is_late: Option<bool>,
    ) -> Option<MatchResult> {
        let bps = self.opts.amount_tolerance_bps as u128;
        let tolerance = tolerance_from_bps(expected.amount, bps);
        let min_acceptable = expected.amount.saturating_sub(tolerance);
        let underpaid_by = expected.amount - event.amount;

        if self.opts.allow_partial && self.opts.partial_tolerance_mode == ToleranceMode::Each {
            return self.handle_each_mode_underpayment(
                event,
                expected,
                underpaid_by,
                tolerance,
                is_late,
            );
        }

        if event.amount >= min_acceptable {
            return None; // within tolerance
        }

        if self.opts.allow_partial {
            return Some(self.accumulate_partial(
                event,
                expected,
                memo_raw,
                min_acceptable,
                is_late,
            ));
        }

        let reason = format!(
            "underpaid: got {}, expected {}",
            event.amount, expected.amount
        );
        Some(self.result_with(
            event,
            MatchStatus::MismatchAmount,
            Some(expected.clone()),
            &reason,
            None,
            Some(underpaid_by),
            is_late,
        ))
    }

    /// "Each" mode: tolerance applied per individual payment.
    fn handle_each_mode_underpayment(
        &self,
        event: &PaymentEvent,
        expected: &ExpectedPayment,
        underpaid_by: u128,
        tolerance: u128,
        is_late: Option<bool>,
    ) -> Option<MatchResult> {
        if underpaid_by > tolerance {
            let reason = format!(
                "underpaid: got {}, expected {}",
                event.amount, expected.amount
            );
            return Some(self.result_with(
                event,
                MatchStatus::MismatchAmount,
                Some(expected.clone()),
                &reason,
                None,
                Some(underpaid_by),
                is_late,
            ));
        }
        None // within tolerance, treat as match
    }

    /// "Final" mode: accumulate partials until cumulative total meets min_acceptable.
    fn accumulate_partial(
        &mut self,
        event: &PaymentEvent,
        expected: &ExpectedPayment,
        memo_raw: &str,
        min_acceptable: u128,
        is_late: Option<bool>,
    ) -> MatchResult {
        let cumulative = self.store.add_partial(memo_raw, event.amount);
        if cumulative >= min_acceptable {
            self.store.remove_expected(memo_raw);
            self.store.remove_partial(memo_raw);
            let overpaid_by = if cumulative > expected.amount {
                Some(cumulative - expected.amount)
            } else {
                None
            };
            return self.result_with(
                event,
                MatchStatus::Matched,
                Some(expected.clone()),
                "",
                overpaid_by,
                None,
                is_late,
            );
        }
        let remaining = expected.amount.saturating_sub(cumulative);
        let reason = format!(
            "partial: accumulated {}, need {}",
            cumulative, expected.amount
        );
        self.result_with(
            event,
            MatchStatus::Partial,
            Some(expected.clone()),
            &reason,
            None,
            Some(remaining),
            is_late,
        )
    }

    /// Build a MatchResult with common defaults.
    fn result(
        &self,
        event: &PaymentEvent,
        status: MatchStatus,
        expected: Option<ExpectedPayment>,
        reason: &str,
    ) -> MatchResult {
        self.result_with(event, status, expected, reason, None, None, None)
    }

    /// Build a MatchResult with all fields.
    #[allow(clippy::too_many_arguments)]
    fn result_with(
        &self,
        event: &PaymentEvent,
        status: MatchStatus,
        expected: Option<ExpectedPayment>,
        reason: &str,
        overpaid_by: Option<u128>,
        remaining_amount: Option<u128>,
        is_late: Option<bool>,
    ) -> MatchResult {
        MatchResult {
            status,
            payment: event.clone(),
            expected,
            reason: if reason.is_empty() {
                None
            } else {
                Some(reason.to_string())
            },
            overpaid_by,
            remaining_amount,
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
