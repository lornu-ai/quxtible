//! Health check and status endpoints
//!
//! Provides detailed health status of all system components for:
//! - Kubernetes liveness/readiness probes
//! - Load balancer health checks
//! - Operator monitoring

use crate::state::AppState;
use axum::extract::State;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Overall system health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    /// Overall status: "healthy", "degraded", "unhealthy"
    pub status: String,
    /// Timestamp of check
    pub timestamp: String,
    /// Per-component status
    pub components: ComponentStatus,
    /// Message for operators
    pub message: String,
}

/// Individual component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub database: ComponentHealth,
    pub cost_estimator: ComponentHealth,
    pub query_optimizer: ComponentHealth,
    pub batch_optimizer: ComponentHealth,
    pub tuning_advisor: ComponentHealth,
}

/// Single component health details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub status: String,  // "healthy", "degraded", "unhealthy"
    pub message: String,
    pub response_time_ms: f32,
}

/// Health check endpoint handler
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> axum::Json<HealthStatus> {
    debug!("Health check requested");

    let start = std::time::Instant::now();

    // Check database connectivity
    let db_start = std::time::Instant::now();
    let db_status = match state.database.get_schema().await {
        Ok(_) => ComponentHealth {
            status: "healthy".to_string(),
            message: "Database connected and schema accessible".to_string(),
            response_time_ms: db_start.elapsed().as_secs_f32() * 1000.0,
        },
        Err(e) => ComponentHealth {
            status: "unhealthy".to_string(),
            message: format!("Database error: {}", e),
            response_time_ms: db_start.elapsed().as_secs_f32() * 1000.0,
        },
    };

    // Cost estimator is always healthy (in-memory)
    let cost_estimator = ComponentHealth {
        status: "healthy".to_string(),
        message: "Cost estimator initialized".to_string(),
        response_time_ms: 0.1,
    };

    // Query optimizer is always healthy (requires API key, but initialized)
    let query_optimizer = ComponentHealth {
        status: "healthy".to_string(),
        message: format!("Query optimizer ready (model: {})", state.config.llm.model),
        response_time_ms: 0.1,
    };

    // Batch optimizer is always healthy (in-memory)
    let batch_optimizer = ComponentHealth {
        status: "healthy".to_string(),
        message: "Batch optimizer initialized".to_string(),
        response_time_ms: 0.1,
    };

    // Tuning advisor is always healthy (in-memory with Mutex)
    let tuning_advisor = ComponentHealth {
        status: "healthy".to_string(),
        message: "Tuning advisor ready".to_string(),
        response_time_ms: 0.1,
    };

    let components = ComponentStatus {
        database: db_status.clone(),
        cost_estimator,
        query_optimizer,
        batch_optimizer,
        tuning_advisor,
    };

    // Overall status: unhealthy if any critical component is down
    let overall_status = if db_status.status == "unhealthy" {
        "unhealthy"
    } else {
        "healthy"
    };

    let message = match overall_status {
        "unhealthy" => "Database is unavailable".to_string(),
        _ => "All systems operational".to_string(),
    };

    let elapsed = start.elapsed().as_secs_f32() * 1000.0;

    axum::Json(HealthStatus {
        status: overall_status.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        components,
        message,
    })
}

/// Readiness check - used by Kubernetes for rolling deployments
/// Returns 200 only when service is ready to accept traffic
pub async fn readiness_check(
    State(state): State<Arc<AppState>>,
) -> Result<&'static str, axum::http::StatusCode> {
    match state.database.get_schema().await {
        Ok(_) => Ok("Ready"),
        Err(_) => Err(axum::http::StatusCode::SERVICE_UNAVAILABLE),
    }
}

/// Liveness check - used by Kubernetes to determine if pod should be restarted
/// Returns 200 if service is still running (even if not ready)
pub async fn liveness_check() -> &'static str {
    "Alive"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_health_serialization() {
        let health = ComponentHealth {
            status: "healthy".to_string(),
            message: "Test".to_string(),
            response_time_ms: 1.5,
        };

        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("healthy"));
    }

    #[test]
    fn test_health_status_structure() {
        let db_health = ComponentHealth {
            status: "healthy".to_string(),
            message: "Connected".to_string(),
            response_time_ms: 5.0,
        };

        let components = ComponentStatus {
            database: db_health,
            cost_estimator: ComponentHealth {
                status: "healthy".to_string(),
                message: "Ready".to_string(),
                response_time_ms: 0.1,
            },
            query_optimizer: ComponentHealth {
                status: "healthy".to_string(),
                message: "Ready".to_string(),
                response_time_ms: 0.1,
            },
            batch_optimizer: ComponentHealth {
                status: "healthy".to_string(),
                message: "Ready".to_string(),
                response_time_ms: 0.1,
            },
            tuning_advisor: ComponentHealth {
                status: "healthy".to_string(),
                message: "Ready".to_string(),
                response_time_ms: 0.1,
            },
        };

        let status = HealthStatus {
            status: "healthy".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            components,
            message: "All systems operational".to_string(),
        };

        assert_eq!(status.status, "healthy");
        assert_eq!(status.components.database.status, "healthy");
    }
}
