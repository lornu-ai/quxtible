//! HTTP request handlers for optimization endpoints

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use axum::{extract::State, Json};
use quxtible_core::types::{
    CostEstimate, OptimizationResult, OptimizationStatus, QueryRequest, QueryRefinement,
};
use quxtible_core::{
    phase1_cost_estimation::CostEstimator,
    phase2_llm_refinement::QueryOptimizer,
    phase4_autonomous_tuning::QueryExecutionMetrics,
};
use std::sync::Arc;
use tracing::{info, debug};

/// Health check endpoint
pub async fn healthz() -> &'static str {
    "ok"
}

/// Phase 1: Cost estimation endpoint
///
/// Runs EXPLAIN on the query and estimates execution cost.
/// Returns risk level (Safe/Warning/Critical) and execution metrics.
pub async fn estimate_cost(
    State(state): State<Arc<AppState>>,
    Json(request): Json<QueryRequest>,
) -> ApiResult<Json<CostEstimate>> {
    use crate::validation::validate_query_request;

    // Validate request
    validate_query_request(&request, state.config.server.max_request_body_bytes)?;

    info!("💰 Estimating cost for query: {}", request.nl_query);

    let estimate = state
        .cost_estimator
        .estimate_cost(&request.sql)
        .await
        .map_err(|e| ApiError::Optimization(e.to_string()))?;

    debug!(
        "Cost estimate: {} ({}ms)",
        estimate.estimated_cost, estimate.execution_time_ms_estimated
    );

    Ok(Json(estimate))
}

/// Phase 2: Query refinement endpoint
///
/// Uses LLM (Claude) to optimize SQL queries.
/// Applies improvements: CTEs, JOIN optimization, predicate pushdown, etc.
pub async fn refine_query(
    State(state): State<Arc<AppState>>,
    Json(request): Json<QueryRequest>,
) -> ApiResult<Json<QueryRefinement>> {
    use crate::validation::validate_query_request;

    // Validate request
    validate_query_request(&request, state.config.server.max_request_body_bytes)?;

    info!("🤖 Refining query with LLM: {}", request.nl_query);

    // Optionally get EXPLAIN feedback for better optimization
    let explain_feedback = state
        .cost_estimator
        .estimate_cost(&request.sql)
        .await
        .ok()
        .map(|est| est.explain_plan);

    let refinement = state
        .query_optimizer
        .refine_query(&request.sql, explain_feedback.as_deref())
        .await
        .map_err(|e| ApiError::Optimization(e.to_string()))?;

    debug!(
        "Optimized SQL: {} (confidence: {})",
        refinement.refined_sql, refinement.confidence
    );

    Ok(Json(refinement))
}

/// Phase 3: Batch query optimization endpoint
///
/// Consolidates multiple queries into optimized batch plan.
/// Detects similar queries, extracts shared context, and computes execution order.
pub async fn batch_optimize(
    State(state): State<Arc<AppState>>,
    Json(requests): Json<Vec<QueryRequest>>,
) -> ApiResult<Json<quxtible_core::types::BatchQueryPlan>> {
    use crate::validation::validate_batch_requests;

    // Validate requests
    validate_batch_requests(&requests, state.config.server.max_request_body_bytes)?;

    info!("📦 Optimizing batch of {} queries", requests.len());

    let batch_plan = state
        .batch_optimizer
        .optimize_batch(requests)
        .map_err(|e| ApiError::Optimization(e.to_string()))?;

    debug!(
        "Batch plan: {} queries, execution order: {:?}",
        batch_plan.queries.len(),
        batch_plan.execution_order
    );

    Ok(Json(batch_plan))
}

