//! Append-only provenance activity log over generic resources.
//!
//! Records **who** did **what** to a resource and **when**, enabling audit
//! trails and FAIR R1.2 compliance (detailed provenance) for any host domain.
//!
//! # Core vs domain activities
//!
//! The universal verbs live in [`queryfabric_contract::Activity`]: `Created`,
//! `Deleted`, `Restored`, `Accessed`, `Modified`, `OwnershipTransferred`,
//! `ContentHashRecorded`, `FederationFlow`, `BackupAnchor`. Everything
//! domain-specific (a host's analysis jobs, upload shapes, query executions)
//! is carried as a [`RecordedActivity::Domain`] entry: a stable `kind` tag
//! plus an opaque serialized payload supplied through the host's
//! [`queryfabric_contract::DomainActivity`] impl. This crate never inspects
//! a domain payload beyond its tag.
//!
//! Storage is abstracted behind [`ProvenanceStore`]; [`VecProvenanceStore`]
//! is the in-memory reference implementation. Hosts persist entries however
//! they like (e.g. a ClickHouse event table) as long as the append-only and
//! ordering semantics hold.

mod entry;
mod store;

pub use entry::{ProvenanceEntry, ProvenanceHistory, RecordedActivity};
pub use store::{HistoryFilter, ProvenanceError, ProvenanceStore, VecProvenanceStore};

#[cfg(test)]
mod tests;
