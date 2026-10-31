use serde_json::json;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn shell_spec_command_exposes_full_source_srun_and_pyright_options() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config.json");
    fs::write(
        &config,
        json!({
            "default_provider": "offline",
            "providers": [{
                "id": "offline",
                "kind": "openai-compatible",
                "model": "offline",
                "base_url": "http://127.0.0.1:9/v1"
            }]
        })
        .to_string(),
    )
    .unwrap();
    let mut proc = Command::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .args(["--workspace", root.path().to_str().unwrap(), "chat"])
        .env("ATLAS_CONFIG", &config)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut input = proc.stdin.take().unwrap();
        writeln!(input, "/shell-spec").unwrap();
        writeln!(input, "/exit").unwrap();
    }
    let result = proc.wait_with_output().unwrap();
    assert!(
        result.status.success(),
        "shell-spec failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output = String::from_utf8(result.stdout).unwrap();
    for expected in [
        "srun — Run a command on SLURM cluster nodes",
        "-n, --ntasks <count>",
        "-N, --nodes <count>",
        "pyright — Type checker for Python",
        "--pythonplatform <PLATFORM>",
        "--pythonversion <VERSION>",
        "--venvpath, -v <DIRECTORY>",
        "--threads <N>",
        "files — Specify files or directories",
    ] {
        assert!(
            output.contains(expected),
            "missing command option {expected}: {output}"
        );
    }
}
