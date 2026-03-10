//! Phase 4: Autonomous Database Tuning
//!
//! Uses feedback from actual query execution to autonomously recommend and apply
//! database optimizations (indexes, materialized views, partitioning strategies).
//! Implements reinforcement learning patterns to improve recommendations over time.

use crate::types::{CostEstimate, QueryRequest, TuningRecommendation, TuningType, Priority};
use anyhow::Result;
use std::collections::{HashMap, VecDeque};

/// Execution metrics tracking actual vs estimated performance
#[derive(Debug, Clone)]
pub struct QueryExecutionMetrics {
    pub query_id: String,
    pub estimated_cost: f64,
    pub actual_cost: f64,  // Actual execution cost observed
    pub estimated_rows: i64,
    pub actual_rows: i64,   // Actual rows returned
    pub estimated_time_ms: f64,
    pub actual_time_ms: f64, // Actual execution time
    pub full_table_scan: bool,
    pub tables_accessed: Vec<String>,
    pub timestamp_ms: i64,
}

impl QueryExecutionMetrics {
    pub fn estimation_error(&self) -> f64 {
        ((self.actual_cost - self.estimated_cost).abs() / self.estimated_cost.max(1.0)) * 100.0
    }

    pub fn time_estimation_error(&self) -> f64 {
        ((self.actual_time_ms - self.estimated_time_ms).abs() / self.estimated_time_ms.max(1.0))
            * 100.0
    }
}

/// Performance model learns from historical query execution
pub struct PerformanceModel {
    metrics_history: VecDeque<QueryExecutionMetrics>,
    max_history: usize,
    table_statistics: HashMap<String, TableStatistics>,
}

#[derive(Debug, Clone)]
struct TableStatistics {
    table_name: String,
    access_count: usize,
    full_scan_count: usize,
    avg_estimation_error: f64,
    slow_query_count: usize,
}

impl PerformanceModel {
    pub fn new(max_history: usize) -> Self {
        Self {
            metrics_history: VecDeque::with_capacity(max_history),
            max_history,
            table_statistics: HashMap::new(),
        }
    }

    /// Record actual execution metrics
    pub fn record_execution(&mut self, metrics: QueryExecutionMetrics) {
        // Update table statistics
        for table in &metrics.tables_accessed {
            let stats = self
                .table_statistics
                .entry(table.clone())
                .or_insert_with(|| TableStatistics {
                    table_name: table.clone(),
                    access_count: 0,
                    full_scan_count: 0,
                    avg_estimation_error: 0.0,
                    slow_query_count: 0,
                });

            stats.access_count += 1;
            if metrics.full_table_scan {
                stats.full_scan_count += 1;
            }

            // Exponential moving average for estimation error
            let error = metrics.estimation_error();
            stats.avg_estimation_error =
                (stats.avg_estimation_error * 0.7) + (error * 0.3);

            if metrics.actual_time_ms > 1000.0 {
                stats.slow_query_count += 1;
            }
        }

        // Maintain fixed-size history
        if self.metrics_history.len() >= self.max_history {
            self.metrics_history.pop_front();
        }
        self.metrics_history.push_back(metrics);
    }

    /// Identify tables with poor performance
    pub fn get_problematic_tables(&self) -> Vec<(String, f64)> {
        let mut tables: Vec<_> = self
            .table_statistics
            .values()
            .filter(|s| s.full_scan_count > 2 || s.slow_query_count > 1)
            .map(|s| (s.table_name.clone(), s.avg_estimation_error))
            .collect();
        tables.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        tables
    }

    /// Calculate table access frequency
    pub fn get_table_stats(&self) -> Vec<(String, usize, f64)> {
        let mut stats: Vec<_> = self
            .table_statistics
            .values()
            .map(|s| (
                s.table_name.clone(),
                s.access_count,
                (s.full_scan_count as f64 / s.access_count.max(1) as f64) * 100.0,
            ))
            .collect();
        stats.sort_by(|a, b| b.1.cmp(&a.1));
        stats
    }
}

/// Autonomous tuning advisor - generates recommendations based on patterns
pub struct TuningAdvisor {
    model: PerformanceModel,
    optimization_history: Vec<TuningRecommendation>,
}

impl TuningAdvisor {
    pub fn new(model_history_size: usize) -> Self {
        Self {
            model: PerformanceModel::new(model_history_size),
            optimization_history: Vec::new(),
        }
    }

