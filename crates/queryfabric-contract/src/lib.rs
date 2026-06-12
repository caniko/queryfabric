//! Domain-neutral contract traits between QueryFabric and its host application.
//!
//! This crate is the single seam through which a host injects domain knowledge
//! into QueryFabric: resource identity ([`ResourceRef`], [`NodeId`]), provenance
//! activity ([`Activity`], [`DomainActivity`]), access control ([`Subject`],
//! [`AccessPolicy`], [`AccessDecision`]), cost statistics ([`RelationStats`],
//! [`StatisticsSource`]), and cluster health ([`ClusterProbe`]).
//!
//! The rule, recorded in `DECISIONS.md`: domain types enter QueryFabric only
//! via trait impls in the host. No crate in this workspace names a host domain
//! concept.

mod access;
mod cluster;
mod identity;
mod provenance;
mod stats;

pub use access::{AccessDecision, AccessOutcome, AccessPolicy, Subject};
pub use cluster::{ClusterProbe, Health, ProbeResult};
pub use identity::{NodeId, ResourceRef};
pub use provenance::{Activity, DomainActivity};
pub use stats::{RelationStats, StatisticsSource};
