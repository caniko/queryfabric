//! The demonstrator's domain: urban air-quality monitoring.
//!
//! Deliberately **not** the domain QueryFabric was extracted from — the
//! demonstrator exists to prove the crates are domain-neutral. Five fixed
//! monitoring stations each carry 72 hourly readings of three pollutants
//! (PM2.5, NO₂, ozone). All identifiers and values are deterministic so
//! seeding is idempotent and bundle hashes are reproducible.

use queryfabric::{
    ColumnSchema, DataType, MemoryCatalog, RelationKind, RelationSchema,
};
use queryfabric_contract::ResourceRef;
use queryfabric_namespace_uuid::NamespacedIds;
use uuid::Uuid;

/// Namespace all demonstrator resources live in.
pub const DEMO_NAMESPACE: Uuid = Uuid::from_bytes(*b"qfdemo-namespace");

/// Catalog snapshot id announced to query clients.
pub const SNAPSHOT_ID: &str = "qfdemo-air-quality-v1";

/// Seed epoch for readings and provenance: 2026-01-01T00:00:00Z.
pub const SEED_EPOCH_MS: i64 = 1_767_225_600_000;

/// Hourly readings seeded per station.
pub const READINGS_PER_STATION: u32 = 72;

/// UUIDv5 namespace for station ids.
pub struct StationIds;
impl NamespacedIds for StationIds {
    const NAMESPACE: Uuid = Uuid::from_bytes(*b"qfdemo-airq-stat");
}

/// UUIDv5 namespace for reading ids.
pub struct ReadingIds;
impl NamespacedIds for ReadingIds {
    const NAMESPACE: Uuid = Uuid::from_bytes(*b"qfdemo-airq-read");
}

/// UUIDv5 namespace for account ids.
pub struct AccountIds;
impl NamespacedIds for AccountIds {
    const NAMESPACE: Uuid = Uuid::from_bytes(*b"qfdemo-airq-acct");
}

/// One monitoring station, with per-pollutant baselines for the synthetic
/// reading series.
pub struct StationSpec {
    pub code: &'static str,
    pub name: &'static str,
    pub city: &'static str,
    pub latitude: f64,
    pub longitude: f64,
    pub pm25_base: f64,
    pub no2_base: f64,
    pub ozone_base: f64,
}

impl StationSpec {
    /// Deterministic station id.
    #[must_use]
    pub fn id(&self) -> Uuid {
        StationIds::from_str_key(self.code)
    }

    /// The station as a generic queryfabric resource.
    #[must_use]
    pub fn resource(&self) -> ResourceRef {
        ResourceRef {
            namespace: DEMO_NAMESPACE,
            id: self.id(),
        }
    }
}

/// The five demonstration stations.
pub const STATIONS: [StationSpec; 5] = [
    StationSpec {
        code: "lis-baixa",
        name: "Baixa Monitoring Station",
        city: "Lisbon",
        latitude: 38.7100,
        longitude: -9.1364,
        pm25_base: 11.0,
        no2_base: 24.0,
        ozone_base: 58.0,
    },
    StationSpec {
        code: "tll-kesklinn",
        name: "Kesklinn Monitoring Station",
        city: "Tallinn",
        latitude: 59.4339,
        longitude: 24.7535,
        pm25_base: 7.0,
        no2_base: 16.0,
        ozone_base: 62.0,
    },
    StationSpec {
        code: "ghe-centrum",
        name: "Centrum Monitoring Station",
        city: "Ghent",
        latitude: 51.0543,
        longitude: 3.7174,
        pm25_base: 12.5,
        no2_base: 27.0,
        ozone_base: 49.0,
    },
    StationSpec {
        code: "rkv-vesturbaer",
        name: "Vesturbær Monitoring Station",
        city: "Reykjavik",
        latitude: 64.1466,
        longitude: -21.9426,
        pm25_base: 4.5,
        no2_base: 9.0,
        ozone_base: 71.0,
    },
    StationSpec {
        code: "lju-center",
        name: "Center Monitoring Station",
        city: "Ljubljana",
        latitude: 46.0569,
        longitude: 14.5058,
        pm25_base: 14.0,
        no2_base: 22.0,
        ozone_base: 55.0,
    },
];

/// Look a station up by UUID or by code.
#[must_use]
pub fn find_station(key: &str) -> Option<&'static StationSpec> {
    if let Ok(id) = key.parse::<Uuid>() {
        return STATIONS.iter().find(|station| station.id() == id);
    }
    STATIONS.iter().find(|station| station.code == key)
}

