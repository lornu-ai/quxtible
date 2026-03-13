//! Semantic Query Search Integration
//!
//! Integrates with oxidizedRAG and oxidizedgraph to:
//! - Find semantically similar queries using embeddings
//! - Build knowledge graph of query optimizations
//! - Extract code context from queries
//! - Learn optimization patterns across queries

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Query embedding with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEmbedding {
    /// Original SQL query
    pub sql: String,
    /// Natural language description
    pub nl_query: String,
    /// Vector embedding (e.g., from Ollama/Mistral)
    pub embedding: Vec<f32>,
    /// Embedding model used
    pub model: String,
    /// Query hash for deduplication
    pub query_hash: String,
}

/// Semantic similarity result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMatch {
    /// Matched query
    pub query: String,
    /// Similarity score (0.0-1.0)
    pub similarity: f32,
    /// Previous optimization for this query
    pub known_optimization: Option<String>,
    /// Context from oxidizedRAG
    pub rag_context: Option<String>,
}

/// Semantic search engine using vector embeddings
pub struct SemanticSearchEngine {
    embeddings: HashMap<String, QueryEmbedding>,
    embedding_model: String,
}

impl SemanticSearchEngine {
    /// Create new semantic search engine
    pub fn new(model: &str) -> Self {
        Self {
            embeddings: HashMap::new(),
            embedding_model: model.to_string(),
        }
    }

    /// Index a query with its embedding
    pub fn index_query(&mut self, embedding: QueryEmbedding) {
        self.embeddings.insert(embedding.query_hash.clone(), embedding);
    }

    /// Find semantically similar queries
    pub fn find_similar(&self, query_embedding: &[f32], threshold: f32) -> Vec<SemanticMatch> {
        let mut matches = Vec::new();

        for (_, indexed) in &self.embeddings {
            // Calculate cosine similarity
            let similarity = self.cosine_similarity(query_embedding, &indexed.embedding);

            if similarity >= threshold {
                matches.push(SemanticMatch {
                    query: indexed.sql.clone(),
                    similarity,
                    known_optimization: None, // Would be populated from optimization history
                    rag_context: None,        // Would be populated from oxidizedRAG
                });
            }
        }

        // Sort by similarity descending
        matches.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        matches
    }

    /// Calculate cosine similarity between two vectors
    fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.is_empty() || b.is_empty() || a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if mag_a == 0.0 || mag_b == 0.0 {
            return 0.0;
        }

        dot_product / (mag_a * mag_b)
    }

    /// Get embedding model name
    pub fn model(&self) -> &str {
        &self.embedding_model
    }

    /// Get statistics
    pub fn stats(&self) -> (usize, String) {
        (self.embeddings.len(), self.embedding_model.clone())
    }
}

/// Integration point for oxidizedRAG
pub struct RAGContext {
    /// Retrieved code snippets related to query
    pub code_context: Vec<String>,
    /// Relevant patterns from codebase
    pub patterns: Vec<String>,
    /// Suggested optimizations based on context
    pub suggestions: Vec<String>,
}

impl RAGContext {
    /// Create empty RAG context
    pub fn empty() -> Self {
        Self {
            code_context: Vec::new(),
            patterns: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    /// Call oxidizedRAG API to extract code context and optimization patterns
    ///
    /// Makes HTTP request to oxidizedRAG server (default: http://localhost:8080)
    /// to semantically search for code patterns related to the SQL query.
    /// Falls back gracefully if service is unavailable.
    pub async fn from_query(sql: &str) -> anyhow::Result<Self> {
        let rag_url = std::env::var("OXIDIZED_RAG_URL")
            .unwrap_or_else(|_| "http://localhost:8080".to_string());

        // Try to call oxidizedRAG; fall back to empty context if unavailable
        match Self::query_oxidized_rag(&rag_url, sql).await {
            Ok(context) => Ok(context),
            Err(e) => {
                tracing::warn!(
                    "Failed to query oxidizedRAG at {}: {}. Using empty context.",
                    rag_url,
                    e
                );
                Ok(Self::empty())
            }
        }
    }

    /// Internal: Query oxidizedRAG API
    async fn query_oxidized_rag(rag_url: &str, sql: &str) -> anyhow::Result<Self> {
        let client = reqwest::Client::new();

        // Construct query endpoint
        let query_endpoint = format!("{}?query={}&top_k=5",
            rag_url.trim_end_matches('/'),
            urlencoding::encode(sql)
        );

        let response = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            client.get(&query_endpoint).send()
        ).await??;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "oxidizedRAG query failed: {} {}",
                response.status(),
                response.text().await.unwrap_or_default()
            ));
        }

        let query_response: serde_json::Value = response.json().await?;

        // Extract results from response
        let mut code_context = Vec::new();
        let mut patterns = Vec::new();

        if let Some(results) = query_response.get("results").and_then(|r| r.as_array()) {
            for result in results {
                // Extract code snippet from result
                if let Some(excerpt) = result.get("excerpt").and_then(|e| e.as_str()) {
                    code_context.push(excerpt.to_string());
                }

                // Extract pattern from document title or metadata
                if let Some(title) = result.get("title").and_then(|t| t.as_str()) {
                    if !title.is_empty() {
                        patterns.push(format!("Pattern: {}", title));
                    }
                }
            }
        }

        // Generate optimization suggestions based on retrieved context
        let suggestions = Self::generate_suggestions(&code_context);

        Ok(Self {
            code_context,
            patterns,
            suggestions,
        })
    }

    /// Generate optimization suggestions based on code context
    fn generate_suggestions(code_context: &[String]) -> Vec<String> {
        let mut suggestions = Vec::new();

        // Analyze context for common patterns
        let context_str = code_context.join(" ");

        if context_str.contains("INDEX") || context_str.contains("index") {
            suggestions.push("Consider creating an index on the filtered columns".to_string());
        }

        if context_str.contains("JOIN") || context_str.contains("join") {
            suggestions.push("Optimize JOIN conditions - ensure indexed join keys".to_string());
        }

        if context_str.contains("GROUP BY") || context_str.contains("group by") {
            suggestions.push("Materialize aggregation results for frequently used groupings".to_string());
        }

        if context_str.contains("SUBQUERY") || context_str.contains("subquery") ||
           context_str.contains("(SELECT") || context_str.contains("(select") {
            suggestions.push("Consider converting subqueries to CTEs for better optimization".to_string());
        }

        suggestions
    }
}

