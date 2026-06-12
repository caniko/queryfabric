# QueryFabric Host Integration

A host application can use QueryFabric for SyQL parsing and portable query
compilation without defining QueryFabric's public scope.

The host remains responsible for:

- metadata resolution against PostgreSQL and host-specific catalogs
- backend routing policy
- auth and access control
- async job submission
- federation execution
- metadata-driven relation routing and execution policy

ClickHouse adapter-specific materialized-view wrapping and advisories are part
of QueryFabric; the host still decides when and why a given relation should be
queried.

The intended integration shape is:

1. parse SyQL through `queryfabric-dialect-syql`
2. bind through a host-owned `Catalog` implementation
3. analyze candidate backends
4. choose backend in host policy
5. execute the emitted artifact outside QueryFabric
