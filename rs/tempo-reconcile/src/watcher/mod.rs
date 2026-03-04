mod decode;
mod dedup;
mod rpc;
mod watch;

#[cfg(feature = "watcher-ws")]
mod watch_ws;

pub use watch::{get_tip20_transfer_history, watch_tip20_transfers, WatchConfig, WatchHandle};

#[cfg(feature = "watcher-ws")]
pub use watch_ws::{watch_tip20_transfers_ws, WatchWsConfig};
