# SynDB Host Integration

SynDB is the reference host application for the SyQL dialect, but it is not
the definition of QueryFabric's public scope.

SynDB remains responsible for:

- metadata resolution against PostgreSQL and SynDB-specific catalogs
- backend routing policy
- auth and access control
- async job submission
- federation execution
- metadata-driven relation routing and execution policy

ClickHouse adapter-specific materialized-view wrapping and advisories are part
of QueryFabric now; SynDB still decides when and why a given relation should be
queried.

The intended integration shape is:

1. parse SyQL through `queryfabric-dialect-syql`
2. bind through a SynDB-owned `Catalog` implementation
3. analyze candidate backends
4. choose backend in host policy
5. execute the emitted artifact outside QueryFabric
