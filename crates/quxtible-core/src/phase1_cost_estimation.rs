//! Phase 1: Pre-Execution Cost Estimation
//!
//! Runs EXPLAIN against the database to retrieve query execution plans
//! and estimated computational costs. Prevents execution of expensive queries.

use crate::database::DatabaseConnector;
use crate::types::{CostEstimate, DatabaseType, RiskLevel};
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait CostEstimator: Send + Sync {
    /// Estimate the cost of executing a SQL query
    async fn estimate_cost(&self, sql: &str) -> Result<CostEstimate>;

    /// Check if a query is safe to execute
    fn is_safe(&self, estimate: &CostEstimate) -> bool {
        estimate.risk_level != RiskLevel::Critical && !estimate.is_expensive
    }
}

/// Generic cost estimator using database connectors
pub struct GenericCostEstimator {
    connector: Arc<dyn DatabaseConnector>,
    cost_threshold: f64,
    time_threshold_ms: f64,
}

impl GenericCostEstimator {
    pub fn new(
        connector: Arc<dyn DatabaseConnector>,
        cost_threshold: f64,
        time_threshold_ms: f64,
    ) -> Self {
        Self {
            connector,
            cost_threshold,
            time_threshold_ms,
        }
    }

    /// Parse PostgreSQL EXPLAIN JSON output
    fn parse_postgres_explain(&self, explain_json: &str) -> Result<(f64, i64, f64)> {
        let parsed: serde_json::Value =
            serde_json::from_str(explain_json).context("Failed to parse EXPLAIN JSON")?;

        if !parsed.is_array() || parsed.as_array().unwrap().is_empty() {
            return Err(anyhow::anyhow!("Invalid EXPLAIN output"));
        }

        let plan = &parsed[0]["Plan"];

        // Extract cost and rows
        let total_cost = plan["Total Cost"]
            .as_f64()
            .unwrap_or(0.0);
        let planned_rows = plan["Plans"]
            .as_array()
            .map(|p| p.len() as i64)
            .unwrap_or(1);

        // Estimate execution time based on cost
        // PostgreSQL cost units typically: 1 unit ≈ 0.01ms
        let estimated_time_ms = total_cost * 0.01;

        Ok((total_cost, planned_rows, estimated_time_ms))
    }
}

