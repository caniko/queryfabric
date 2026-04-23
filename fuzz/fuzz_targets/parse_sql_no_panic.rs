#![no_main]

use libfuzzer_sys::fuzz_target;
use queryfabric::{GenericSqlDialect, QueryCompiler, SyqlDialect};

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data);
    let compiler = QueryCompiler::default();
    let _ = compiler.parse(&GenericSqlDialect, &input);
    let _ = compiler.parse(&SyqlDialect, &input);
});
