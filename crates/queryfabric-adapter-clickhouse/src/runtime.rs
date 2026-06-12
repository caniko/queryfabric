//! Reference [`ExecutionRuntime`] adapter for ClickHouse.
//!
//! Interactive mode only: emits ClickHouse SQL from the bound query and
//! streams Arrow record batches from a host-supplied transport. The transport
//! abstracts the wire client (HTTP, native protocol, a test mock) so this
//! crate stays free of any networking dependency.

use std::sync::Arc;

use arrow::record_batch::RecordBatch;
use async_trait::async_trait;
use futures::StreamExt;
use futures::future::{Either, select};
use futures::stream::{self, BoxStream};
use queryfabric_catalog::{BackendAdapter, Catalog, SqlArtifact};
use queryfabric_ir::BoundQuery;
use queryfabric_runtime::{
    ExecutionRuntime, ExecutionRuntimeMode, RecordBatchStream, RuntimeError,
};
use tokio_util::sync::CancellationToken;

use crate::ClickHouseAdapter;

/// Transport that executes a ClickHouse SQL statement and streams Arrow
/// record batches back. Errors are reported as human-readable strings; the
/// runtime wraps them in [`RuntimeError::Adapter`].
pub trait ClickHouseArrowTransport: Send + Sync {
    fn query_arrow_stream(&self, sql: &str) -> BoxStream<'static, Result<RecordBatch, String>>;
}

type SqlTransform = dyn Fn(&SqlArtifact) -> String + Send + Sync;

/// Interactive [`ExecutionRuntime`] backed by a [`ClickHouseArrowTransport`].
pub struct ClickHouseRuntime {
    transport: Arc<dyn ClickHouseArrowTransport>,
    catalog: Arc<dyn Catalog>,
    sql_transform: Option<Box<SqlTransform>>,
}

impl ClickHouseRuntime {
    pub fn new(transport: Arc<dyn ClickHouseArrowTransport>, catalog: Arc<dyn Catalog>) -> Self {
        Self {
            transport,
            catalog,
            sql_transform: None,
        }
    }

    /// Install a host hook that rewrites the emitted SQL artifact before
    /// execution (e.g. casting columns for Arrow-safe output).
    #[must_use]
    pub fn with_sql_transform(
        mut self,
        transform: impl Fn(&SqlArtifact) -> String + Send + Sync + 'static,
    ) -> Self {
        self.sql_transform = Some(Box::new(transform));
        self
    }
}

#[async_trait]
impl ExecutionRuntime for ClickHouseRuntime {
    async fn execute(
        &self,
        plan: BoundQuery,
        mode: ExecutionRuntimeMode,
        cancel: CancellationToken,
    ) -> Result<RecordBatchStream, RuntimeError> {
        if !matches!(mode, ExecutionRuntimeMode::Interactive) {
            return Err(RuntimeError::NotImplemented);
        }
        if cancel.is_cancelled() {
            return Err(RuntimeError::Cancelled);
        }

        let artifact = BackendAdapter::emit(&ClickHouseAdapter, &plan, self.catalog.as_ref())
            .map_err(|error| {
                RuntimeError::Adapter(format!(
                    "emit ClickHouse SQL artifact from bound query plan: {error}"
                ))
            })?
            .as_sql()
            .cloned()
            .ok_or_else(|| {
                RuntimeError::Adapter(
                    "bound query plan did not produce a ClickHouse SQL artifact; use a ClickHouse-compatible backend adapter"
                        .to_owned(),
                )
            })?;
        let sql = match &self.sql_transform {
            Some(transform) => transform(&artifact),
            None => artifact.text.clone(),
        };

        let batches = self.transport.query_arrow_stream(&sql);
        Ok(Box::pin(stream::unfold(
            (batches, cancel),
            |(mut batches, cancel)| async move {
                let cancelled = Box::pin(cancel.clone().cancelled_owned());
                match select(cancelled, batches.next()).await {
                    Either::Left(((), _)) => {
                        Some((Err(RuntimeError::Cancelled), (batches, cancel)))
                    }
                    Either::Right((next, _)) => next.map(|result| {
                        (
                            result.map_err(|error| {
                                RuntimeError::Adapter(format!(
                                    "stream ClickHouse Arrow batches for emitted SQL: {error}"
                                ))
                            }),
                            (batches, cancel),
                        )
                    }),
                }
            },
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use arrow::array::Int64Array;
    use arrow::datatypes::{DataType as ArrowDataType, Field, Schema};
    use futures::TryStreamExt;
    use queryfabric_catalog::{
        ColumnSchema, MemoryCatalog, RelationKind, RelationSchema, bind_and_validate,
    };
    use queryfabric_dialect_sql::GenericSqlDialect;
    use queryfabric_ir::{DataType, Dialect, QueryParameters};

    use super::*;

    struct MockTransport {
        batches: Vec<RecordBatch>,
        seen_sql: Mutex<Option<String>>,
    }

    impl ClickHouseArrowTransport for MockTransport {
        fn query_arrow_stream(&self, sql: &str) -> BoxStream<'static, Result<RecordBatch, String>> {
            *self.seen_sql.lock().expect("sql mutex") = Some(sql.to_owned());
            stream::iter(self.batches.clone().into_iter().map(Ok)).boxed()
        }
    }

    fn sample_batch() -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![Field::new(
            "x",
            ArrowDataType::Int64,
            false,
        )]));
        RecordBatch::try_new(schema, vec![Arc::new(Int64Array::from(vec![1_i64, 2, 3]))])
            .expect("record batch")
    }

