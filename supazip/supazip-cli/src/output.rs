//! Structured output support for the CLI (`--output json|yaml|text`).

use serde::Serialize;

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable table (the legacy default).
    Text,
    /// JSON.
    Json,
    /// YAML.
    Yaml,
}

/// Serialize `value` to stdout in the requested format.
///
/// For [`OutputFormat::Text`] the caller is responsible for printing its own
/// human-readable representation — this function returns `Ok(())` without
/// printing anything so the caller can keep its existing text-path code.
pub fn print<T: Serialize>(
    format: OutputFormat,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Text => Ok(()),
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(value)?);
            Ok(())
        }
        OutputFormat::Yaml => {
            print!("{}", serde_yaml::to_string(value)?);
            Ok(())
        }
    }
}
