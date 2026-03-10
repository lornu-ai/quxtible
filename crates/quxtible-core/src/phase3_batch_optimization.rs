//! Phase 3: Batch Query Optimization
//!
//! For multi-agent workflows, consolidates overlapping or redundant sub-queries
//! into a structured query plan graph (Halo pattern). Shares context caches,
//! batches similar queries, and eliminates redundant computations.

use crate::types::{BatchQueryPlan, QueryRequest};
use anyhow::Result;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Batch query optimizer for multi-agent scenarios
pub struct BatchOptimizer {
    similarity_threshold: f32,
}

impl BatchOptimizer {
    pub fn new() -> Self {
        Self {
            similarity_threshold: 0.7, // 70% similarity = consider same
        }
    }

    pub fn with_threshold(threshold: f32) -> Self {
        Self {
            similarity_threshold: threshold,
        }
    }

    /// Consolidate multiple queries into an optimized batch plan
    pub fn optimize_batch(&self, queries: Vec<QueryRequest>) -> Result<BatchQueryPlan> {
        if queries.is_empty() {
            return Ok(BatchQueryPlan {
                queries: vec![],
                consolidated_plan: String::new(),
                shared_context: BTreeMap::new(),
                execution_order: vec![],
            });
        }

        // Step 1: Find similar queries
        let similarity_groups = self.find_similar_queries(&queries);

        // Step 2: Extract shared context from similar queries
        let shared_context = self.extract_shared_context(&queries, &similarity_groups);

        // Step 3: Build query execution DAG (Halo pattern)
        let execution_dag = self.build_execution_dag(&queries, &similarity_groups);

        // Step 4: Compute optimal execution order (topological sort with cache consideration)
        let execution_order = self.compute_execution_order(&queries, &execution_dag);

        // Step 5: Build consolidated SQL plan
        let consolidated_plan = self.build_consolidated_plan(&queries, &similarity_groups, &shared_context);

        Ok(BatchQueryPlan {
            queries,
            consolidated_plan,
            shared_context,
            execution_order,
        })
    }

    /// Detect query similarities (cosine similarity on SQL tokens)
    fn find_similar_queries(&self, queries: &[QueryRequest]) -> Vec<Vec<usize>> {
        let mut groups: Vec<Vec<usize>> = vec![];
        let mut assigned = HashSet::new();

        for i in 0..queries.len() {
            if assigned.contains(&i) {
                continue;
            }

            let mut group = vec![i];
            assigned.insert(i);

            for j in (i + 1)..queries.len() {
                if assigned.contains(&j) {
                    continue;
                }

                let similarity = self.compute_sql_similarity(&queries[i].sql, &queries[j].sql);
                if similarity >= self.similarity_threshold {
                    group.push(j);
                    assigned.insert(j);
                }
            }

            groups.push(group);
        }

        groups
    }

    /// Compute SQL similarity using token overlap
    fn compute_sql_similarity(&self, sql1: &str, sql2: &str) -> f32 {
        let lower1 = sql1.to_lowercase();
        let lower2 = sql2.to_lowercase();

        let tokens1: HashSet<String> = lower1
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let tokens2: HashSet<String> = lower2
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        if tokens1.is_empty() || tokens2.is_empty() {
            if tokens1 == tokens2 {
                1.0
            } else {
                0.0
            }
        } else {
            let intersection = tokens1.intersection(&tokens2).count() as f32;
            let union = tokens1.union(&tokens2).count() as f32;
            intersection / union
        }
    }

    /// Extract shared context from similar queries
    fn extract_shared_context(
        &self,
        queries: &[QueryRequest],
        similarity_groups: &[Vec<usize>],
    ) -> BTreeMap<String, serde_json::Value> {
        let mut context = BTreeMap::new();

        for group in similarity_groups {
            if group.len() > 1 {
                // Multiple similar queries detected
                let table_names = self.extract_tables(queries, group);
                if !table_names.is_empty() {
                    context.insert(
                        format!("shared_tables_group_{}", group[0]),
                        serde_json::json!(table_names),
                    );
                }

                // Extract common filters
                let common_conditions = self.extract_common_conditions(queries, group);
                if !common_conditions.is_empty() {
                    context.insert(
                        format!("common_filters_group_{}", group[0]),
                        serde_json::json!(common_conditions),
                    );
                }
            }
        }

        context
    }

    /// Extract table names from queries
    fn extract_tables(&self, queries: &[QueryRequest], indices: &[usize]) -> Vec<String> {
        let mut tables = HashSet::new();

        for &idx in indices {
            let sql = &queries[idx].sql;
            let upper = sql.to_uppercase();

            // Simple extraction: find FROM <table>
            if let Some(from_pos) = upper.find("FROM") {
                let after_from = &sql[from_pos + 4..];
                let table = after_from
                    .trim()
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !table.is_empty() {
                    tables.insert(table);
                }
            }
        }

        tables.into_iter().collect()
    }

