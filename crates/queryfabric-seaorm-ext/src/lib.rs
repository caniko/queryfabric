use std::sync::Arc;

pub use sea_orm::FromJsonQueryResult;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, DbErr, ExecResult, QueryResult, Statement,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Cheaply cloneable wrapper around a SeaORM connection pool handle.
#[derive(Clone, Debug)]
pub struct SharedDatabaseConnection(Arc<DatabaseConnection>);

impl SharedDatabaseConnection {
    pub fn new(connection: DatabaseConnection) -> Self {
        Self(Arc::new(connection))
    }
}

impl std::ops::Deref for SharedDatabaseConnection {
    type Target = DatabaseConnection;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl AsRef<DatabaseConnection> for SharedDatabaseConnection {
    fn as_ref(&self) -> &DatabaseConnection {
        self.0.as_ref()
    }
}

#[async_trait::async_trait]
impl ConnectionTrait for SharedDatabaseConnection {
    fn get_database_backend(&self) -> DbBackend {
        self.0.get_database_backend()
    }
    async fn execute(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        self.0.execute(stmt).await
    }
    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        self.0.execute_unprepared(sql).await
    }
    async fn query_one(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        self.0.query_one(stmt).await
    }
    async fn query_all(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        self.0.query_all(stmt).await
    }
}

/// A `Vec<i16>` stored as JSONB in the database.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct I16Vec(pub Vec<i16>);

/// A `Vec<Uuid>` stored as JSONB in the database.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct UuidVec(pub Vec<Uuid>);

impl std::ops::Deref for I16Vec {
    type Target = Vec<i16>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for I16Vec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::ops::Deref for UuidVec {
    type Target = Vec<Uuid>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for UuidVec {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<i16>> for I16Vec {
    fn from(v: Vec<i16>) -> Self {
        Self(v)
    }
}

impl From<I16Vec> for Vec<i16> {
    fn from(v: I16Vec) -> Self {
        v.0
    }
}

impl From<Vec<Uuid>> for UuidVec {
    fn from(v: Vec<Uuid>) -> Self {
        Self(v)
    }
}

impl From<UuidVec> for Vec<Uuid> {
    fn from(v: UuidVec) -> Self {
        v.0
    }
}
