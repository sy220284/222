//! Native Rust port derived from original: src/utils/bash/specs/pyright.ts
//!
//! `pyright` command specification. The original's withfig-style
//! `options` array plus a variadic `files` argument is normalized into
//! this repository's flat `CommandSpec::arguments` list (option flags
//! keep both spellings in `name`, `args` become optional arguments,
//! trailing variadic file list keeps `variadic: true`).
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::portable_missing_utilities::{ArgumentSpec, CommandSpec};

fn flag(name: &str, description: &str) -> ArgumentSpec {
    ArgumentSpec {
        name: name.to_string(),
        description: description.to_string(),
        optional: true,
        variadic: false,
        is_command: false,
        is_script: false,
    }
}

fn flag_with_arg(name: &str, description: &str, arg_name: &str) -> ArgumentSpec {
    ArgumentSpec {
        name: format!("{} <{}>", name, arg_name),
        description: description.to_string(),
        optional: true,
        variadic: false,
        is_command: false,
        is_script: false,
    }
}

/// Type checker for Python.
pub fn pyright_spec() -> CommandSpec {
    CommandSpec {
        name: "pyright".to_string(),
        description: "Type checker for Python".to_string(),
        arguments: vec![
            flag("--help, -h", "Show help message"),
            flag("--version", "Print pyright version and exit"),
            flag("--watch, -w", "Continue to run and watch for changes"),
            flag_with_arg(
                "--project, -p",
                "Use the configuration file at this location",
                "FILE OR DIRECTORY",
            ),
            flag("-", "Read file or directory list from stdin"),
            flag_with_arg(
                "--createstub",
                "Create type stub file(s) for import",
                "IMPORT",
            ),
            flag_with_arg(
                "--typeshedpath, -t",
                "Use typeshed type stubs at this location",
                "DIRECTORY",
            ),
            flag_with_arg(
                "--verifytypes",
                "Verify completeness of types in py.typed package",
                "IMPORT",
            ),
            flag(
                "--ignoreexternal",
                "Ignore external imports for --verifytypes",
            ),
            flag_with_arg("--pythonpath", "Path to the Python interpreter", "FILE"),
            flag_with_arg(
                "--venvpath",
                "Directory that contains virtual environments",
                "DIRECTORY",
            ),
            flag("--outputjson", "Output results in JSON format"),
            flag("--verbose", "Emit verbose diagnostics"),
            flag("--stats", "Print detailed performance stats"),
            flag("--dependencies", "Emit import dependency information"),
            flag_with_arg("--level", "Minimum diagnostic level", "LEVEL"),
            flag(
                "--skipunannotated",
                "Skip type analysis of unannotated functions",
            ),
            flag("--warnings", "Use exit code of 1 if warnings are reported"),
            ArgumentSpec {
                name: "--threads <N>".to_string(),
                description: "Use up to N threads to parallelize type checking".to_string(),
                optional: true,
                variadic: false,
                is_command: false,
                is_script: false,
            },
            ArgumentSpec {
                name: "files".to_string(),
                description: "Specify files or directories to analyze (overrides config file)"
                    .to_string(),
                optional: true,
                variadic: true,
                is_command: false,
                is_script: false,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pyright_spec_shape() {
        let spec = pyright_spec();
        assert_eq!(spec.name, "pyright");
        // 19 option flags + variadic files argument.
        assert_eq!(spec.arguments.len(), 20);
        let files = spec.arguments.last().unwrap();
        assert_eq!(files.name, "files");
        assert!(files.variadic && files.optional);
        assert!(spec.arguments.iter().any(|a| a.name.contains("--watch")));
        assert!(spec.arguments.iter().any(|a| a.name.contains("--project")));
        // --threads has optional arg but is itself a flag.
        let threads = spec
            .arguments
            .iter()
            .find(|a| a.name.starts_with("--threads"))
            .unwrap();
        assert!(threads.optional && !threads.variadic);
    }
}
