//! Access-control tiers and GDPR data-rights operations over generic resources.
//!
//! Implements the three-tier access decision (`Open | Registered | Restricted`)
//! from [`queryfabric_contract::AccessPolicy`] with a **deny-by-default**
//! restricted tier: access requires ownership, group membership, or an
//! accepted data-use agreement, all looked up through the host-implemented
//! [`OwnershipSource`] trait (no user table is baked in — `queryfabric-tenancy`
//! ships an in-memory implementation, hosts bring their own).
//!
//! The GDPR data-rights surface ([`DataRights`]) expresses Articles 15/16/17
//! against a generic [`queryfabric_contract::ResourceRef`]:
//!
//! - **access** ([`DataRights::access_export`]) returns the resource's policy
//!   plus its full provenance history as a structured record;
//! - **rectification** ([`DataRights::rectify`]) records
//!   `Activity::Modified { field }`;
//! - **erasure** ([`DataRights::soft_delete`]) is soft-delete-with-reason plus
//!   audit, recording `Activity::Deleted { reason }`. It is deliberately *not*
//!   a physical purge: the provenance trail must survive erasure so the
//!   erasure itself stays auditable.
//!
//! Every operation appends the corresponding activity to an injected
//! [`queryfabric_provenance::ProvenanceStore`].

mod decision;
mod license;
mod policy;
mod rights;

pub use decision::{
    GroupId, OwnershipSnapshot, OwnershipSource, SnapshotAccessDecision, evaluate_access,
    evaluate_with_snapshot, snapshot_for,
};
pub use license::DataLicense;
pub use policy::{DataUseRestriction, ResourcePolicy};
pub use rights::{AccessExportRecord, DataRights, RectifyReceipt, SoftDeletion};

#[cfg(test)]
mod tests;
