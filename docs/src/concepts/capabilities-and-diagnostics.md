# Capabilities and Diagnostics

QueryFabric does not treat portability as "the parser accepted it."

Instead, binding computes the semantic requirements of a query and backend
analysis decides whether an adapter can satisfy them.

## Capability Requirements

Bound queries carry the requirements they need from an adapter. Examples:

- join families
- window functions
- aggregate behavior
- function namespaces
- type coercion rules

That lets a host ask several adapters the same question before it chooses where
to run the query.

## Backend Diagnostics

`BackendAnalysis` contains structured diagnostics rather than plain strings.

Those diagnostics are meant to be machine-usable and user-usable at the same
time:

- stable code
- severity
- message
- optional remediation hint
- backend context

In practice, this is the difference between "backend unsupported" and a useful
answer such as "PostgreSQL rejects this query because it requires a ClickHouse
namespaced function."

## Supported Does Not Mean Executed

`supported = true` only means the adapter can faithfully emit an artifact for
the query. QueryFabric still does not own execution, authentication, routing,
or job orchestration.

## Host Pattern

The intended host flow is:

1. bind once
2. analyze against each candidate backend
3. apply host routing policy
4. emit only for the chosen backend

That keeps backend choice explicit, testable, and explainable.
