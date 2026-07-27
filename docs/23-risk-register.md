# Risk Register

**Document ID:** QF-RISK-001  
**Version:** 1.0.0  
**Status:** Living register (v1 baseline)  

Impact/Likelihood: **H** high, **M** medium, **L** low.

---

## Risk table

| ID | Risk | Impact | Likelihood | Mitigation | Validation |
|----|------|--------|------------|------------|------------|
| RK-01 | **Schema drift** between crates/docs/code | H | M | Single CBOR schema module; version fields; CI fixture decode | Migration + decode tests on every PR touching schema |
| RK-02 | **Recovery bugs** (lost commits / false healthy open) | H | M | Strict fsync protocol; fail-closed integrity; LKG | REC-01..08 fault injection on Windows/Linux |
| RK-03 | **Migration bugs** destroying content | H | M | Never in-place only copy; pre/post hashes; unknown identity asserts | Fixture matrix + crash mid-migrate |
| RK-04 | **Plugin-content loss** (unknown strip) | H | M | UnknownBlock raw payload path; encode identity tests | Plugin remove/reinstall suite |
| RK-05 | **Layout nondeterminism** | M | H | Fixed-point math; pinned fonts; sorted iteration | Cross-OS golden hashes |
| RK-06 | **Sync conflicts / divergence** | H | M | CRDT primary; op_id idempotency; property tests | Multi-replica convergence harness |
| RK-07 | **AI permission misuse** (exfil / silent edit) | H | M | Capability tokens; suggestion-only API; no apply method on AiService | Security tests; API review checklist |
| RK-08 | **Export fidelity drift** | M | M | Golden exports; pin generator version in manifest | Export CI goldens |
| RK-09 | **Performance regression** | M | H | Budgets in CI benches; flamegraph on regress | Benchmark gates |
| RK-10 | **Journal poison** (bad op committed) | H | L | Dry-run apply before journal; pure apply | Dry-run failure never writes journal |
| RK-11 | **Windows atomic rename edge cases** | H | M | Platform-specific save tests; LKG | Dedicated Windows CI jobs |
| RK-12 | **GPU device loss instability** | M | M | Full resource recreate path | Device-loss simulation test |
| RK-13 | **Encryption key loss** | H | L | Clear UX; keyfile backup guidance; no silent re-encrypt | User docs + tests for wrong key |
| RK-14 | **CRDT complexity delay** | M | H | Phase single-player offsets first; migrate on share | Roadmap phase gates |
| RK-15 | **ZIP bomb / malicious package** | M | M | Size limits; streaming counts; reject quotas | Malformed package fuzzer |
| RK-16 | **Audit log contains secrets/content** | M | M | Default redaction; schema review | Audit content scanner test |
| RK-17 | **Telemetry accidental content** | M | L | Default off; denylist; code review | Static checks for telemetry APIs |
| RK-18 | **Dependency supply chain** | M | M | Cargo audit; lockfile; minimal deps on crypto | `cargo deny` / audit in CI |

---

## Priority treatment

Risks **RK-02, RK-03, RK-04, RK-10** map directly to absolute priorities 1–5. They block release if unmitigated.

---

## Review cadence

- Update this register at end of each roadmap phase.  
- New H-impact risks require design note before merge of related code.  

---

*End of risk register.*
