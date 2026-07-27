# Telemetry Specification

**Document ID:** QF-TEL-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Defaults

| Setting | Value |
|---------|-------|
| Telemetry | **Disabled by default** |
| Document content collection | **Forbidden always** |
| Keystrokes / text | **Forbidden** |
| File names | Optional hashed only if user enables “diagnostics” |
| Crash stacks | Opt-in; scrub paths |

---

## 2. Allowed metrics (when enabled)

- Performance counters (apply latency histograms)  
- Feature use counters (export format used, not content)  
- Error codes (enum), not payloads  
- Session length  
- App version, OS version  

---

## 3. API

```rust
pub struct TelemetryConfig {
    pub enabled: bool, // default false
    pub endpoint: Option<Url>,
}

pub struct Telemetry { /* ... */ }

impl Telemetry {
    pub fn emit_counter(&self, name: MetricName, value: u64);
    pub fn emit_histogram_ms(&self, name: MetricName, ms: u64);
}

// MetricName is an allowlisted enum — not free-form strings from plugins
pub enum MetricName {
    CommitApplyMs,
    JournalFsyncMs,
    LayoutReflowMs,
    OpenDocumentMs,
    ExportMs,
    SyncPushFailure,
    // ...
}
```

Plugins MUST NOT emit arbitrary telemetry containing user data.

---

## 4. Enforcement

- Code review checklist  
- CI grep for suspicious sinks  
- Runtime assert: disabled → no network  

---

*End of telemetry specification.*
