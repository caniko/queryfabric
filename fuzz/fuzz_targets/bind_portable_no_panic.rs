#![no_main]

mod common;

use libfuzzer_sys::fuzz_target;
use queryfabric::{
    GenericSqlDialect, QueryCompiler, QueryParameters, SyqlDialect, bind_and_validate_query,
};

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let compiler = QueryCompiler::default();
    let catalog = common::portable_catalog();

    if let Ok(parsed) = compiler.parse(&GenericSqlDialect, &input) {
        let _ = bind_and_validate_query(&parsed, &catalog, &QueryParameters::default());
    }

    if let Ok(parsed) = compiler.parse(&SyqlDialect, &input) {
        let _ = bind_and_validate_query(&parsed, &catalog, &QueryParameters::default());
    }
});
