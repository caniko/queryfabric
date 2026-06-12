//! Generic multi-tenant ownership model: accounts, collections, groups.
//!
//! Holds the identity shapes a multi-tenant data platform needs — [`Account`]
//! (human or service), [`Collection`], [`Group`] with membership — plus
//! [`InMemoryOwnership`], an implementation of
//! [`queryfabric_access::OwnershipSource`] that backs access decisions in
//! tests and the demonstrator host. Production hosts implement
//! `OwnershipSource` over their own identity store.
//!
//! Deliberately absent: password hashing, OAuth linkage, profile fields, and
//! uniqueness enforcement (email is a plain field — uniqueness is the host
//! database's job).

mod model;
mod ownership;

pub use model::{Account, AccountKind, Collection, Group};
pub use ownership::InMemoryOwnership;

#[cfg(test)]
mod tests;
