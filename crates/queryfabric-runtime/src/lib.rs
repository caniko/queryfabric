//! Runtime dispatch traits for QueryFabric execution backends.

mod driver;
mod error;
#[cfg(feature = "flight")]
pub mod flight;
mod mode;
mod runtime;
pub mod util;

pub use driver::{
    IsolatedExecutionDriver, IsolatedJobSpec, ObjectStoreFormat, ResourceRequest, StorageAccessMode,
};
pub use error::{DriverError, RuntimeError};
pub use mode::{ExecutionRuntimeMode, resolve_runtime_mode, runtime_mode_for_estimate};
pub use runtime::{ExecutionRuntime, RecordBatchStream};

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use queryfabric_catalog::{
        ColumnSchema, MemoryCatalog, RelationKind, RelationSchema, ResultDeliveryMode,
        bind_and_validate,
    };
    use queryfabric_dialect_sql::GenericSqlDialect;
    use queryfabric_ir::{DataType, Dialect, QueryParameters};
    use tokio_util::sync::CancellationToken;

    use super::{
        ExecutionRuntimeMode, IsolatedExecutionDriver, IsolatedJobSpec, ObjectStoreFormat,
        ResourceRequest, StorageAccessMode,
    };

    fn assert_driver_object(_: Box<dyn IsolatedExecutionDriver>) {}

    #[test]
    fn result_delivery_modes_map_to_runtime_modes() {
        assert_eq!(
            ExecutionRuntimeMode::from(ResultDeliveryMode::InteractiveStream),
            ExecutionRuntimeMode::Interactive
        );
        assert_eq!(
            ExecutionRuntimeMode::from(ResultDeliveryMode::PagedResult),
            ExecutionRuntimeMode::Batch
        );
        assert_eq!(
            ExecutionRuntimeMode::from(ResultDeliveryMode::AsyncMaterializedExport),
            ExecutionRuntimeMode::Isolated
        );
        assert_eq!(
            ExecutionRuntimeMode::from(ResultDeliveryMode::RejectedOverBudget),
            ExecutionRuntimeMode::Interactive
        );
    }

    #[test]
    fn runtime_enums_and_specs_serde_roundtrip() {
        let mode = ExecutionRuntimeMode::Isolated;
        let json = serde_json::to_string(&mode).expect("mode serialize");
        assert_eq!(
            serde_json::from_str::<ExecutionRuntimeMode>(&json).expect("mode deserialize"),
            mode
        );

        let storage_modes = [
            StorageAccessMode::ReplicatedReadOnly,
            StorageAccessMode::SnapshotClone {
                source_pvc: "clickhouse-data-0".into(),
                snapshot_class: "csi-snap".into(),
            },
            StorageAccessMode::ObjectStore {
                uri: "s3://queryfabric-search/query.parquet".into(),
                format: ObjectStoreFormat::Parquet,
            },
        ];
        for storage in storage_modes {
            let json = serde_json::to_string(&storage).expect("storage serialize");
            assert_eq!(
                serde_json::from_str::<StorageAccessMode>(&json).expect("storage deserialize"),
                storage
            );
        }

        let resources = ResourceRequest {
            cpu_request: "8000m".into(),
            memory_request: "32Gi".into(),
            cpu_limit: "8".into(),
            memory_limit: "48Gi".into(),
        };
        let json = serde_json::to_string(&resources).expect("resources serialize");
        assert_eq!(
            serde_json::from_str::<ResourceRequest>(&json).expect("resources deserialize"),
            resources
        );

        let mut catalog = MemoryCatalog::default();
        catalog.register_relation(RelationSchema {
            namespace: None,
            name: "records".into(),
            aliases: Vec::new(),
            kind: RelationKind::Table,
            columns: vec![ColumnSchema {
                name: "record_id".into(),
                data_type: DataType::Uuid,
                nullable: false,
                metadata: Default::default(),
            }],
            metadata: Default::default(),
        });
        let parsed = GenericSqlDialect
            .parse("SELECT record_id FROM records")
            .expect("parse");
        let query =
            bind_and_validate(&parsed, &catalog, &QueryParameters::default()).expect("bind query");
        let spec = IsolatedJobSpec {
            query,
            storage: StorageAccessMode::ReplicatedReadOnly,
            resources,
            timeout: Duration::from_secs(300),
        };
        let json = serde_json::to_string(&spec).expect("spec serialize");
        assert_eq!(
            serde_json::from_str::<IsolatedJobSpec>(&json).expect("spec deserialize"),
            spec
        );
    }

    #[test]
    fn runtime_traits_are_object_safe_send_sync() {
        assert_driver_object(Box::new(StubDriver));
    }

    struct StubDriver;

    #[async_trait::async_trait]
    impl IsolatedExecutionDriver for StubDriver {
        async fn spawn(
            &self,
            _: IsolatedJobSpec,
            _: CancellationToken,
        ) -> Result<super::RecordBatchStream, super::DriverError> {
            Err(super::DriverError::Spawn("stub".into()))
        }
    }
}