    fn bound_query(catalog: &MemoryCatalog) -> BoundQuery {
        let parsed = GenericSqlDialect
            .parse("SELECT x FROM samples")
            .expect("parse");
        bind_and_validate(&parsed, catalog, &QueryParameters::default()).expect("bind")
    }

    fn samples_catalog() -> MemoryCatalog {
        let mut catalog = MemoryCatalog::default();
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: "samples".into(),
            aliases: Vec::new(),
            kind: RelationKind::Table,
            columns: vec![ColumnSchema {
                name: "x".into(),
                data_type: DataType::Int64,
                nullable: false,
                metadata: Default::default(),
            }],
            metadata: Default::default(),
        });
        catalog
    }

    #[test]
    fn interactive_execution_streams_batches_from_transport() {
        let catalog = Arc::new(samples_catalog());
        let transport = Arc::new(MockTransport {
            batches: vec![sample_batch(), sample_batch()],
            seen_sql: Mutex::new(None),
        });
        let runtime = ClickHouseRuntime::new(transport.clone(), catalog.clone());
        let query = bound_query(&catalog);

        let stream = futures::executor::block_on(runtime.execute(
            query,
            ExecutionRuntimeMode::Interactive,
            CancellationToken::new(),
        ))
        .expect("interactive stream");
        let batches: Vec<RecordBatch> =
            futures::executor::block_on(stream.try_collect()).expect("collect batches");

        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].num_rows(), 3);
        let seen = transport
            .seen_sql
            .lock()
            .expect("sql mutex")
            .clone()
            .expect("transport saw SQL");
        assert_eq!(seen, "SELECT samples.x FROM samples");
    }

    #[test]
    fn sql_transform_hook_rewrites_emitted_sql() {
        let catalog = Arc::new(samples_catalog());
        let transport = Arc::new(MockTransport {
            batches: vec![sample_batch()],
            seen_sql: Mutex::new(None),
        });
        let runtime = ClickHouseRuntime::new(transport.clone(), catalog.clone())
            .with_sql_transform(|artifact| {
                format!("{} SETTINGS output_format=arrow", artifact.text)
            });
        let query = bound_query(&catalog);

        let stream = futures::executor::block_on(runtime.execute(
            query,
            ExecutionRuntimeMode::Interactive,
            CancellationToken::new(),
        ))
        .expect("interactive stream");
        let _batches: Vec<RecordBatch> =
            futures::executor::block_on(stream.try_collect()).expect("collect batches");

        let seen = transport
            .seen_sql
            .lock()
            .expect("sql mutex")
            .clone()
            .expect("transport saw SQL");
        assert!(seen.ends_with("SETTINGS output_format=arrow"));
    }

    #[test]
    fn non_interactive_modes_and_pre_cancellation_short_circuit() {
        let catalog = Arc::new(samples_catalog());
        let transport = Arc::new(MockTransport {
            batches: Vec::new(),
            seen_sql: Mutex::new(None),
        });
        let runtime = ClickHouseRuntime::new(transport, catalog.clone());
        let query = bound_query(&catalog);

        let batch_mode = futures::executor::block_on(runtime.execute(
            query.clone(),
            ExecutionRuntimeMode::Batch,
            CancellationToken::new(),
        ));
        assert!(matches!(batch_mode, Err(RuntimeError::NotImplemented)));

        let cancelled_token = CancellationToken::new();
        cancelled_token.cancel();
        let cancelled = futures::executor::block_on(runtime.execute(
            query,
            ExecutionRuntimeMode::Interactive,
            cancelled_token,
        ));
        assert!(matches!(cancelled, Err(RuntimeError::Cancelled)));
    }
}