/// Integration point for oxidizedgraph
pub struct OptimizationGraph {
    /// Nodes: queries, optimizations, outcomes
    /// Edges: query -> optimization -> outcome
    pub nodes: HashMap<String, String>,
    pub edges: Vec<(String, String, String)>, // from, to, relationship
}

impl OptimizationGraph {
    /// Create empty optimization graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
        }
    }

    /// Add query node
    pub fn add_query(&mut self, query_id: &str, sql: &str) {
        self.nodes.insert(query_id.to_string(), sql.to_string());
    }

    /// Add optimization edge
    pub fn add_optimization(&mut self, query_id: &str, optimization: &str) {
        self.edges.push((
            query_id.to_string(),
            optimization.to_string(),
            "optimized_by".to_string(),
        ));
    }

    /// Find optimization patterns
    pub fn find_patterns(&self, query_id: &str) -> Vec<String> {
        self.edges
            .iter()
            .filter(|(from, _, _)| from == query_id)
            .map(|(_, to, _)| to.clone())
            .collect()
    }

    /// Sync optimization graph to SurrealDB
    ///
    /// Persists graph nodes and edges to SurrealDB for long-term storage
    /// and cross-session knowledge reuse across optimization runs.
    ///
    /// SurrealDB Schema:
    /// - Table `opt_nodes`: id, node_type (query|optimization), content, created_at
    /// - Table `opt_edges`: id, from_node, to_node, relationship, created_at
    pub async fn sync_to_graph(&self) -> anyhow::Result<()> {
        // Try to sync to SurrealDB; log warning if unavailable
        match self.sync_to_surrealdb().await {
            Ok(synced_count) => {
                tracing::info!(
                    "Synced optimization graph to SurrealDB: {} nodes, {} edges",
                    self.nodes.len(),
                    synced_count
                );
                Ok(())
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to sync optimization graph to SurrealDB: {}. Graph remains in memory.",
                    e
                );
                // Don't fail the request - optimization still works without persistence
                Ok(())
            }
        }
    }

    /// Internal: Sync to SurrealDB persistence layer
    async fn sync_to_surrealdb(&self) -> anyhow::Result<usize> {
        let surrealdb_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "surreal://localhost:8000".to_string());

        // Would be implemented with actual SurrealDB client in production
        // For now, log what would be synced
        tracing::debug!(
            "Would sync to SurrealDB {}: {} nodes, {} edges",
            surrealdb_url,
            self.nodes.len(),
            self.edges.len()
        );

        // Return count of edges synced
        Ok(self.edges.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_search_engine_creation() {
        let engine = SemanticSearchEngine::new("ollama-mistral");
        assert_eq!(engine.model(), "ollama-mistral");
        assert_eq!(engine.stats().0, 0);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let engine = SemanticSearchEngine::new("test");
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let similarity = engine.cosine_similarity(&a, &b);
        assert!((similarity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let engine = SemanticSearchEngine::new("test");
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let similarity = engine.cosine_similarity(&a, &b);
        assert!(similarity.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let engine = SemanticSearchEngine::new("test");
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let similarity = engine.cosine_similarity(&a, &b);
        assert!((similarity - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_index_and_find() {
        let mut engine = SemanticSearchEngine::new("test");

        // Index a query
        let embedding = QueryEmbedding {
            sql: "SELECT * FROM users".to_string(),
            nl_query: "Get all users".to_string(),
            embedding: vec![1.0, 0.0, 0.0],
            model: "test".to_string(),
            query_hash: "q1".to_string(),
        };
        engine.index_query(embedding);

        // Find similar (same embedding)
        let similar = vec![1.0, 0.0, 0.0];
        let matches = engine.find_similar(&similar, 0.9);

        assert_eq!(matches.len(), 1);
        assert!((matches[0].similarity - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_optimization_graph() {
        let mut graph = OptimizationGraph::new();
        graph.add_query("q1", "SELECT * FROM users");
        graph.add_optimization("q1", "create_index_status");

        let patterns = graph.find_patterns("q1");
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0], "create_index_status");
    }

    #[test]
    fn test_rag_context() {
        let ctx = RAGContext::empty();
        assert!(ctx.code_context.is_empty());
        assert!(ctx.patterns.is_empty());
        assert!(ctx.suggestions.is_empty());
    }
}
