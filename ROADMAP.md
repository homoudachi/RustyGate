# RustyGate Development Roadmap

## Phase 1: Core Foundation (Completed)
- [x] Basic BACnet/IP stack integration (`bacnet-rs`).
- [x] Robust remote integration testing infrastructure.
- [x] Network discovery (Who-Is/I-Am) with unicast fallback.
- [x] Object discovery (ReadProperty ObjectList).
- [x] Advanced Simulator with AI/BI/Device support and dynamic updates.
- [x] Robust networking (error suppression, clean shutdown).

## Phase 2: Web UI Development (Current)
- [ ] Network interface selection via Web UI.
- [ ] Visual Device Discovery process and searchable list.
- [ ] Drill-down to Object Discovery and point listing.
- [ ] Real-time system logs and status updates in the UI.
- [ ] Integration of the "Dual-Socket" reliable networking logic.

## Phase 3: Data Acquisition & Bridging
- [ ] Automatic polling engine for discovered/mapped objects.
- [ ] MQTT broker connection management and status monitoring.
- [ ] Point-to-Topic mapping configuration.
- [ ] COV (Change of Value) subscription support with polling fallback.

## Phase 4: Persistence & Management
- [ ] SQLite integration via `sqlx` for state persistence.
- [ ] Configuration storage (Device Map, Point Map).
- [ ] Gateway identity settings (Device Object ID, Vendor info).
- [ ] Audit logging and error history.

## Phase 5: Advanced Features & BTL Readiness
- [ ] WriteProperty support with Priority Array management.
- [ ] BBMD (BACnet Broadcast Management Device) Foreign Device registration.
- [ ] Multi-segment message handling for large object lists.
- [ ] Performance benchmarking for 1000+ points.
- [ ] Formal BIBB compliance verification for B-GW profile.