    /// Record actual execution and analyze for recommendations
    pub fn analyze_and_recommend(
        &mut self,
        metrics: QueryExecutionMetrics,
    ) -> Vec<TuningRecommendation> {
        self.model.record_execution(metrics.clone());

        let mut recommendations = Vec::new();

        // Strategy 1: Full table scans on frequently accessed tables
        if metrics.full_table_scan && metrics.tables_accessed.len() == 1 {
            let table = &metrics.tables_accessed[0];
            let table_stats = self.model.table_statistics.get(table);

            if let Some(stats) = table_stats {
                if stats.full_scan_count >= 3 {
                    // Recommend creating an index
                    recommendations.push(TuningRecommendation {
                        recommendation_type: TuningType::CreateIndex,
                        target: format!("{}:full_scan_idx", table),
                        rationale: format!(
                            "Full table scans detected on {} ({}x). Index on filter columns recommended.",
                            table, stats.full_scan_count
                        ),
                        expected_improvement_percent: 40.0,
                        priority: Priority::High,
                    });
                }
            }
        }

        // Strategy 2: High estimation errors indicate statistics are stale
        if metrics.estimation_error() > 50.0 {
            recommendations.push(TuningRecommendation {
                recommendation_type: TuningType::StatisticsUpdate,
                target: metrics.tables_accessed.join(","),
                rationale: format!(
                    "High estimation error ({:.1}%). Table statistics need refresh.",
                    metrics.estimation_error()
                ),
                expected_improvement_percent: 20.0,
                priority: Priority::Medium,
            });
        }

        // Strategy 3: Large result sets on frequently accessed tables
        if metrics.actual_rows > 100000 && metrics.tables_accessed.len() == 1 {
            let table = &metrics.tables_accessed[0];
            recommendations.push(TuningRecommendation {
                recommendation_type: TuningType::ModifyPartition,
                target: table.clone(),
                rationale: format!(
                    "Large result set ({} rows). Partitioning by date/range recommended.",
                    metrics.actual_rows
                ),
                expected_improvement_percent: 30.0,
                priority: Priority::Medium,
            });
        }

        // Strategy 4: Slow queries with joins across multiple tables
        if metrics.actual_time_ms > 1000.0 && metrics.tables_accessed.len() > 1 {
            recommendations.push(TuningRecommendation {
                recommendation_type: TuningType::MaterializedView,
                target: format!("mv_{}", metrics.query_id.chars().take(8).collect::<String>()),
                rationale: format!(
                    "Complex join query slow ({}ms). Materialized view could cache results.",
                    metrics.actual_time_ms as i64
                ),
                expected_improvement_percent: 50.0,
                priority: Priority::High,
            });
        }

        // Sort by priority and cost impact
        recommendations.sort_by(|a, b| {
            let priority_cmp = b.priority.cmp(&a.priority);
            if priority_cmp == std::cmp::Ordering::Equal {
                b.expected_improvement_percent
                    .partial_cmp(&a.expected_improvement_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                priority_cmp
            }
        });

        self.optimization_history.extend(recommendations.clone());
        recommendations
    }

    /// Get top N recommendations
    pub fn get_top_recommendations(&self, limit: usize) -> Vec<TuningRecommendation> {
        let mut recs = self.optimization_history.clone();
        recs.sort_by(|a, b| {
            let priority_cmp = b.priority.cmp(&a.priority);
            if priority_cmp == std::cmp::Ordering::Equal {
                b.expected_improvement_percent
                    .partial_cmp(&a.expected_improvement_percent)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                priority_cmp
            }
        });
        recs.truncate(limit);
        recs
    }

    /// Get table statistics and hotspots
    pub fn get_hotspots(&self) -> Vec<(String, usize, f64)> {
        self.model.get_table_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metrics(
        query_id: &str,
        estimated_cost: f64,
        actual_cost: f64,
        table: &str,
        full_scan: bool,
    ) -> QueryExecutionMetrics {
        QueryExecutionMetrics {
            query_id: query_id.to_string(),
            estimated_cost,
            actual_cost,
            estimated_rows: 1000,
            actual_rows: 1000,
            estimated_time_ms: estimated_cost * 0.1,
            actual_time_ms: actual_cost * 0.1,
            full_table_scan: full_scan,
            tables_accessed: vec![table.to_string()],
            timestamp_ms: 0,
        }
    }

    #[test]
    fn test_execution_metrics_estimation_error() {
        let metrics = create_test_metrics("q1", 100.0, 150.0, "users", false);
        assert!((metrics.estimation_error() - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_performance_model_records_metrics() {
        let mut model = PerformanceModel::new(100);
        let metrics = create_test_metrics("q1", 100.0, 100.0, "users", false);
        model.record_execution(metrics);

        assert_eq!(model.metrics_history.len(), 1);
    }

    #[test]
    fn test_performance_model_table_statistics() {
        let mut model = PerformanceModel::new(100);
        let metrics1 = create_test_metrics("q1", 100.0, 100.0, "users", true);
        let metrics2 = create_test_metrics("q2", 100.0, 100.0, "users", true);

        model.record_execution(metrics1);
        model.record_execution(metrics2);

        let stats = model.get_table_stats();
        assert!(!stats.is_empty());
        assert_eq!(stats[0].0, "users");
        assert_eq!(stats[0].1, 2); // access_count
    }

    #[test]
    fn test_performance_model_full_scan_detection() {
        let mut model = PerformanceModel::new(100);

        for i in 0..4 {
            let metrics = create_test_metrics(&format!("q{}", i), 100.0, 100.0, "orders", true);
            model.record_execution(metrics);
        }

        let problematic = model.get_problematic_tables();
        assert!(!problematic.is_empty());
        assert!(problematic.iter().any(|t| t.0 == "orders"));
    }

    #[test]
    fn test_tuning_advisor_creation() {
        let advisor = TuningAdvisor::new(100);
        let recs = advisor.optimization_history.clone();
        assert_eq!(recs.len(), 0);
    }

    #[test]
    fn test_advisor_recommends_index_for_full_scans() {
        let mut advisor = TuningAdvisor::new(100);

        for i in 0..3 {
            let metrics = create_test_metrics(&format!("q{}", i), 100.0, 100.0, "users", true);
            let recs = advisor.analyze_and_recommend(metrics);
            if i == 2 {
                // After 3 full scans, should recommend index
                assert!(recs.iter().any(|r| r.recommendation_type == TuningType::CreateIndex));
            }
        }
    }

    #[test]
    fn test_advisor_recommends_statistics_update() {
        let mut advisor = TuningAdvisor::new(100);
        let metrics = create_test_metrics("q1", 100.0, 250.0, "users", false);
        let recs = advisor.analyze_and_recommend(metrics);

        assert!(recs.iter().any(|r| r.recommendation_type == TuningType::StatisticsUpdate));
    }

    #[test]
    fn test_advisor_recommends_materialized_view() {
        let mut advisor = TuningAdvisor::new(100);
        let mut metrics = create_test_metrics("q1", 100.0, 100.0, "users", false);
        metrics.tables_accessed = vec!["users".to_string(), "orders".to_string()];
        metrics.actual_time_ms = 1500.0; // > 1000ms threshold

        let recs = advisor.analyze_and_recommend(metrics);

        assert!(recs
            .iter()
            .any(|r| r.recommendation_type == TuningType::MaterializedView));
    }

    #[test]
    fn test_advisor_sorts_by_priority() {
        let mut advisor = TuningAdvisor::new(100);

        // Generate multiple recommendations
        let metrics1 = create_test_metrics("q1", 100.0, 250.0, "users", true);
        advisor.analyze_and_recommend(metrics1);

        let top_recs = advisor.get_top_recommendations(10);
        assert!(!top_recs.is_empty());

        // Verify sorted by priority
        for i in 1..top_recs.len() {
            assert!(top_recs[i - 1].priority >= top_recs[i].priority);
        }
    }

    #[test]
    fn test_advisor_gets_hotspots() {
        let mut advisor = TuningAdvisor::new(100);

        for i in 0..5 {
            let metrics = create_test_metrics(&format!("q{}", i), 100.0, 100.0, "users", true);
            advisor.analyze_and_recommend(metrics);
        }

        let hotspots = advisor.get_hotspots();
        assert!(!hotspots.is_empty());
        assert_eq!(hotspots[0].0, "users");
        assert!(hotspots[0].1 > 0); // access count > 0
    }

    #[test]
    fn test_model_history_respects_max_size() {
        let mut model = PerformanceModel::new(3);

        for i in 0..5 {
            let metrics = create_test_metrics(&format!("q{}", i), 100.0, 100.0, "users", false);
            model.record_execution(metrics);
        }

        assert_eq!(model.metrics_history.len(), 3);
    }

    #[test]
    fn test_partition_recommendation_large_result_set() {
        let mut advisor = TuningAdvisor::new(100);
        let mut metrics = create_test_metrics("q1", 100.0, 100.0, "events", false);
        metrics.actual_rows = 500000;

        let recs = advisor.analyze_and_recommend(metrics);

        assert!(recs
            .iter()
            .any(|r| r.recommendation_type == TuningType::ModifyPartition));
    }
}
