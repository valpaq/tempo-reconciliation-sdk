use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use alloy::primitives::{Address, FixedBytes, U256};
use alloy::providers::{Provider, ProviderBuilder};

use crate::constants::MAX_U256;
use crate::error::NonceError;
use crate::rpc::get_nonce_from_precompile;
use crate::types::{NonceMode, NoncePoolOptions, NoncePoolStats, NonceSlot, SlotState};

/// Production-grade nonce pool for Tempo's 2D nonce system.
///
/// Supports two modes:
/// - **Lanes**: N independent parallel nonce sequences (nonceKey 1..N)
/// - **Expiring**: Single TIP-1009 slot (nonceKey = MAX_U256) with time-bounded validity
///
/// # State Machine
///
/// ```text
/// free → acquire() → reserved → submit() → submitted → confirm() → free (nonce++)
///                                         → fail()    → free
///                   → fail()    → free
///                   → reap()    → free (after TTL)
/// any  → release() → free
/// ```
///
/// # Thread safety
///
/// `NoncePool` requires `&mut self` for all mutation methods and is therefore
/// `!Sync`. For shared access across async tasks, wrap in `Arc<Mutex<NoncePool>>`.
pub struct NoncePool {
    address: Address,
    rpc_url: String,
    mode: NonceMode,
    lane_count: u32,
    reservation_ttl_ms: u64,
    valid_before_offset_s: u64,
    chain_id: u64,
    validate_chain_id: bool,

    slots: Vec<NonceSlot>,
    slot_index: HashMap<U256, usize>,
    initialized: bool,

    confirmed_count: u64,
    failed_count: u64,
    reaped_count: u64,
}

impl std::fmt::Debug for NoncePool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoncePool")
            .field("address", &self.address)
            .field("mode", &self.mode)
            .field("lane_count", &self.lane_count)
            .field("initialized", &self.initialized)
            .field("slot_count", &self.slots.len())
            .finish()
    }
}

impl NoncePool {
    /// Create a new nonce pool. Validates options but does NOT call RPC.
    /// Call [`init()`](Self::init) to fetch initial nonces.
    pub fn new(options: NoncePoolOptions) -> Result<Self, NonceError> {
        if options.address == Address::ZERO {
            return Err(NonceError::MissingAddress);
        }
        if options.rpc_url.is_empty() {
            return Err(NonceError::MissingRpcUrl);
        }
        if options.lanes < 1 {
            return Err(NonceError::InvalidLanes);
        }
        if options.reservation_ttl_ms == 0 {
            return Err(NonceError::InvalidTtl);
        }
        if options.valid_before_offset_s == 0 {
            return Err(NonceError::InvalidValidBefore);
        }

        Ok(Self {
            address: options.address,
            rpc_url: options.rpc_url,
            mode: options.mode,
            lane_count: options.lanes,
            reservation_ttl_ms: options.reservation_ttl_ms,
            valid_before_offset_s: options.valid_before_offset_s,
            chain_id: options.chain_id,
            validate_chain_id: options.validate_chain_id,
            slots: Vec::new(),
            slot_index: HashMap::new(),
            initialized: false,
            confirmed_count: 0,
            failed_count: 0,
            reaped_count: 0,
        })
    }

    /// Configured chain ID.
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Initialize the pool by querying on-chain nonce values.
    pub async fn init(&mut self) -> Result<(), NonceError> {
        if self.initialized {
            return Err(NonceError::AlreadyInitialized);
        }
        self.fetch_and_init_slots().await?;
        self.initialized = true;
        Ok(())
    }

