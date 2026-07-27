//! Telemetry is disabled by default. Document content MUST NEVER be collected.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricName {
    CommitApplyMs,
    JournalFsyncMs,
    LayoutReflowMs,
    OpenDocumentMs,
    ExportMs,
    SyncPushFailure,
}

#[derive(Clone, Debug)]
pub struct TelemetryConfig {
    pub enabled: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

#[derive(Default)]
pub struct Telemetry {
    pub config: TelemetryConfig,
    counters: Vec<(MetricName, u64)>,
}

impl Telemetry {
    pub fn new(config: TelemetryConfig) -> Self {
        Self {
            config,
            counters: vec![],
        }
    }

    pub fn emit_counter(&mut self, name: MetricName, value: u64) {
        if !self.config.enabled {
            return;
        }
        self.counters.push((name, value));
    }

    pub fn emit_histogram_ms(&mut self, name: MetricName, ms: u64) {
        self.emit_counter(name, ms);
    }

    /// Compile-time policy helper for reviews.
    pub const FORBIDS_DOCUMENT_CONTENT: bool = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_disabled_emits_nothing() {
        let mut t = Telemetry::default();
        t.emit_counter(MetricName::ExportMs, 10);
        assert!(t.counters.is_empty());
    }
}
