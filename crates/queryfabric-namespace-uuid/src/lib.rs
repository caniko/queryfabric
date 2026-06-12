//! Typed UUIDv5 namespace derivation.
//!
//! Define a marker type, give it a `const NAMESPACE: Uuid`, and get
//! deterministic UUIDv5 helpers for free. All methods inline and
//! monomorphise per-type so there's no dispatch overhead.
//!
//! ```ignore
//! use queryfabric_namespace_uuid::NamespacedIds;
//! use uuid::Uuid;
//!
//! pub struct Tenant;
//! impl NamespacedIds for Tenant {
//!     const NAMESPACE: Uuid = Uuid::from_bytes(*b"tenant-app-ns-v1");
//! }
//!
//! let id = Tenant::from_u64(42);
//! ```

#![warn(missing_docs)]

use std::io::Write as _;

use uuid::Uuid;

/// Trait implemented by marker types to scope UUIDv5 generation.
///
/// Implementors set `const NAMESPACE` to a fixed 16-byte UUID; default
/// methods derive deterministic IDs from common key shapes.
pub trait NamespacedIds {
    /// Per-namespace UUIDv5 root (exactly 16 bytes).
    const NAMESPACE: Uuid;

    /// Deterministic UUIDv5 from a `u64` key.
    #[inline]
    #[must_use]
    fn from_u64(key: u64) -> Uuid {
        Uuid::new_v5(&Self::NAMESPACE, key.to_string().as_bytes())
    }

    /// Deterministic UUIDv5 from a string key.
    #[inline]
    #[must_use]
    fn from_str_key(key: &str) -> Uuid {
        Uuid::new_v5(&Self::NAMESPACE, key.as_bytes())
    }

    /// Deterministic UUIDv5 from composite parts joined by `:`.
    #[inline]
    #[must_use]
    fn from_parts(parts: &[&str]) -> Uuid {
        joined_uuid(&Self::NAMESPACE, parts)
    }

    /// Deterministic UUIDv5 keyed by two ids and three coordinates.
    ///
    /// Stack-allocated buffer — no heap allocation per call.
    #[inline]
    #[must_use]
    fn from_coords(pre: u64, post: u64, x: i64, y: i64, z: i64) -> Uuid {
        let mut buf = [0u8; 128];
        let len = {
            let mut cursor = &mut buf[..];
            let _ = write!(cursor, "{pre}:{post}:{x}:{y}:{z}");
            128 - cursor.len()
        };
        Uuid::new_v5(&Self::NAMESPACE, &buf[..len])
    }
}

/// Join `parts` with `:` and hash as UUIDv5.
///
/// Uses a 256-byte stack buffer; falls back to heap if exceeded.
#[inline]
#[must_use]
pub fn joined_uuid(namespace: &Uuid, parts: &[&str]) -> Uuid {
    let mut buf = [0u8; 256];
    let mut pos = 0usize;

    for (i, part) in parts.iter().enumerate() {
        if i > 0 && pos < buf.len() {
            buf[pos] = b':';
            pos += 1;
        }
        let bytes = part.as_bytes();
        let end = pos + bytes.len();
        if end <= buf.len() {
            buf[pos..end].copy_from_slice(bytes);
            pos = end;
        } else {
            let name = parts.join(":");
            return Uuid::new_v5(namespace, name.as_bytes());
        }
    }

    Uuid::new_v5(namespace, &buf[..pos])
}

/// Generate a marker struct with a [`NamespacedIds`] implementation.
///
/// ```ignore
/// queryfabric_namespace_uuid::namespaced_ids!(Tenant, b"tenant-app-ns-v1");
/// ```
#[macro_export]
macro_rules! namespaced_ids {
    ($name:ident, $namespace:expr) => {
        /// Marker type for namespace-scoped UUID generation.
        pub struct $name;

        impl $crate::NamespacedIds for $name {
            const NAMESPACE: ::uuid::Uuid = ::uuid::Uuid::from_bytes(*$namespace);
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Test;
    impl NamespacedIds for Test {
        const NAMESPACE: Uuid = Uuid::from_bytes(*b"test-ns-uuid-v1!");
    }

    #[test]
    fn deterministic_and_distinct() {
        assert_eq!(Test::from_u64(42), Test::from_u64(42));
        assert_ne!(Test::from_u64(1), Test::from_u64(2));
        assert_eq!(Test::from_str_key("AVAL"), Test::from_str_key("AVAL"));
        assert_eq!(
            Test::from_parts(&["100", "200", "AL"]),
            Test::from_parts(&["100", "200", "AL"]),
        );
        assert_ne!(Test::from_parts(&["1", "2"]), Test::from_parts(&["2", "1"]),);
        assert_eq!(
            Test::from_coords(1, 2, 10, 20, 30),
            Test::from_coords(1, 2, 10, 20, 30),
        );
        assert_ne!(
            Test::from_coords(1, 2, 10, 20, 30),
            Test::from_coords(1, 2, 11, 20, 30),
        );
    }

    #[test]
    fn version_is_v5() {
        assert_eq!(Test::from_u64(42).get_version_num(), 5);
    }

    #[test]
    fn different_namespaces_differ() {
        struct Other;
        impl NamespacedIds for Other {
            const NAMESPACE: Uuid = Uuid::from_bytes(*b"other-ns-uuid-v1");
        }
        assert_ne!(Test::from_u64(42), Other::from_u64(42));
    }

    #[test]
    fn joined_uuid_empty_parts_hashes_empty_name() {
        assert_eq!(joined_uuid(&Test::NAMESPACE, &[]), Test::from_str_key(""));
    }

    #[test]
    fn joined_uuid_matches_colon_joined_parts() {
        assert_eq!(
            joined_uuid(&Test::NAMESPACE, &["pre", "post", "12"]),
            Test::from_str_key("pre:post:12")
        );
        assert_ne!(
            joined_uuid(&Test::NAMESPACE, &["pre", "post", "12"]),
            Test::from_str_key("prepost12")
        );
    }

    #[test]
    fn joined_uuid_heap_fallback_matches_regular_join() {
        let long = "x".repeat(300);
        let parts = ["prefix", long.as_str(), "suffix"];
        assert_eq!(
            joined_uuid(&Test::NAMESPACE, &parts),
            Test::from_str_key(&parts.join(":"))
        );
    }

    #[test]
    fn from_coords_includes_sign_and_order() {
        assert_ne!(
            Test::from_coords(1, 2, -3, 4, 5),
            Test::from_coords(1, 2, 3, 4, 5)
        );
        assert_ne!(
            Test::from_coords(1, 2, -3, 4, 5),
            Test::from_coords(2, 1, -3, 4, 5)
        );
    }
}
