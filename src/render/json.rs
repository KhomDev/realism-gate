//! JSON output, schema version 1. Fields are only ever added within a schema
//! version; a removal or a change of meaning bumps `schema`.

use super::{TOOL, VERSION};
use crate::engine::Report;
use serde::Serialize;

pub const SCHEMA: u32 = 1;

#[derive(Serialize)]
struct Document<'a> {
    tool: &'static str,
    version: &'static str,
    schema: u32,
    #[serde(flatten)]
    report: &'a Report,
}

pub fn render(report: &Report) -> String {
    let doc = Document {
        tool: TOOL,
        version: VERSION,
        schema: SCHEMA,
        report,
    };
    let mut out = serde_json::to_string_pretty(&doc).expect("report serializes");
    out.push('\n');
    out
}
