//! Metrics and observability
//!
//! Tracks performance metrics for:
//! - Request counts and latency
//! - Error rates
//! - Optimization outcomes
//! - Database performance

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Application metrics tracker
#[derive(Clone)]
pub struct Metrics {
    inner: Arc<MetricsInner>,
}

struct MetricsInner {
    // Request counts
    pub total_requests: AtomicU64,
    pub successful_requests: AtomicU64,
    pub failed_requests: AtomicU64,

    // By endpoint
    pub estimate_cost_calls: AtomicU64,
    pub refine_calls: AtomicU64,
    pub batch_optimize_calls: AtomicU64,
    pub optimize_calls: AtomicU64,

    // Errors
    pub validation_errors: AtomicU64,
    pub optimization_errors: AtomicU64,
    pub database_errors: AtomicU64,

    // Performance
    pub total_request_time_ms: AtomicU64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                total_requests: AtomicU64::new(0),
                successful_requests: AtomicU64::new(0),
                failed_requests: AtomicU64::new(0),
                estimate_cost_calls: AtomicU64::new(0),
                refine_calls: AtomicU64::new(0),
                batch_optimize_calls: AtomicU64::new(0),
                optimize_calls: AtomicU64::new(0),
                validation_errors: AtomicU64::new(0),
                optimization_errors: AtomicU64::new(0),
                database_errors: AtomicU64::new(0),
                total_request_time_ms: AtomicU64::new(0),
            }),
        }
    }

    pub fn record_request_start(&self) {
        self.inner.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_success(&self, endpoint: &str, duration_ms: f32) {
        self.inner
            .successful_requests
            .fetch_add(1, Ordering::Relaxed);
        self.inner
            .total_request_time_ms
            .fetch_add(duration_ms as u64, Ordering::Relaxed);

        match endpoint {
            "estimate_cost" => self.inner.estimate_cost_calls.fetch_add(1, Ordering::Relaxed),
            "refine" => self.inner.refine_calls.fetch_add(1, Ordering::Relaxed),
            "batch_optimize" => self.inner.batch_optimize_calls.fetch_add(1, Ordering::Relaxed),
            "optimize" => self.inner.optimize_calls.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };
    }

    pub fn record_error(&self, error_type: &str) {
        self.inner.failed_requests.fetch_add(1, Ordering::Relaxed);
        match error_type {
            "validation" => self.inner.validation_errors.fetch_add(1, Ordering::Relaxed),
            "optimization" => self.inner.optimization_errors.fetch_add(1, Ordering::Relaxed),
            "database" => self.inner.database_errors.fetch_add(1, Ordering::Relaxed),
            _ => 0,
        };
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let total = self.inner.total_requests.load(Ordering::Relaxed);
        let successful = self.inner.successful_requests.load(Ordering::Relaxed);
        let failed = self.inner.failed_requests.load(Ordering::Relaxed);
        let total_time = self.inner.total_request_time_ms.load(Ordering::Relaxed);

        let avg_latency_ms = if total > 0 {
            total_time as f32 / total as f32
        } else {
            0.0
        };

        MetricsSnapshot {
            timestamp: chrono::Utc::now().to_rfc3339(),
            total_requests: total,
            successful_requests: successful,
            failed_requests: failed,
            success_rate: if total > 0 {
                (successful as f32 / total as f32) * 100.0
            } else {
                0.0
            },
            average_latency_ms: avg_latency_ms,
            endpoints: EndpointMetrics {
                estimate_cost: self.inner.estimate_cost_calls.load(Ordering::Relaxed),
                refine: self.inner.refine_calls.load(Ordering::Relaxed),
                batch_optimize: self.inner.batch_optimize_calls.load(Ordering::Relaxed),
                optimize: self.inner.optimize_calls.load(Ordering::Relaxed),
            },
            errors: ErrorMetrics {
                validation: self.inner.validation_errors.load(Ordering::Relaxed),
                optimization: self.inner.optimization_errors.load(Ordering::Relaxed),
                database: self.inner.database_errors.load(Ordering::Relaxed),
            },
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of metrics at a point in time
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub timestamp: String,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub success_rate: f32,
    pub average_latency_ms: f32,
    pub endpoints: EndpointMetrics,
    pub errors: ErrorMetrics,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointMetrics {
    pub estimate_cost: u64,
    pub refine: u64,
    pub batch_optimize: u64,
    pub optimize: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorMetrics {
    pub validation: u64,
    pub optimization: u64,
    pub database: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_requests, 0);
    }

    #[test]
    fn test_metrics_record_success() {
        let metrics = Metrics::new();
        metrics.record_request_start();
        metrics.record_success("optimize", 50.0);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.successful_requests, 1);
        assert_eq!(snapshot.endpoints.optimize, 1);
    }

    #[test]
    fn test_metrics_record_error() {
        let metrics = Metrics::new();
        metrics.record_request_start();
        metrics.record_error("validation");

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.failed_requests, 1);
        assert_eq!(snapshot.errors.validation, 1);
    }

    #[test]
    fn test_metrics_success_rate() {
        let metrics = Metrics::new();
        metrics.record_request_start();
        metrics.record_success("optimize", 10.0);
        metrics.record_request_start();
        metrics.record_error("database");

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.total_requests, 2);
        assert_eq!(snapshot.successful_requests, 1);
        assert_eq!(snapshot.failed_requests, 1);
        assert!((snapshot.success_rate - 50.0).abs() < 0.1);
    }
}