/// Full optimization pipeline endpoint
///
/// Orchestrates all four phases:
/// 1. Cost estimation (Phase 1)
/// 2. LLM refinement if cost is Warning/Critical (Phase 2)
/// 3. Batch optimization for multi-agent scenarios (Phase 3)
/// 4. Autonomous tuning recommendations (Phase 4)
pub async fn optimize_query(
    State(state): State<Arc<AppState>>,
    Json(request): Json<QueryRequest>,
) -> ApiResult<Json<OptimizationResult>> {
    use crate::validation::validate_query_request;

    // Validate request
    validate_query_request(&request, state.config.server.max_request_body_bytes)?;

    info!("🔄 Full optimization pipeline for: {}", request.nl_query);

    // Phase 1: Cost Estimation
    let cost_estimate = state
        .cost_estimator
        .estimate_cost(&request.sql)
        .await
        .map_err(|e| ApiError::Optimization(format!("Cost estimation failed: {}", e)))?;

    debug!("Phase 1 ✓ Cost: {} ({}ms)", cost_estimate.estimated_cost, cost_estimate.execution_time_ms_estimated);

    // Determine if refinement is needed
    let refinements = if state.cost_estimator.is_safe(&cost_estimate) {
        debug!("Query is safe, skipping Phase 2");
        None
    } else {
        // Phase 2: LLM Refinement for non-safe queries
        debug!("Query needs optimization, applying Phase 2");
        let refinement = state
            .query_optimizer
            .refine_query(&request.sql, Some(&cost_estimate.explain_plan))
            .await
            .map_err(|e| ApiError::Optimization(format!("Query refinement failed: {}", e)))?;

        debug!("Phase 2 ✓ Refined SQL (confidence: {})", refinement.confidence);
        Some(vec![refinement])
    };

    // Phase 4: Generate autonomous tuning recommendations
    // This would use actual execution metrics - for now, generate recommendations
    // based on estimated metrics
    let mut tuning_advisor = state.tuning_advisor.lock().await;
    let metrics = QueryExecutionMetrics {
        query_id: cost_estimate.query_id.clone(),
        estimated_cost: cost_estimate.estimated_cost,
        actual_cost: cost_estimate.estimated_cost, // Use estimate as actual for now
        estimated_rows: cost_estimate.estimated_rows,
        actual_rows: cost_estimate.estimated_rows,
        estimated_time_ms: cost_estimate.execution_time_ms_estimated,
        actual_time_ms: cost_estimate.execution_time_ms_estimated,
        full_table_scan: cost_estimate.is_expensive,
        tables_accessed: extract_tables(&request.sql),
        timestamp_ms: request.context.timestamp_ms,
    };

    let recommendations = tuning_advisor.analyze_and_recommend(metrics);
    debug!("Phase 4 ✓ Generated {} recommendations", recommendations.len());

    // Determine optimization status
    let status = if state.cost_estimator.is_safe(&cost_estimate) {
        OptimizationStatus::Approved
    } else if refinements.is_some() {
        OptimizationStatus::Refinement
    } else {
        OptimizationStatus::Blocked
    };

    let optimized_sql = refinements
        .as_ref()
        .and_then(|refs| refs.first())
        .map(|ref_| ref_.refined_sql.clone());

    let result = OptimizationResult {
        query_id: cost_estimate.query_id.clone(),
        original_sql: request.sql,
        optimized_sql,
        cost_estimate,
        refinements,
        recommendations,
        status,
    };

    info!("✅ Optimization complete: {:?}", result.status);
    Ok(Json(result))
}

/// Helper: Extract table names from SQL (simple heuristic)
fn extract_tables(sql: &str) -> Vec<String> {
    let upper = sql.to_uppercase();
    let mut tables = Vec::new();

    if let Some(from_pos) = upper.find("FROM") {
        let after_from = &sql[from_pos + 4..];
        if let Some(table_name) = after_from.trim().split(|c: char| !c.is_alphanumeric() && c != '_')
            .next()
        {
            if !table_name.is_empty() {
                tables.push(table_name.to_string());
            }
        }
    }

    tables
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tables_single() {
        let sql = "SELECT * FROM users WHERE id = 1";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["users"]);
    }

    #[test]
    fn test_extract_tables_case_insensitive() {
        let sql = "select * from orders where status = 'pending'";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["orders"]);
    }

    #[test]
    fn test_extract_tables_none() {
        let sql = "SELECT 1";
        let tables = extract_tables(sql);
        assert!(tables.is_empty());
    }
}
