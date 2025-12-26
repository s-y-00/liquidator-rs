use log::info;
use std::time::Instant;

/// Performance metrics for a single epoch
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub epoch_start: Instant,
    pub oracle_fetch_ms: u64,
    pub obligations_fetch_ms: u64,
    pub reserves_fetch_ms: u64,
    pub processing_ms: u64,
    pub total_obligations: usize,
    pub unhealthy_obligations: usize,
    pub liquidations_attempted: usize,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            epoch_start: Instant::now(),
            oracle_fetch_ms: 0,
            obligations_fetch_ms: 0,
            reserves_fetch_ms: 0,
            processing_ms: 0,
            total_obligations: 0,
            unhealthy_obligations: 0,
            liquidations_attempted: 0,
        }
    }
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_epoch() -> Self {
        Self {
            epoch_start: Instant::now(),
            ..Default::default()
        }
    }

    pub fn log_summary(&self) {
        let total_ms = self.epoch_start.elapsed().as_millis();
        info!("Epoch Performance Summary:");
        info!("  Oracle Fetch:      {} ms", self.oracle_fetch_ms);
        info!("  Obligations Fetch: {} ms", self.obligations_fetch_ms);
        info!("  Reserves Fetch:    {} ms", self.reserves_fetch_ms);
        info!("  Processing:        {} ms", self.processing_ms);
        info!("  Total Epoch Time:  {} ms", total_ms);
        info!(
            "  Stats: {} total obs, {} unhealthy, {} liquidations",
            self.total_obligations, self.unhealthy_obligations, self.liquidations_attempted
        );
    }
}