    /// Extract common conditions/filters
    fn extract_common_conditions(&self, queries: &[QueryRequest], indices: &[usize]) -> Vec<String> {
        if indices.len() < 2 {
            return vec![];
        }

        let mut conditions: Vec<HashSet<String>> = vec![];

        for &idx in indices {
            let sql = &queries[idx].sql;
            let upper = sql.to_uppercase();

            if let Some(where_pos) = upper.find("WHERE") {
                let where_clause = &sql[where_pos + 5..];
                let conds: HashSet<String> = where_clause
                    .split("AND")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                conditions.push(conds);
            }
        }

        if conditions.is_empty() {
            return vec![];
        }

        // Find intersection (common conditions)
        let mut common = conditions[0].clone();
        for cond_set in &conditions[1..] {
            common.retain(|c| cond_set.contains(c));
        }

        common.into_iter().collect()
    }

    /// Build execution DAG (directed acyclic graph)
    fn build_execution_dag(
        &self,
        queries: &[QueryRequest],
        similarity_groups: &[Vec<usize>],
    ) -> HashMap<usize, Vec<usize>> {
        let mut dag: HashMap<usize, Vec<usize>> = HashMap::new();

        // Initialize with no dependencies
        for i in 0..queries.len() {
            dag.insert(i, vec![]);
        }

        // Add dependencies: if query i's result can help query j, add edge i -> j
        for i in 0..queries.len() {
            for j in (i + 1)..queries.len() {
                // Check if i and j are in same similarity group
                for group in similarity_groups {
                    if group.contains(&i) && group.contains(&j) {
                        // Same group: execute in order to enable caching
                        if let Some(deps) = dag.get_mut(&j) {
                            deps.push(i);
                        }
                    }
                }
            }
        }

        dag
    }

    /// Compute optimal execution order (topological sort with cache optimization)
    fn compute_execution_order(
        &self,
        queries: &[QueryRequest],
        dag: &HashMap<usize, Vec<usize>>,
    ) -> Vec<usize> {
        let mut result = vec![];
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        // Topological sort using DFS
        for i in 0..queries.len() {
            if !visited.contains(&i) {
                self.topological_visit(i, dag, &mut result, &mut visited, &mut visiting);
            }
        }

        result
    }

    fn topological_visit(
        &self,
        node: usize,
        dag: &HashMap<usize, Vec<usize>>,
        result: &mut Vec<usize>,
        visited: &mut HashSet<usize>,
        visiting: &mut HashSet<usize>,
    ) {
        if visited.contains(&node) {
            return;
        }

        if visiting.contains(&node) {
            return; // Cycle detected (shouldn't happen in this context)
        }

        visiting.insert(node);

        if let Some(neighbors) = dag.get(&node) {
            for &neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    self.topological_visit(neighbor, dag, result, visited, visiting);
                }
            }
        }

        visiting.remove(&node);
        visited.insert(node);
        result.push(node);
    }

    /// Build consolidated SQL plan
    fn build_consolidated_plan(
        &self,
        queries: &[QueryRequest],
        similarity_groups: &[Vec<usize>],
        shared_context: &BTreeMap<String, serde_json::Value>,
    ) -> String {
        let mut plan = String::from("-- Consolidated Batch Query Plan\n\n");

        // Document shared context
        if !shared_context.is_empty() {
            plan.push_str("-- Shared Context:\n");
            for (key, value) in shared_context {
                plan.push_str(&format!("-- {}: {}\n", key, value));
            }
            plan.push_str("\n");
        }

        // Document similarity groups
        if !similarity_groups.is_empty() {
            plan.push_str("-- Query Groups:\n");
            for (group_idx, group) in similarity_groups.iter().enumerate() {
                if group.len() > 1 {
                    plan.push_str(&format!(
                        "-- Group {}: Queries {} (similarity {}%)\n",
                        group_idx,
                        group
                            .iter()
                            .map(|i| i.to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                        (self.similarity_threshold * 100.0) as u32
                    ));
                }
            }
            plan.push_str("\n");
        }

        // Add all queries in order
        for (idx, query) in queries.iter().enumerate() {
            plan.push_str(&format!("-- Query {}\n", idx));
            plan.push_str(&query.sql);
            plan.push_str(";\n\n");
        }

        plan
    }
}

