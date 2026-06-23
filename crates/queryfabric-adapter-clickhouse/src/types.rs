use std::fmt;

use arrow::datatypes::{DataType as ArrowDataType, TimeUnit};

/// ClickHouse column type used in table definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChType {
    Bool,
    Int16,
    Int32,
    UInt16,
    UInt64,
    Float64,
    String,
    Uuid,
    Json,
    DateTime64(u8),
    LowCardinalityString,
    Enum8(&'static str),
}

impl ChType {
    pub const fn needs_arrow_cast(self) -> bool {
        matches!(self, Self::Uuid | Self::Json)
    }

    pub fn to_simple(self) -> SimpleColumnType {
        match self {
            Self::Int16 | Self::Int32 | Self::UInt16 | Self::UInt64 => SimpleColumnType::Int,
            Self::Float64 => SimpleColumnType::Float,
            Self::String | Self::LowCardinalityString => SimpleColumnType::Ascii,
            Self::Uuid => SimpleColumnType::Uuid,
            Self::Bool => SimpleColumnType::Boolean,
            Self::DateTime64(_) => SimpleColumnType::Timestamp,
            Self::Enum8(_) => SimpleColumnType::Ascii,
            Self::Json => SimpleColumnType::Text,
        }
    }

    pub fn to_arrow(self) -> ArrowDataType {
        match self {
            Self::Uuid => ArrowDataType::Utf8,
            Self::String => ArrowDataType::Utf8,
            Self::Json => ArrowDataType::Utf8,
            Self::Bool => ArrowDataType::Boolean,
            Self::Int16 => ArrowDataType::Int16,
            Self::Int32 => ArrowDataType::Int32,
            Self::UInt16 => ArrowDataType::UInt16,
            Self::UInt64 => ArrowDataType::UInt64,
            Self::Float64 => ArrowDataType::Float64,
            Self::DateTime64(3) => {
                ArrowDataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into()))
            }
            Self::DateTime64(6) => {
                ArrowDataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into()))
            }
            Self::DateTime64(_) => {
                ArrowDataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into()))
            }
            Self::LowCardinalityString => ArrowDataType::Utf8,
            Self::Enum8(_) => ArrowDataType::Utf8,
        }
    }
}

impl fmt::Display for ChType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool => f.write_str("Bool"),
            Self::Int16 => f.write_str("Int16"),
            Self::Int32 => f.write_str("Int32"),
            Self::UInt16 => f.write_str("UInt16"),
            Self::UInt64 => f.write_str("UInt64"),
            Self::Float64 => f.write_str("Float64"),
            Self::String => f.write_str("String"),
            Self::Uuid => f.write_str("UUID"),
            Self::Json => f.write_str("JSON"),
            Self::DateTime64(p) => write!(f, "DateTime64({p})"),
            Self::LowCardinalityString => f.write_str("LowCardinality(String)"),
            Self::Enum8(variants) => write!(f, "Enum8({variants})"),
        }
    }
}

/// Simplified column type category for API consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimpleColumnType {
    Int,
    Float,
    Ascii,
    Uuid,
    Boolean,
    Timestamp,
    Text,
    Unknown,
}

impl fmt::Display for SimpleColumnType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => f.write_str("int"),
            Self::Float => f.write_str("float"),
            Self::Ascii => f.write_str("ascii"),
            Self::Uuid => f.write_str("uuid"),
            Self::Boolean => f.write_str("boolean"),
            Self::Timestamp => f.write_str("timestamp"),
            Self::Text => f.write_str("text"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}
