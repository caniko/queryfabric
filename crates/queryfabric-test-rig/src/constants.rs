#![allow(missing_docs)]
//! Default image tags and credentials for test infrastructure.

/// PostgreSQL image tag.
pub const POSTGRES_IMAGE: &str = "postgres:16-alpine";
/// ClickHouse server image tag.
pub const CLICKHOUSE_IMAGE: &str = "clickhouse/clickhouse-server:24.12-alpine";
/// MinIO image tag.
pub const MINIO_IMAGE: &str = "minio/minio:RELEASE.2025-02-28T00-00-00Z";

/// Default PostgreSQL database name for tests.
pub const POSTGRES_DB: &str = "testdb";
/// Default PostgreSQL user for tests.
pub const POSTGRES_USER: &str = "testuser";
/// Default PostgreSQL password for tests.
pub const POSTGRES_PASSWORD: &str = "testpass";

/// Default MinIO root user.
pub const MINIO_ROOT_USER: &str = "minioadmin";
/// Default MinIO root password.
pub const MINIO_ROOT_PASSWORD: &str = "minioadmin";