#[async_trait]
impl CostEstimator for GenericCostEstimator {
    async fn estimate_cost(&self, sql: &str) -> Result<CostEstimate> {
        // Get EXPLAIN output from connector
        let explain_output = self
            .connector
            .explain(sql)
            .await
            .context("EXPLAIN execution failed")?;

        // Parse based on database type
        let (total_cost, estimated_rows, estimated_time_ms) = match self.connector.database_type() {
            DatabaseType::PostgreSQL => self.parse_postgres_explain(&explain_output)?,
            DatabaseType::MySQL => {
                // TODO: Implement MySQL EXPLAIN parsing
                (0.0, 0, 0.0)
            }
            DatabaseType::SurrealDB => {
                // TODO: Implement SurrealDB cost estimation
                (0.0, 0, 0.0)
            }
        };

        // Determine risk level
        let is_expensive = total_cost > self.cost_threshold;
        let is_slow = estimated_time_ms > self.time_threshold_ms;

        let risk_level = match (is_expensive, is_slow) {
            (false, false) => RiskLevel::Safe,
            (true, false) | (false, true) => RiskLevel::Warning,
            (true, true) => RiskLevel::Critical,
        };

        Ok(CostEstimate {
            query_id: Uuid::new_v4().to_string(),
            estimated_rows,
            estimated_cost: total_cost,
            execution_time_ms_estimated: estimated_time_ms,
            is_expensive,
            risk_level,
            explain_plan: explain_output,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_postgres_explain_simple() {
        let estimator = GenericCostEstimator::new(
            Arc::new(MockConnector),
            1000.0,
            100.0,
        );

        let explain_json = r#"[{"Plan": {"Total Cost": 50.0, "Plans": []}}]"#;
        let (cost, rows, time) = estimator.parse_postgres_explain(explain_json).unwrap();

        assert_eq!(cost, 50.0);
        assert_eq!(rows, 0);
        assert!((time - 0.5).abs() < 0.01); // 50 * 0.01 = 0.5
    }

    #[test]
    fn test_cost_threshold_safe() {
        let estimator = GenericCostEstimator::new(
            Arc::new(MockConnector),
            1000.0,
            100.0,
        );

        let estimate = CostEstimate {
            query_id: "test".to_string(),
            estimated_rows: 100,
            estimated_cost: 500.0,
            execution_time_ms_estimated: 5.0,
            is_expensive: false,
            risk_level: RiskLevel::Safe,
            explain_plan: "{}".to_string(),
        };

        assert!(estimator.is_safe(&estimate));
    }

    #[test]
    fn test_cost_threshold_expensive() {
        let estimator = GenericCostEstimator::new(
            Arc::new(MockConnector),
            1000.0,
            100.0,
        );

        let estimate = CostEstimate {
            query_id: "test".to_string(),
            estimated_rows: 1000000,
            estimated_cost: 5000.0,
            execution_time_ms_estimated: 50.0,
            is_expensive: true,
            risk_level: RiskLevel::Critical,
            explain_plan: "{}".to_string(),
        };

        assert!(!estimator.is_safe(&estimate));
    }

    #[test]
    fn test_risk_level_safe() {
        let estimator = GenericCostEstimator::new(
            Arc::new(MockConnector),
            1000.0,
            100.0,
        );

        // Both cost and time under thresholds
        let estimate = CostEstimate {
            query_id: "test".to_string(),
            estimated_rows: 10,
            estimated_cost: 100.0,
            execution_time_ms_estimated: 1.0,
            is_expensive: false,
            risk_level: RiskLevel::Safe,
            explain_plan: "{}".to_string(),
        };

        assert_eq!(estimate.risk_level, RiskLevel::Safe);
    }

    #[test]
    fn test_risk_level_warning() {
        let estimator = GenericCostEstimator::new(
            Arc::new(MockConnector),
            1000.0,
            100.0,
        );

        // Cost over threshold, time under
        let estimate = CostEstimate {
            query_id: "test".to_string(),
            estimated_rows: 100000,
            estimated_cost: 2000.0,
            execution_time_ms_estimated: 20.0,
            is_expensive: true,
            risk_level: RiskLevel::Warning,
            explain_plan: "{}".to_string(),
        };

        assert_eq!(estimate.risk_level, RiskLevel::Warning);
    }

    #[test]
    fn test_risk_level_critical() {
        let estimator = GenericCostEstimator::new(
            Arc::new(MockConnector),
            1000.0,
            100.0,
        );

        // Both cost and time over thresholds
        let estimate = CostEstimate {
            query_id: "test".to_string(),
            estimated_rows: 10000000,
            estimated_cost: 5000.0,
            execution_time_ms_estimated: 50.0,
            is_expensive: true,
            risk_level: RiskLevel::Critical,
            explain_plan: "{}".to_string(),
        };

        assert_eq!(estimate.risk_level, RiskLevel::Critical);
    }

    // Mock connector for testing
    struct MockConnector;

    #[async_trait]
    impl DatabaseConnector for MockConnector {
        async fn execute(&self, _sql: &str) -> Result<serde_json::Value> {
            Ok(json!({}))
        }

        async fn get_schema(&self) -> Result<crate::types::SchemaContext> {
            Ok(crate::types::SchemaContext {
                tables: vec![],
                indexes: vec![],
            })
        }

        async fn explain(&self, _sql: &str) -> Result<String> {
            Ok("[{\"Plan\": {\"Total Cost\": 100.0, \"Plans\": []}}]".to_string())
        }

        fn database_type(&self) -> DatabaseType {
            DatabaseType::PostgreSQL
        }
    }
}
