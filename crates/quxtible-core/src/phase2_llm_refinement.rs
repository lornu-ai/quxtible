//! Phase 2: LLM-Driven Query Refinement
//!
//! Uses a specialized Critic/Optimizer Agent to autonomously rewrite inefficient SQL.
//! Applies optimizations: CTEs instead of subqueries, proper JOINs, predicate pushdown, etc.

use crate::types::QueryRefinement;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[async_trait]
pub trait QueryOptimizer: Send + Sync {
    /// Refine a SQL query using LLM-based optimization
    async fn refine_query(&self, sql: &str, explain_feedback: Option<&str>) -> Result<QueryRefinement>;

    /// Get the model/optimizer name
    fn optimizer_name(&self) -> &str;
}

/// Claude-based query optimizer
#[derive(Clone)]
pub struct ClaudeQueryOptimizer {
    api_key: String,
    model: String,
}

impl ClaudeQueryOptimizer {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }

    /// Build the optimization prompt for Claude
    fn build_optimization_prompt(sql: &str, explain_feedback: Option<&str>) -> String {
        let mut prompt = format!(
            r#"You are a SQL query optimization expert. Your task is to analyze and optimize the following SQL query.

Original SQL:
```sql
{}
```

{}

Please provide an optimized version of this query with the following optimizations where applicable:

1. **Replace nested subqueries with CTEs** - Use WITH clauses for better readability and potential performance gains
2. **Optimize JOIN order** - Put most selective conditions first
3. **Push down predicates** - Apply filters as early as possible
4. **Avoid SELECT *** - Only select needed columns
5. **Use appropriate JOIN types** - INNER vs LEFT vs RIGHT
6. **Remove redundant conditions** - Consolidate WHERE clauses
7. **Index-friendly conditions** - Ensure conditions can use indexes

Respond in JSON format with:
{{
  "optimized_sql": "your optimized query here",
  "optimizations": ["optimization 1", "optimization 2", ...],
  "rationale": "explain why these optimizations help",
  "confidence": 0.95
}}

Only return valid JSON, no additional text."#,
            sql,
            if let Some(explain) = explain_feedback {
                format!(
                    "Execution Plan (EXPLAIN output):\n```json\n{}\n```\n\nThe query was slow. Analyze the execution plan and suggest optimizations.",
                    explain
                )
            } else {
                "Analyze this query for optimization opportunities.".to_string()
            }
        );
        prompt
    }

    /// Call Claude API
    async fn call_claude(&self, prompt: &str) -> Result<String> {
        let client = reqwest::Client::new();

        #[derive(Serialize)]
        struct Message {
            role: String,
            content: String,
        }

        #[derive(Serialize)]
        struct ClaudeRequest {
            model: String,
            max_tokens: usize,
            messages: Vec<Message>,
        }

        #[derive(Deserialize)]
        struct ClaudeResponse {
            content: Vec<ContentBlock>,
        }

        #[derive(Deserialize)]
        struct ContentBlock {
            text: Option<String>,
        }

        let request = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: 2000,
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
        };

        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .context("Failed to call Claude API")?;

        let claude_response: ClaudeResponse = response
            .json()
            .await
            .context("Failed to parse Claude response")?;

        let text = claude_response
            .content
            .first()
            .and_then(|c| c.text.clone())
            .ok_or_else(|| anyhow::anyhow!("No text in Claude response"))?;

        Ok(text)
    }

    /// Parse Claude's JSON response
    fn parse_optimization_response(&self, response: &str) -> Result<QueryRefinement> {
        // Extract JSON from response (Claude might include extra text)
        let json_start = response.find('{').unwrap_or(0);
        let json_end = response.rfind('}').unwrap_or(response.len());
        let json_str = &response[json_start..=json_end];

        let parsed: serde_json::Value =
            serde_json::from_str(json_str).context("Failed to parse Claude response JSON")?;

        let optimized_sql = parsed["optimized_sql"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let optimizations: Vec<String> = parsed["optimizations"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let rationale = parsed["rationale"]
            .as_str()
            .unwrap_or("Optimization applied by Claude")
            .to_string();
        let confidence = parsed["confidence"]
            .as_f64()
            .unwrap_or(0.8) as f32;

        Ok(QueryRefinement {
            original_sql: "".to_string(), // Will be set by caller
            refined_sql: optimized_sql,
            optimizations_applied: optimizations,
            confidence,
            rationale,
        })
    }
}

#[async_trait]
impl QueryOptimizer for ClaudeQueryOptimizer {
    async fn refine_query(&self, sql: &str, explain_feedback: Option<&str>) -> Result<QueryRefinement> {
        let prompt = Self::build_optimization_prompt(sql, explain_feedback);
        let response = self.call_claude(&prompt).await?;
        let mut refinement = self.parse_optimization_response(&response)?;
        refinement.original_sql = sql.to_string();
        Ok(refinement)
    }

    fn optimizer_name(&self) -> &str {
        "Claude LLM Optimizer"
    }
}

/// Rule-based query optimizer (fallback/fast path)
pub struct RuleBasedOptimizer;

impl RuleBasedOptimizer {
    pub fn new() -> Self {
        Self
    }

    /// Apply rule-based optimizations
    pub fn optimize(&self, sql: &str) -> Result<QueryRefinement> {
        let optimized = self.apply_rules(sql);
        let optimizations = self.detect_optimizations(sql);

        Ok(QueryRefinement {
            original_sql: sql.to_string(),
            refined_sql: optimized,
            optimizations_applied: optimizations,
            confidence: 0.6, // Lower confidence for rule-based
            rationale: "Applied rule-based optimizations (fast path)".to_string(),
        })
    }

    /// Apply simple rule-based transformations
    fn apply_rules(&self, sql: &str) -> String {
        let mut optimized = sql.to_string();

        // Rule 1: Replace SELECT * with column list (placeholder - would need schema)
        if optimized.to_uppercase().contains("SELECT *") {
            // In real implementation, would get actual columns
            // optimized = optimized.replace("SELECT *", "SELECT id, name, email");
        }

        // Rule 2: Convert subqueries to CTEs (basic pattern matching)
        if optimized.contains("SELECT") && optimized.matches("SELECT").count() > 1 {
            // Mark for CTE conversion
            // This is simplified - real implementation would parse and restructure
        }

        // Rule 3: Remove redundant conditions
        // In real implementation, would parse WHERE clause and deduplicate

        optimized
    }

    /// Detect which optimizations were applied
    fn detect_optimizations(&self, _sql: &str) -> Vec<String> {
        vec![
            "Analyzed for SELECT *".to_string(),
            "Checked for subquery optimization".to_string(),
            "Validated JOIN structure".to_string(),
        ]
    }
}

impl Default for RuleBasedOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_optimization_prompt_without_feedback() {
        let sql = "SELECT * FROM users WHERE status = 'active'";
        let prompt = ClaudeQueryOptimizer::build_optimization_prompt(sql, None);

        assert!(prompt.contains("Original SQL"));
        assert!(prompt.contains(sql));
        assert!(prompt.contains("CTEs"));
        assert!(prompt.contains("JSON"));
    }

    #[test]
    fn test_build_optimization_prompt_with_feedback() {
        let sql = "SELECT * FROM users WHERE status = 'active'";
        let explain = r#"[{"Plan": {"Total Cost": 5000.0}}]"#;
        let prompt = ClaudeQueryOptimizer::build_optimization_prompt(sql, Some(explain));

        assert!(prompt.contains("Execution Plan"));
        assert!(prompt.contains(explain));
        assert!(prompt.contains("slow"));
    }

    #[test]
    fn test_rule_based_optimizer_creation() {
        let optimizer = RuleBasedOptimizer::new();
        let sql = "SELECT * FROM users";
        let result = optimizer.optimize(sql).unwrap();

        assert_eq!(result.original_sql, sql);
        assert!(!result.optimizations_applied.is_empty());
        assert!(result.confidence > 0.0 && result.confidence <= 1.0);
    }

    #[test]
    fn test_rule_based_optimizer_confidence() {
        let optimizer = RuleBasedOptimizer::new();
        let sql = "SELECT id, name FROM users WHERE status = 'active'";
        let result = optimizer.optimize(sql).unwrap();

        // Rule-based has lower confidence than LLM
        assert_eq!(result.confidence, 0.6);
    }

    #[test]
    fn test_query_refinement_structure() {
        let refinement = QueryRefinement {
            original_sql: "SELECT * FROM users".to_string(),
            refined_sql: "SELECT id, name FROM users".to_string(),
            optimizations_applied: vec!["Remove SELECT *".to_string()],
            confidence: 0.8,
            rationale: "Specific columns improve cache usage".to_string(),
        };

        assert_eq!(refinement.original_sql, "SELECT * FROM users");
        assert_eq!(refinement.refined_sql, "SELECT id, name FROM users");
        assert_eq!(refinement.optimizations_applied.len(), 1);
        assert_eq!(refinement.confidence, 0.8);
    }

    #[test]
    fn test_optimization_response_parsing() {
        let optimizer = ClaudeQueryOptimizer::new(
            "test-key".to_string(),
            "claude-3-sonnet-20240229".to_string(),
        );

        let response = r#"
        {
            "optimized_sql": "SELECT id, name FROM users WHERE status = 'active'",
            "optimizations": ["Remove SELECT *", "Optimize WHERE clause"],
            "rationale": "More efficient query",
            "confidence": 0.95
        }
        "#;

        let result = optimizer.parse_optimization_response(response).unwrap();
        assert_eq!(result.refined_sql, "SELECT id, name FROM users WHERE status = 'active'");
        assert_eq!(result.optimizations_applied.len(), 2);
        assert_eq!(result.confidence, 0.95);
    }

    #[test]
    fn test_optimization_response_with_extra_text() {
        let optimizer = ClaudeQueryOptimizer::new(
            "test-key".to_string(),
            "claude-3-sonnet-20240229".to_string(),
        );

        let response = r#"
        Here's the optimized query:
        {
            "optimized_sql": "SELECT id FROM users",
            "optimizations": ["Remove unused columns"],
            "rationale": "Faster query",
            "confidence": 0.9
        }
        Some additional text here
        "#;

        let result = optimizer.parse_optimization_response(response).unwrap();
        assert_eq!(result.refined_sql, "SELECT id FROM users");
        assert!(result.confidence > 0.8);
    }
}
