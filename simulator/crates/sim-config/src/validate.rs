//! JSON Schema validation glue.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("io error reading {path}: {source}")]
    Io { path: String, source: std::io::Error },

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON Schema validation failed: {0}")]
    Schema(String),

    #[error("cross-validation failed: {0}")]
    CrossValidation(String),
}

/// Validate an arbitrary `serde_yaml::Value` against the IC manifest schema.
pub fn validate_manifest(_value: &serde_yaml::Value) -> Result<(), ValidationError> {
    // TODO:
    //   1. Embed the schema JSON via include_str! at compile time.
    //   2. Compile the schema once (lazy_static or OnceCell).
    //   3. Convert the YAML value to JSON.
    //   4. Validate.
    //   5. Map errors to a useful message.
    unimplemented!()
}

/// Validate an arbitrary `serde_yaml::Value` against the board schema.
pub fn validate_board(_value: &serde_yaml::Value) -> Result<(), ValidationError> {
    // TODO: same shape as validate_manifest, but pointing at board schema.
    unimplemented!()
}

// TODO: include_str! for the schema files. Path:
//   const MANIFEST_SCHEMA: &str = include_str!("../../../../schema/ic-manifest.schema.json");
//   const BOARD_SCHEMA:    &str = include_str!("../../../../schema/board.schema.json");
