# ClickHouse and PostgreSQL

QueryFabric `0.1` ships with two production SQL emitters:

- ClickHouse
- PostgreSQL

Both target the verified portable subset. The difference is not that one parser
accepts more syntax; the difference is what each adapter can analyze and emit
faithfully.

## ClickHouse Adapter

The ClickHouse adapter is the primary analytical path today. It can:

- emit ClickHouse SQL for the portable subset
- surface ClickHouse-specific diagnostics and advisories
- attach adapter metadata to emitted SQL artifacts

Hosts can use that metadata to make policy decisions without smuggling
ClickHouse semantics into the neutral IR.

## PostgreSQL Adapter

The PostgreSQL adapter intentionally targets the portable relational subset.

If a query needs backend-specific semantics, the correct behavior is structured
rejection during analysis rather than silent weakening or ad hoc rewriting.

## Shared Host Flow

The simplest multi-backend pattern looks like this:

1. parse once
2. bind once
3. analyze against both adapters
4. pick the backend explicitly
5. emit SQL for the chosen backend

The `multi_backend.rs` example demonstrates that shape directly.

## What Is Not Promised

QueryFabric does not promise full SQL feature parity across both backends.

The public promise is narrower and stronger:

- a documented portable subset
- explicit structured rejections outside that subset
- stable result schemas and provenance when emission succeeds
