# Changelog

All notable changes to `tempo-reconcile` will be documented in this file.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)

## [0.1.0] - 2026-02-01

### Added
- Memo v1 encoder/decoder (TEMPO-RECONCILE-MEMO-001)
- Plain-text memo decoder
- Issuer tag from namespace
- ULID ↔ bytes16 conversion
- TIP-20 watcher with HTTP polling (feature: `watcher`)
- WebSocket watcher (feature: `watcher-ws`)
- Reconciler with 8 match statuses
- Partial payment accumulation with tolerance modes
- CSV, JSON, JSONL export
- Webhook export with HMAC-SHA256 and retry (feature: `webhook`)
- Explorer client (feature: `explorer`)
- Pluggable `ReconcileStore` trait
- CLI binary (`tempo-reconcile-cli`)
