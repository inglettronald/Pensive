//! The run configurations a project saves in `.pensive/project.toml`, as `[[run]]` tables.

use anyhow::Result;
use serde::Deserialize;

/// Where a project keeps its own settings, relative to its root.
pub const PROJECT_FILE: &str = ".pensive/project.toml";

/// A command the Run button can start, under the name its picker shows.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct RunConfig {
    pub name: String,
    pub command: String,
}

/// The parts of a project file this crate reads. Other tables are left for their readers.
#[derive(Deserialize)]
struct ProjectFile {
    #[serde(default)]
    run: Vec<RunConfig>,
}

/// The run configurations in a project file's text, in the order it lists them.
pub fn run_configs(text: &str) -> Result<Vec<RunConfig>> {
    Ok(toml::from_str::<ProjectFile>(text)?.run)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_each_run_table_in_order_and_ignores_other_tables() {
        let text = r#"
            icon = "logo.svg"

            [[run]]
            name = "pensive"
            command = "cargo run"

            [[run]]
            name = "mocks"
            command = "cargo mocks"
        "#;
        let names: Vec<String> = run_configs(text)
            .unwrap()
            .into_iter()
            .map(|config| config.name)
            .collect();
        assert_eq!(names, ["pensive", "mocks"]);
    }

    #[test]
    fn a_file_without_run_tables_has_none() {
        assert!(run_configs("icon = \"logo.svg\"\n").unwrap().is_empty());
    }

    #[test]
    fn a_run_table_without_a_command_is_an_error() {
        assert!(run_configs("[[run]]\nname = \"pensive\"\n").is_err());
    }
}
