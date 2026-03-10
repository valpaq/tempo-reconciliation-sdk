use crate::types::{ExpectedPayment, MatchResult};
use crate::ReconcileError;
use std::collections::HashMap;

/// Storage backend for the reconciler.
///
/// Implement this trait to use a custom persistence layer (Postgres, Redis, SQLite, etc.).
/// The default implementation is [`InMemoryStore`].
///
/// # Example: custom implementation skeleton
///
/// ```no_run
/// use tempo_reconcile::{ExpectedPayment, MatchResult, ReconcileError, ReconcileStore};
///
/// struct MyStore { /* db connection */ }
///
/// impl ReconcileStore for MyStore {
///     fn add_expected(&mut self, p: ExpectedPayment) -> Result<(), ReconcileError> {
///         // INSERT INTO expected_payments ...
///         Ok(())
///     }
///     fn get_expected(&self, _memo_raw: &str) -> Option<&ExpectedPayment> { todo!() }
///     fn get_all_expected(&self) -> Vec<&ExpectedPayment> { todo!() }
///     fn remove_expected(&mut self, _memo_raw: &str) -> bool { todo!() }
///     fn add_result(&mut self, _key: &str, _result: MatchResult) { todo!() }
///     fn get_result(&self, _key: &str) -> Option<&MatchResult> { todo!() }
///     fn get_all_results(&self) -> Vec<&MatchResult> { todo!() }
///     fn add_partial(&mut self, _memo_raw: &str, _amount: u128) -> u128 { todo!() }
///     fn get_partial_total(&self, _memo_raw: &str) -> u128 { todo!() }
///     fn remove_partial(&mut self, _memo_raw: &str) { todo!() }
///     fn clear(&mut self) { todo!() }
/// }
/// ```
pub trait ReconcileStore {
    // --- Expected payments ---

    /// Register an expected payment. Errors if `memo_raw` is already present.
    fn add_expected(&mut self, payment: ExpectedPayment) -> Result<(), ReconcileError>;

    fn get_expected(&self, memo_raw: &str) -> Option<&ExpectedPayment>;

    fn get_all_expected(&self) -> Vec<&ExpectedPayment>;

    fn remove_expected(&mut self, memo_raw: &str) -> bool;

    // --- Match results ---

    /// Store a match result keyed by `"{tx_hash}:{log_index}"`.
    fn add_result(&mut self, key: &str, result: MatchResult);

    fn get_result(&self, key: &str) -> Option<&MatchResult>;

    fn get_all_results(&self) -> Vec<&MatchResult>;

    // --- Partial payment accumulation ---

    /// Add `amount` to the cumulative partial for `memo_raw`. Returns the new cumulative total.
    fn add_partial(&mut self, memo_raw: &str, amount: u128) -> u128;

    fn get_partial_total(&self, memo_raw: &str) -> u128;

    fn remove_partial(&mut self, memo_raw: &str);

    // --- Lifecycle ---

    fn clear(&mut self);
}

/// In-memory store backed by HashMaps.
///
/// Suitable for scripts, CLI tools, and tests. Swap for a persistent implementation
/// in production by implementing [`ReconcileStore`].
#[derive(Debug, Default)]
pub struct InMemoryStore {
    expected: HashMap<String, ExpectedPayment>,
    results: HashMap<String, MatchResult>,
    partials: HashMap<String, u128>,
}

impl InMemoryStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ReconcileStore for InMemoryStore {
    fn add_expected(&mut self, payment: ExpectedPayment) -> Result<(), ReconcileError> {
        let key = payment.memo_raw.to_ascii_lowercase();
        if self.expected.contains_key(&key) {
            return Err(ReconcileError::DuplicateExpected(key));
        }
        self.expected.insert(key, payment);
        Ok(())
    }

    fn get_expected(&self, memo_raw: &str) -> Option<&ExpectedPayment> {
        self.expected.get(&memo_raw.to_ascii_lowercase())
    }

    fn get_all_expected(&self) -> Vec<&ExpectedPayment> {
        self.expected.values().collect()
    }

    fn remove_expected(&mut self, memo_raw: &str) -> bool {
        self.expected
            .remove(&memo_raw.to_ascii_lowercase())
            .is_some()
    }

    fn add_result(&mut self, key: &str, result: MatchResult) {
        self.results.insert(key.to_ascii_lowercase(), result);
    }

    fn get_result(&self, key: &str) -> Option<&MatchResult> {
        self.results.get(&key.to_ascii_lowercase())
    }

    fn get_all_results(&self) -> Vec<&MatchResult> {
        self.results.values().collect()
    }

    fn add_partial(&mut self, memo_raw: &str, amount: u128) -> u128 {
        let key = memo_raw.to_ascii_lowercase();
        let cumulative = self.partials.entry(key).or_insert(0);
        *cumulative = cumulative.saturating_add(amount);
        *cumulative
    }

    fn get_partial_total(&self, memo_raw: &str) -> u128 {
        *self
            .partials
            .get(&memo_raw.to_ascii_lowercase())
            .unwrap_or(&0)
    }

    fn remove_partial(&mut self, memo_raw: &str) {
        self.partials.remove(&memo_raw.to_ascii_lowercase());
    }

    fn clear(&mut self) {
        self.expected.clear();
        self.results.clear();
        self.partials.clear();
    }
}
