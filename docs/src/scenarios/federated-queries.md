# Scenario: Federate Queries Across Nodes

**Who this is for:** You run multiple QueryFabric instances (e.g. one per
research group) and want to run queries that span them — scatter a query to
every node and gather the partial results.

**What you'll end up with:** A hub node that accepts a SyQL query with
`SCOPE federation`, scatters it to member nodes via Arrow Flight, and merges
the partial results.

## How federation works

```
User query ──► Hub node
                   │
          ┌────────┼────────┐
          ▼        ▼        ▼
       Node A   Node B   Node C
          │        │        │
          └────────┼────────┘
                   ▼
             Merged result
```

The hub decomposes aggregate queries:

| Aggregate | Scatter (per node) | Gather (hub) |
|-----------|-------------------|---------------|
| `SUM(x)` | `SUM(x)` | `SUM(partial)` |
| `COUNT(*)` | `COUNT(*)` | `SUM(partial)` |
| `AVG(x)` | `SUM(x), COUNT(x)` | `SUM(sums) / SUM(counts)` |
| `MIN(x)` | `MIN(x)` | `MIN(partial)` |
| `MAX(x)` | `MAX(x)` | `MAX(partial)` |

## Step 1: Configure nodes

Each node runs the standard QueryFabric service with federation enabled:

```nix
services.queryfabric = {
  enable = true;
  federation = {
    enable = true;
    nodeName = "node-a";
    listenAddr = "/ip4/0.0.0.0/tcp/4001";
    hubEndpoint = "https://hub.queryfabric.internal:50053";
  };
};
```

## Step 2: Register nodes with the hub

On the hub, register each node:

```bash
curl -X POST https://hub.queryfabric.internal/api/v1/federation/nodes \
  -H "Content-Type: application/json" \
  -d '{
    "name": "node-a",
    "flight_endpoint": "node-a.queryfabric.internal:50053",
    "public_key": "..."
  }'
```

## Step 3: Run a federated query

```syql
SCOPE federation
FROM measurements
WHERE value > 10
GROUP BY sample_type
SELECT sample_type, avg(value) AS mean
```

The hub:

1. Detects `SCOPE federation`
2. Validates the query is decomposable (no CTEs, no JOINs, no subqueries)
3. Builds a scatter-gather plan: each node runs
   `SELECT sample_type, sum(value), count(value) FROM measurements GROUP BY sample_type`
4. Sends the scatter SQL to each registered node via Arrow Flight DoGet
5. Collects partial results
6. Runs the gather: `SELECT sample_type, sum(sums) / sum(counts) AS mean FROM ({partials}) GROUP BY sample_type`
7. Streams the merged result to the user

## Limitations

Federation supports a subset of SQL:

- ✅ `SELECT`, `WHERE`, `GROUP BY`, `HAVING`
- ✅ `SUM`, `COUNT`, `AVG`, `MIN`, `MAX`
- ✅ `ORDER BY`, `LIMIT`, `OFFSET`
- ❌ `JOIN`, CTEs (`WITH`), subqueries in `FROM`
- ❌ `DISTINCT`, window functions
- ❌ `SETTINGS`, `FORMAT`

A federated query that uses an unsupported feature produces a clear diagnostic
before execution.