    async fn fetch_and_init_slots(&mut self) -> Result<(), NonceError> {
        let provider = ProviderBuilder::new().connect_http(
            self.rpc_url
                .parse()
                .map_err(|_| NonceError::MissingRpcUrl)?,
        );

        if self.validate_chain_id {
            let actual: u64 = provider
                .get_chain_id()
                .await
                .map_err(|e| NonceError::Rpc(alloy::contract::Error::TransportError(e)))?;
            if actual != self.chain_id {
                return Err(NonceError::ChainIdMismatch {
                    configured: self.chain_id,
                    actual,
                });
            }
        }

        self.slots.clear();
        self.slot_index.clear();

        match self.mode {
            NonceMode::Lanes => {
                let futs: Vec<_> = (1..=self.lane_count)
                    .map(|i| {
                        let key = U256::from(i);
                        let prov = &provider;
                        let addr = self.address;
                        async move {
                            let nonce = get_nonce_from_precompile(prov, addr, key).await?;
                            Ok::<_, NonceError>((key, nonce))
                        }
                    })
                    .collect();
                let results = futures::future::join_all(futs).await;
                for (idx, result) in results.into_iter().enumerate() {
                    let (key, nonce) = result?;
                    self.slots.push(NonceSlot::new(key, nonce));
                    self.slot_index.insert(key, idx);
                }
            }
            NonceMode::Expiring => {
                let key = MAX_U256;
                let nonce = get_nonce_from_precompile(&provider, self.address, key).await?;
                self.slots.push(NonceSlot::new(key, nonce));
                self.slot_index.insert(key, 0);
            }
        }

        Ok(())
    }

    /// Acquire a free slot, transitioning it to Reserved.
    ///
    /// If `request_id` is provided and a slot with the same ID exists in
    /// Reserved or Submitted state, that slot is returned (idempotency).
    pub fn acquire(&mut self, request_id: Option<&str>) -> Result<&NonceSlot, NonceError> {
        if !self.initialized {
            return Err(NonceError::NotInitialized);
        }

        if let Some(rid) = request_id {
            if let Some(idx) = self.find_slot_by_request_id(rid) {
                return Ok(&self.slots[idx]);
            }
        }

        self.reap();

        let idx = self
            .slots
            .iter()
            .position(|s| s.state == SlotState::Free)
            .ok_or(NonceError::Exhausted)?;

        let now = Instant::now();
        let slot = &mut self.slots[idx];
        slot.state = SlotState::Reserved;
        slot.reserved_at = Some(now);
        slot.request_id = request_id.map(String::from);

        if self.mode == NonceMode::Expiring {
            let unix_now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            slot.valid_before = Some(unix_now + self.valid_before_offset_s);
        }

        Ok(&self.slots[idx])
    }

    /// Transition a slot from Reserved to Submitted.
    pub fn submit(&mut self, nonce_key: U256, tx_hash: FixedBytes<32>) -> Result<(), NonceError> {
        let slot = self.get_slot_mut(nonce_key)?;

        if slot.state != SlotState::Reserved {
            return Err(NonceError::InvalidState {
                action: "submit",
                nonce_key,
                actual: slot.state.as_str(),
                expected: "reserved",
            });
        }

        slot.state = SlotState::Submitted;
        slot.submitted_at = Some(Instant::now());
        slot.tx_hash = Some(tx_hash);
        Ok(())
    }

    /// Confirm a submitted slot. Increments nonce by 1 and returns to Free.
    pub fn confirm(&mut self, nonce_key: U256) -> Result<(), NonceError> {
        let slot = self.get_slot_mut(nonce_key)?;

        if slot.state != SlotState::Submitted {
            return Err(NonceError::InvalidState {
                action: "confirm",
                nonce_key,
                actual: slot.state.as_str(),
                expected: "submitted",
            });
        }

        slot.nonce += 1;
        slot.reset();
        self.confirmed_count += 1;
        Ok(())
    }

    /// Fail a reserved or submitted slot. Nonce unchanged, returns to Free.
    pub fn fail(&mut self, nonce_key: U256) -> Result<(), NonceError> {
        let slot = self.get_slot_mut(nonce_key)?;

        if slot.state != SlotState::Reserved && slot.state != SlotState::Submitted {
            return Err(NonceError::InvalidState {
                action: "fail",
                nonce_key,
                actual: slot.state.as_str(),
                expected: "submitted\" or \"reserved",
            });
        }

        slot.reset();
        self.failed_count += 1;
        Ok(())
    }

    /// Release any slot back to Free, regardless of state.
    pub fn release(&mut self, nonce_key: U256) -> Result<(), NonceError> {
        let slot = self.get_slot_mut(nonce_key)?;
        slot.reset();
        Ok(())
    }

