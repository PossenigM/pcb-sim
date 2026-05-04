//! JSON Schema validation glue.
//!
//! Schemas are embedded at compile time via `include_str!`. Validation
//! converts the caller's `serde_yaml::Value` to `serde_json::Value` first
//! (jsonschema operates on JSON values), then runs the compiled validator.

use thiserror::Error;

const MANIFEST_SCHEMA_JSON: &str =
    include_str!("../../../../schema/ic-manifest.schema.json");
const BOARD_SCHEMA_JSON: &str =
    include_str!("../../../../schema/board.schema.json");

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("io error reading {path}: {source}")]
    Io { path: String, source: std::io::Error },

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON Schema validation failed:\n{0}")]
    Schema(String),

    #[error("cross-validation failed:\n{0}")]
    CrossValidation(String),
}

/// Convert a `serde_yaml::Value` to `serde_json::Value`.
/// All keys in our YAML files are strings, so this is lossless.
pub(crate) fn yaml_to_json(
    yaml: &serde_yaml::Value,
) -> Result<serde_json::Value, ValidationError> {
    serde_json::to_value(yaml)
        .map_err(|e| ValidationError::Schema(format!("YAML→JSON conversion failed: {e}")))
}

/// Validate an arbitrary `serde_yaml::Value` against the IC manifest schema.
pub fn validate_manifest(value: &serde_yaml::Value) -> Result<(), ValidationError> {
    let schema: serde_json::Value = serde_json::from_str(MANIFEST_SCHEMA_JSON)
        .expect("embedded manifest schema is valid JSON");
    let instance = yaml_to_json(value)?;
    run_validation(&schema, &instance)
}

/// Validate an arbitrary `serde_yaml::Value` against the board schema.
pub fn validate_board(value: &serde_yaml::Value) -> Result<(), ValidationError> {
    let schema: serde_json::Value = serde_json::from_str(BOARD_SCHEMA_JSON)
        .expect("embedded board schema is valid JSON");
    let instance = yaml_to_json(value)?;
    run_validation(&schema, &instance)
}

fn run_validation(
    schema: &serde_json::Value,
    instance: &serde_json::Value,
) -> Result<(), ValidationError> {
    let compiled = jsonschema::JSONSchema::compile(schema)
        .map_err(|e| ValidationError::Schema(format!("schema compile error: {e}")))?;
    if let Err(errors) = compiled.validate(instance) {
        let messages: Vec<String> = errors.map(|e| e.to_string()).collect();
        return Err(ValidationError::Schema(messages.join("\n")));
    }
    Ok(())
}
