//! Native Rust port derived from original: src/utils/bash/specs/srun.ts
//!
//! SLURM `srun` command specification for the bash command registry.
//! The original declares withfig-style `options` + single `args`; this
//! repository's `portable_missing_utilities::CommandSpec` normalizes both
//! into a flat `arguments` list, so each option becomes an optional
//! `ArgumentSpec` whose name carries both flag spellings, followed by the
//! trailing command argument. Semantics (two flags, command arg) preserved.
#![allow(dead_code, non_snake_case, non_camel_case_types)]

use crate::portable_missing_utilities::{ArgumentSpec, CommandSpec};

fn argument(name: &str, description: &str, optional: bool, is_command: bool) -> ArgumentSpec {
    ArgumentSpec {
        name: name.to_string(),
        description: description.to_string(),
        optional,
        variadic: false,
        is_command,
        is_script: false,
    }
}

/// Run a command on SLURM cluster nodes.
pub fn srun_spec() -> CommandSpec {
    CommandSpec {
        name: "srun".to_string(),
        description: "Run a command on SLURM cluster nodes".to_string(),
        arguments: vec![
            argument("-n, --ntasks <count>", "Number of tasks", true, false),
            argument(
                "-N, --nodes <count>",
                "Number of nodes to allocate",
                true,
                false,
            ),
            argument("command", "Command to run on the cluster", false, true),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srun_spec_shape() {
        let spec = srun_spec();
        assert_eq!(spec.name, "srun");
        assert_eq!(spec.arguments.len(), 3);
        let ntasks = &spec.arguments[0];
        assert!(ntasks.optional);
        assert!(!ntasks.is_command);
        assert!(ntasks.name.contains("-n") && ntasks.name.contains("--ntasks"));
        let nodes = &spec.arguments[1];
        assert!(nodes.name.contains("-N") && nodes.name.contains("--nodes"));
        let command = &spec.arguments[2];
        assert!(!command.optional);
        assert!(command.is_command);
    }
}