/// One synthetic reading.
pub struct Reading {
    pub id: Uuid,
    pub hour_offset: u32,
    pub pm25: f64,
    pub no2: f64,
    pub ozone: f64,
}

/// The deterministic reading series for a station.
#[must_use]
pub fn readings_for(station: &StationSpec) -> Vec<Reading> {
    (0..READINGS_PER_STATION)
        .map(|hour| {
            let phase = f64::from(hour) * std::f64::consts::TAU / 24.0;
            Reading {
                id: ReadingIds::from_parts(&[station.code, &hour.to_string()]),
                hour_offset: hour,
                pm25: round2(station.pm25_base + 4.0 * phase.sin()),
                no2: round2(station.no2_base + 8.0 * (phase + 1.0).sin()),
                ozone: round2(station.ozone_base - 12.0 * phase.sin()),
            }
        })
        .collect()
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// The relational catalog the query endpoint binds against.
#[must_use]
pub fn build_catalog() -> MemoryCatalog {
    let mut catalog = MemoryCatalog::default();
    catalog.set_snapshot_id(SNAPSHOT_ID);
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "stations".into(),
        aliases: Vec::new(),
        kind: RelationKind::Table,
        columns: vec![
            column("station_id", DataType::Uuid, false),
            column("code", DataType::Utf8, false),
            column("name", DataType::Utf8, false),
            column("city", DataType::Utf8, false),
            column("latitude", DataType::Float64, false),
            column("longitude", DataType::Float64, false),
        ],
        metadata: Default::default(),
    });
    catalog.register_relation(RelationSchema {
        namespace: None,
        name: "readings".into(),
        aliases: Vec::new(),
        kind: RelationKind::Table,
        columns: vec![
            column("reading_id", DataType::Uuid, false),
            column("station_id", DataType::Uuid, false),
            column("measured_at", DataType::Timestamp { timezone: None }, false),
            column("pm25", DataType::Float64, false),
            column("no2", DataType::Float64, false),
            column("ozone", DataType::Float64, false),
        ],
        metadata: Default::default(),
    });
    catalog
}

fn column(name: &str, data_type: DataType, nullable: bool) -> ColumnSchema {
    ColumnSchema {
        name: name.into(),
        data_type,
        nullable,
        metadata: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use queryfabric::{GenericSqlDialect, PostgresAdapter, QueryCompiler, QueryParameters};

    use super::*;

    #[test]
    fn station_ids_are_deterministic() {
        let first = STATIONS[0].id();
        assert_eq!(first, StationIds::from_str_key(STATIONS[0].code));
        let ids: std::collections::HashSet<Uuid> =
            STATIONS.iter().map(StationSpec::id).collect();
        assert_eq!(ids.len(), STATIONS.len(), "station ids must be unique");
    }

    #[test]
    fn readings_are_deterministic_and_idempotent() {
        let a = readings_for(&STATIONS[0]);
        let b = readings_for(&STATIONS[0]);
        assert_eq!(a.len(), READINGS_PER_STATION as usize);
        assert!(
            a.iter()
                .zip(&b)
                .all(|(x, y)| x.id == y.id && x.pm25 == y.pm25)
        );
    }

    #[test]
    fn find_station_accepts_code_and_uuid() {
        let station = &STATIONS[2];
        assert!(find_station(station.code).is_some());
        assert!(find_station(&station.id().to_string()).is_some());
        assert!(find_station("nonexistent").is_none());
    }

    #[test]
    fn catalog_compiles_a_portable_query_to_postgres_sql() {
        let catalog = build_catalog();
        let compiler = QueryCompiler::new();
        let parsed = compiler
            .parse(
                &GenericSqlDialect,
                "SELECT city, pm25 FROM readings JOIN stations \
                 ON readings.station_id = stations.station_id LIMIT 10",
            )
            .expect("parse");
        let bound = compiler
            .bind_and_validate(&parsed, &catalog, &QueryParameters::default())
            .expect("bind");
        let artifact = compiler
            .emit(&bound, &PostgresAdapter, &catalog)
            .expect("emit");
        let sql = artifact.as_sql().expect("sql artifact");
        assert!(sql.text.contains("readings"));
        assert!(sql.text.contains("stations"));
    }
}
