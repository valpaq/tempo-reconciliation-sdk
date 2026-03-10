# Changelog

All notable changes to `@tempo-reconcile/sdk` will be documented in this file.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)

## [0.1.0] - 2025-12-01

### Added

- Memo v1 encoder/decoder (TEMPO-RECONCILE-MEMO-001)
- Plain-text memo decoder (left/right-padded UTF-8)
- Issuer tag from namespace (keccak256 first 8 bytes)
- ULID ↔ bytes16 conversion
- TIP-20 watcher (HTTP polling + WebSocket)
- Transfer event watcher (standard ERC-20 Transfer)
- Reconciler with 8 match statuses
- Partial payment accumulation with tolerance modes
- CSV, JSON, JSONL export
- Webhook export with HMAC-SHA256 signing and retry
- Pluggable `ReconcileStore` interface
- Explorer client for Tempo block explorer API
