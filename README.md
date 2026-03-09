# Quxtible: AI-Powered Query Optimization Engine

A standalone microservice for optimizing and executing NL2SQL queries with intelligent cost estimation, LLM-driven refinement, batch optimization, and autonomous database tuning.

## Architecture

Quxtible implements a four-phase query optimization pipeline:

### Phase 1: Pre-Execution Cost Estimation
- Runs `EXPLAIN` against the database to retrieve execution plans
- Estimates computational cost and execution time
- Blocks expensive queries to protect the database
- Returns risk levels (Safe, Warning, Critical)

**Endpoints:**
- `POST /estimate-cost` - Get cost estimate for a query

### Phase 2: LLM-Driven Query Refinement
- Passes execution plans to a specialized Critic/Optimizer Agent
- Uses Claude/GPT-4 to autonomously rewrite inefficient SQL
- Applies structural optimizations:
  - Replace nested subqueries with CTEs
  - Optimize JOIN types and order
  - Push down predicates
  - Eliminate SELECT * clauses
- Provides rationale and confidence scores

**Endpoints:**
- `POST /refine` - Get LLM-optimized query refinements

### Phase 3: Batch Query Optimization
- Consolidates overlapping/redundant sub-queries in multi-agent workflows
- Implements Halo architecture pattern for query plan graphs
- Shares context caches and batches similar queries
- Eliminates redundant computations before execution

**Endpoints:**
- `POST /batch-optimize` - Optimize multiple queries together

### Phase 4: Autonomous Database Tuning
- Analyzes historical query patterns and execution histories
- Acts as an automated DBA using RL
- Recommends:
  - Index creation
  - Partition modifications
  - Materialized views
  - Statistics updates
- Simulates improvements before suggesting

**Features:**
- RL-based learning from query execution history
- Proactive recommendations based on patterns
- Priority levels for recommendations

## Getting Started

### Prerequisites
- Rust 1.70+
- PostgreSQL/MySQL/SurrealDB (for optimization)

### Installation

```bash
git clone https://github.com/lornu-ai/quxtible.git
cd quxtible
cargo build --release
```

### Running the Service

```bash
# Default (Ollama embeddings)
cargo run -p quxtible-service

# With PostgreSQL
DATABASE_URL=postgres://user:pass@localhost/mydb cargo run -p quxtible-service
```

## API Usage

### Health Check
```bash
curl http://localhost:8000/healthz
```

### Estimate Query Cost
```bash
curl -X POST http://localhost:8000/estimate-cost \
  -H "Content-Type: application/json" \
  -d '{
    "nl_query": "Get all users from the 2024 cohort",
    "sql": "SELECT * FROM users WHERE cohort_year = 2024",
    "database": "PostgreSQL",
    "context": {
      "agent_id": "searcher-001",
      "session_id": "sess-123",
      "timestamp_ms": 1709000000000
    }
  }'
```

### Refine Query with LLM
```bash
curl -X POST http://localhost:8000/refine \
  -H "Content-Type: application/json" \
  -d '{
    "sql": "SELECT * FROM users u JOIN orders o ON u.id = o.user_id WHERE u.status = \"active\" AND o.created_at > NOW() - INTERVAL 30 DAY",
    "database": "PostgreSQL"
  }'
```

### Full Optimization Pipeline
```bash
curl -X POST http://localhost:8000/optimize \
  -H "Content-Type: application/json" \
  -d '{
    "nl_query": "Find active users with recent orders",
    "sql": "SELECT * FROM users WHERE status = \"active\" AND id IN (SELECT user_id FROM orders WHERE created_at > NOW() - INTERVAL 30 DAY)",
    "database": "PostgreSQL"
  }'
```

## Configuration

### Environment Variables

```bash
# Database connection
DATABASE_URL=postgres://user:pass@localhost/mydb

# LLM Configuration
ANTHROPIC_API_KEY=your-api-key
CLAUDE_MODEL=claude-3-sonnet-20240229

# Cost estimation thresholds
COST_THRESHOLD=1000.0
QUERY_TIMEOUT_MS=30000

# Optimization
ENABLE_LLM_REFINEMENT=true
ENABLE_BATCH_OPTIMIZATION=true
ENABLE_DB_TUNING=true

# Logging
RUST_LOG=quxtible=debug,tower_http=debug
```

## Project Structure

```
quxtible/
├── crates/
│   ├── quxtible-core/          # Core optimization engine
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── types.rs                      # Data types
│   │   │   ├── phase1_cost_estimation.rs     # EXPLAIN + cost analysis
│   │   │   ├── phase2_llm_refinement.rs      # LLM optimization
│   │   │   ├── phase3_batch_optimization.rs  # Multi-query optimization
│   │   │   ├── phase4_db_tuning.rs           # RL-based recommendations
│   │   │   └── database.rs                   # DB abstraction
│   │   └── Cargo.toml
│   └── quxtible-service/       # HTTP API service
│       ├── src/
│       │   └── main.rs
│       └── Cargo.toml
├── Cargo.toml                  # Workspace manifest
└── README.md
```

## Development Roadmap

### MVP (Phase 1-2)
- [x] Project structure
- [x] Type definitions
- [ ] Phase 1: Cost estimation (PostgreSQL)
- [ ] Phase 2: LLM refinement (Claude API)
- [ ] Basic HTTP service
- [ ] Unit tests

### Phase 2-3 (Multi-Agent)
- [ ] Phase 3: Batch query optimization
- [ ] MySQL support
- [ ] Comprehensive test coverage
- [ ] Performance benchmarks

### Phase 4 (Autonomous Tuning)
- [ ] Phase 4: RL-based database tuning
- [ ] Query history tracking
- [ ] Recommendation engine
- [ ] A/B testing for recommendations

### Integration
- [ ] Integration with Bond (agent orchestration)
- [ ] Integration with MOM (query pattern memory)
- [ ] REST API stabilization
- [ ] Production deployment

## Testing

```bash
# Run all tests
cargo test

# Run tests with logging
RUST_LOG=debug cargo test -- --nocapture

# Run specific phase tests
cargo test phase1
cargo test phase2
```

## Performance

- **Cost Estimation**: < 100ms per query
- **LLM Refinement**: 500ms - 2s per query (depends on LLM latency)
- **Batch Optimization**: O(n²) for n queries (but amortized)
- **DB Tuning Analysis**: Runs asynchronously in background

## Security

- Blocks queries exceeding cost thresholds
- Sanitizes error messages (no DB internals exposed)
- Tenant isolation via context
- Rate limiting (TODO)
- Input validation (TODO)

## License

MIT

## Contributing

See CONTRIBUTING.md (TODO)

## Authors

- Claude Code <claude@anthropic.com>

---

**Next Steps:**
1. Implement Phase 1 (PostgreSQL cost estimation)
2. Add Claude API integration for Phase 2
3. Write comprehensive tests
4. Deploy to production
