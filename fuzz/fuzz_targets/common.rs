use queryfabric::MemoryCatalog;

pub fn portable_catalog() -> MemoryCatalog {
    queryfabric::portable_catalog("fuzz-catalog")
}
