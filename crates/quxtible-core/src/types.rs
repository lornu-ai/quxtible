//! Core types for query optimization

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A SQL query with metadata for optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRequest {
    /// Original natural language query
    pub nl_query: String,
    /// Generated SQL to optimize
    pub sql: String,
    /// Target database type
    pub database: DatabaseType,
    /// Schema context for optimization
    pub schema: Option<SchemaContext>,
    /// User/agent context
    pub context: QueryContext,
}

/// Database type support
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SurrealDB,
}

/// Schema context for query optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaContext {
    pub tables: Vec<TableSchema>,
    pub indexes: Vec<IndexInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSchema {
    pub name: String,
    pub columns: Vec<ColumnSchema>,
    pub row_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub table: String,
    pub columns: Vec<String>,
}

/// Query execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryContext {
    pub agent_id: String,
    pub user_id: Option<String>,
    pub session_id: String,
    pub timestamp_ms: i64,
}

/// Phase 1: Cost estimation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub query_id: String,
    pub estimated_rows: i64,
    pub estimated_cost: f64,
    pub execution_time_ms_estimated: f64,
    pub is_expensive: bool,
    pub risk_level: RiskLevel,
    pub explain_plan: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    Safe,
    Warning,
    Critical,
}

/// Phase 2: LLM-driven optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRefinement {
    pub original_sql: String,
    pub refined_sql: String,
    pub optimizations_applied: Vec<String>,
    pub confidence: f32,
    pub rationale: String,
}

/// Phase 3: Batch query optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueryPlan {
    pub queries: Vec<QueryRequest>,
    pub consolidated_plan: String,
    pub shared_context: BTreeMap<String, serde_json::Value>,
    pub execution_order: Vec<usize>,
}

/// Phase 4: Database tuning recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuningRecommendation {
    pub recommendation_type: TuningType,
    pub target: String,
    pub rationale: String,
    pub expected_improvement_percent: f32,
    pub priority: Priority,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TuningType {
    CreateIndex,
    ModifyPartition,
    MaterializedView,
    StatisticsUpdate,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// Overall optimization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub query_id: String,
    pub original_sql: String,
    pub optimized_sql: Option<String>,
    pub cost_estimate: CostEstimate,
    pub refinements: Option<Vec<QueryRefinement>>,
    pub recommendations: Vec<TuningRecommendation>,
    pub status: OptimizationStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OptimizationStatus {
    Approved,
    Refinement,
    Blocked,
    Error,
}