    /// Reap stale reserved slots that exceed the TTL.
    /// Returns snapshots of reaped slots (state before reset).
    pub fn reap(&mut self) -> Vec<NonceSlot> {
        let now = Instant::now();
        let ttl_ms = self.reservation_ttl_ms;
        let mut reaped = Vec::new();

        for slot in &mut self.slots {
            if slot.state == SlotState::Reserved {
                if let Some(reserved_at) = slot.reserved_at {
                    if now.duration_since(reserved_at).as_millis() as u64 > ttl_ms {
                        reaped.push(slot.clone());
                        slot.reset();
                    }
                }
            }
        }

        self.reaped_count += reaped.len() as u64;
        reaped
    }

    /// Read-only view of all slots.
    pub fn slots(&self) -> &[NonceSlot] {
        &self.slots
    }

    /// Current aggregate statistics.
    pub fn stats(&self) -> NoncePoolStats {
        let mut free = 0;
        let mut reserved = 0;
        let mut submitted = 0;

        for slot in &self.slots {
            match slot.state {
                SlotState::Free => free += 1,
                SlotState::Reserved => reserved += 1,
                SlotState::Submitted => submitted += 1,
            }
        }

        NoncePoolStats {
            total: self.slots.len(),
            free,
            reserved,
            submitted,
            confirmed: self.confirmed_count,
            failed: self.failed_count,
            expired: self.reaped_count,
        }
    }

    /// Re-query all on-chain nonces and reset slots to Free.
    /// Preserves cumulative stats.
    /// Nonce fetches are issued in parallel for all slots.
    pub async fn reset(&mut self) -> Result<(), NonceError> {
        if !self.initialized {
            return Err(NonceError::NotInitialized);
        }

        let provider = ProviderBuilder::new().connect_http(
            self.rpc_url
                .parse()
                .map_err(|_| NonceError::MissingRpcUrl)?,
        );

        let futs: Vec<_> = self
            .slots
            .iter()
            .map(|slot| get_nonce_from_precompile(&provider, self.address, slot.nonce_key))
            .collect();
        let results = futures::future::join_all(futs).await;
        for (slot, result) in self.slots.iter_mut().zip(results) {
            slot.nonce = result?;
            slot.reset();
        }

        Ok(())
    }

    fn get_slot_mut(&mut self, nonce_key: U256) -> Result<&mut NonceSlot, NonceError> {
        let idx = *self
            .slot_index
            .get(&nonce_key)
            .ok_or(NonceError::SlotNotFound(nonce_key))?;
        Ok(&mut self.slots[idx])
    }

    fn find_slot_by_request_id(&self, request_id: &str) -> Option<usize> {
        self.slots.iter().position(|s| {
            (s.state == SlotState::Reserved || s.state == SlotState::Submitted)
                && s.request_id.as_deref() == Some(request_id)
        })
    }

    /// Create an initialized pool for testing without RPC.
    #[doc(hidden)]
    pub fn new_for_testing(
        mode: NonceMode,
        lane_count: u32,
        reservation_ttl_ms: u64,
        valid_before_offset_s: u64,
    ) -> Self {
        let mut pool = Self {
            address: Address::repeat_byte(0x01),
            rpc_url: "http://localhost:8545".to_string(),
            mode,
            lane_count,
            reservation_ttl_ms,
            valid_before_offset_s,
            chain_id: crate::constants::MODERATO_CHAIN_ID,
            validate_chain_id: false,
            slots: Vec::new(),
            slot_index: HashMap::new(),
            initialized: true,
            confirmed_count: 0,
            failed_count: 0,
            reaped_count: 0,
        };

        match mode {
            NonceMode::Lanes => {
                for i in 1..=lane_count {
                    let key = U256::from(i);
                    let idx = pool.slots.len();
                    pool.slots.push(NonceSlot::new(key, 0));
                    pool.slot_index.insert(key, idx);
                }
            }
            NonceMode::Expiring => {
                let key = MAX_U256;
                pool.slots.push(NonceSlot::new(key, 0));
                pool.slot_index.insert(key, 0);
            }
        }

        pool
    }
}