impl Default for BatchOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{DatabaseType, QueryContext};

    fn create_test_query(sql: &str, agent_id: &str) -> QueryRequest {
        QueryRequest {
            nl_query: sql.to_string(),
            sql: sql.to_string(),
            database: DatabaseType::PostgreSQL,
            schema: None,
            context: QueryContext {
                agent_id: agent_id.to_string(),
                user_id: None,
                session_id: "test-session".to_string(),
                timestamp_ms: 0,
            },
        }
    }

    #[test]
    fn test_batch_optimizer_empty_queries() {
        let optimizer = BatchOptimizer::new();
        let result = optimizer.optimize_batch(vec![]).unwrap();

        assert_eq!(result.queries.len(), 0);
        assert_eq!(result.execution_order.len(), 0);
    }

    #[test]
    fn test_batch_optimizer_single_query() {
        let optimizer = BatchOptimizer::new();
        let query = create_test_query("SELECT * FROM users", "agent-1");
        let result = optimizer.optimize_batch(vec![query]).unwrap();

        assert_eq!(result.queries.len(), 1);
        assert_eq!(result.execution_order.len(), 1);
        assert_eq!(result.execution_order[0], 0);
    }

    #[test]
    fn test_sql_similarity_identical() {
        let optimizer = BatchOptimizer::new();
        let sql1 = "SELECT * FROM users WHERE status = 'active'";
        let sql2 = "SELECT * FROM users WHERE status = 'active'";

        let similarity = optimizer.compute_sql_similarity(sql1, sql2);
        assert_eq!(similarity, 1.0);
    }

    #[test]
    fn test_sql_similarity_different() {
        let optimizer = BatchOptimizer::new();
        let sql1 = "SELECT * FROM users WHERE status = 'active'";
        let sql2 = "SELECT * FROM products WHERE price > 100";

        let similarity = optimizer.compute_sql_similarity(sql1, sql2);
        assert!(similarity < 0.5);
    }

    #[test]
    fn test_sql_similarity_partial() {
        let optimizer = BatchOptimizer::new();
        let sql1 = "SELECT id, name FROM users WHERE status = 'active'";
        let sql2 = "SELECT id, name FROM users WHERE status = 'inactive'";

        let similarity = optimizer.compute_sql_similarity(sql1, sql2);
        assert!(similarity > 0.6 && similarity < 0.9);
    }

    #[test]
    fn test_find_similar_queries() {
        let optimizer = BatchOptimizer::with_threshold(0.7);
        let q1 = create_test_query("SELECT * FROM users WHERE status = 'active'", "agent-1");
        let q2 = create_test_query("SELECT * FROM users WHERE status = 'active'", "agent-2");
        let q3 = create_test_query("SELECT * FROM products WHERE price > 100", "agent-3");

        let groups = optimizer.find_similar_queries(&[q1, q2, q3]);

        // Should have 2 groups: [0, 1] (similar) and [2] (different)
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().any(|g| g.len() == 2)); // One group with 2 queries
        assert!(groups.iter().any(|g| g.len() == 1)); // One group with 1 query
    }

    #[test]
    fn test_extract_tables() {
        let optimizer = BatchOptimizer::new();
        let q1 = create_test_query("SELECT * FROM users WHERE id = 1", "agent-1");
        let q2 = create_test_query("SELECT * FROM orders WHERE id = 2", "agent-1");

        let tables = optimizer.extract_tables(&[q1, q2], &[0, 1]);

        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"orders".to_string()));
    }

    #[test]
    fn test_batch_optimizer_execution_order() {
        let optimizer = BatchOptimizer::new();
        let q1 = create_test_query("SELECT * FROM users WHERE id = 1", "agent-1");
        let q2 = create_test_query("SELECT * FROM users WHERE id = 2", "agent-1");
        let q3 = create_test_query("SELECT * FROM products WHERE id = 1", "agent-2");

        let result = optimizer.optimize_batch(vec![q1, q2, q3]).unwrap();

        assert_eq!(result.execution_order.len(), 3);
        // All queries should be present in execution order
        assert!(result.execution_order.contains(&0));
        assert!(result.execution_order.contains(&1));
        assert!(result.execution_order.contains(&2));
    }

    #[test]
    fn test_consolidated_plan_structure() {
        let optimizer = BatchOptimizer::new();
        let q1 = create_test_query("SELECT * FROM users", "agent-1");
        let q2 = create_test_query("SELECT * FROM orders", "agent-2");

        let result = optimizer.optimize_batch(vec![q1, q2]).unwrap();

        assert!(result.consolidated_plan.contains("Consolidated Batch Query Plan"));
        assert!(result.consolidated_plan.contains("SELECT * FROM users"));
        assert!(result.consolidated_plan.contains("SELECT * FROM orders"));
    }

    #[test]
    fn test_threshold_configuration() {
        let optimizer_strict = BatchOptimizer::with_threshold(0.9);
        let optimizer_loose = BatchOptimizer::with_threshold(0.5);

        let q1 = create_test_query("SELECT * FROM users WHERE status = 'active'", "agent-1");
        let q2 = create_test_query("SELECT * FROM users WHERE status = 'inactive'", "agent-2");

        let strict_groups = optimizer_strict.find_similar_queries(&[q1.clone(), q2.clone()]);
        let loose_groups = optimizer_loose.find_similar_queries(&[q1, q2]);

        // Strict should have 2 groups, loose should have 1
        assert_eq!(strict_groups.len(), 2);
        assert_eq!(loose_groups.len(), 1);
    }
}
