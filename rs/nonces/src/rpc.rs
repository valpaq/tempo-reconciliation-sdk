use alloy::primitives::{Address, U256};
use alloy::providers::Provider;
use alloy::sol;

use crate::constants::NONCE_PRECOMPILE;
use crate::error::NonceError;

sol! {
    #[sol(rpc)]
    interface INonce {
        function getNonce(address owner, uint256 key) external view returns (uint64);
    }
}

/// Query the nonce precompile for a specific (address, key) pair.
pub async fn get_nonce_from_precompile<P: Provider>(
    provider: &P,
    address: Address,
    key: U256,
) -> Result<u64, NonceError> {
    let contract = INonce::new(NONCE_PRECOMPILE, provider);
    let nonce = contract.getNonce(address, key).call().await?;
    Ok(nonce)
}

/// Utility for callers who need the standard EVM nonce -- not used by NoncePool internally.
///
/// Queries the protocol nonce (transaction count) for an address using the `pending` block tag.
pub async fn get_protocol_nonce<P: Provider>(
    provider: &P,
    address: Address,
) -> Result<u64, NonceError> {
    let count = provider
        .get_transaction_count(address)
        .pending()
        .await
        .map_err(|e| NonceError::Rpc(alloy::contract::Error::TransportError(e)))?;
    Ok(count)
}
