use std::sync::atomic::{AtomicI32, Ordering};

use piying::Reply;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::host::FederationHost;

/// One versioned schema migration. The DDL body is an **opaque string** —
/// the protocol is generic, the SQL dialect is the host's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaMigration {
    /// Monotonically increasing migration version.
    pub version: i32,
    /// Human-readable migration name.
    pub name: String,
    /// Opaque DDL body applied by the host.
    pub sql: String,
}

/// Schema sync request (hub → cluster).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaSync {
    /// Version the cluster should reach after applying `migrations`.
    pub target_version: i32,
    /// Ordered migrations, including ones the cluster may already have.
    pub migrations: Vec<SchemaMigration>,
}

/// Schema sync reply (cluster → hub).
#[derive(Debug, Clone, Serialize, Deserialize, Reply)]
pub struct SchemaSyncReply {
    /// Whether every pending migration applied cleanly.
    pub success: bool,
    /// Version the cluster ended on.
    pub applied_version: i32,
    /// Errors for migrations that failed or were rejected.
    pub errors: Vec<String>,
}

/// Validate that a migration body is DDL-only (CREATE, ALTER).
///
/// Rejecting DML and destructive statements prevents a compromised hub from
/// executing arbitrary SQL on cluster nodes.
pub fn ddl_allowed(sql: &str) -> bool {
    let upper = sql.trim_start().to_ascii_uppercase();
    upper.starts_with("CREATE ") || upper.starts_with("ALTER ")
}

/// Apply a [`SchemaSync`] through the host's DDL hook with atomic version
/// tracking.
///
/// Skips migrations at or below the current version, validates each pending
/// body with [`ddl_allowed`], applies it via
/// [`FederationHost::apply_ddl`], and stops at the first failure. The
/// version is stored back atomically so concurrent health probes observe a
/// consistent value.
pub async fn apply_schema_sync<H: FederationHost>(
    host: &H,
    version: &AtomicI32,
    sync: &SchemaSync,
) -> SchemaSyncReply {
    let mut errors = Vec::new();
    let mut applied_version = version.load(Ordering::Relaxed);

    for migration in &sync.migrations {
        if migration.version <= applied_version {
            continue;
        }

        if !ddl_allowed(&migration.sql) {
            let err_msg = format!(
                "Migration v{} '{}' rejected: only CREATE/ALTER DDL is allowed",
                migration.version, migration.name
            );
            warn!("{err_msg}");
            errors.push(err_msg);
            break;
        }

        match host.apply_ddl(migration).await {
            Ok(()) => {
                applied_version = migration.version;
                info!(
                    version = migration.version,
                    name = %migration.name,
                    "Applied schema migration"
                );
            }
            Err(e) => {
                let err_msg = format!(
                    "Migration v{} '{}' failed: {e}",
                    migration.version, migration.name
                );
                warn!("{err_msg}");
                errors.push(err_msg);
                break;
            }
        }
    }

    version.store(applied_version, Ordering::Relaxed);

    SchemaSyncReply {
        success: errors.is_empty(),
        applied_version,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ddl_guard_accepts_only_create_and_alter() {
        assert!(ddl_allowed("CREATE TABLE t (id UInt64)"));
        assert!(ddl_allowed("  alter table t add column x String"));
        assert!(!ddl_allowed("DROP TABLE t"));
        assert!(!ddl_allowed("INSERT INTO t VALUES (1)"));
        assert!(!ddl_allowed("SELECT 1"));
    }
}
