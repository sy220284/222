//! Data-driven original 2.1.88 golden regression of REAL Rust production adapters.
//! New registry entries are exercised without adding a new test function.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

#[test]
fn all_registered_source_goldens_execute_against_production_adapters() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry = fs::read_to_string(repo.join("docs/SOURCE_TEST_REGISTRY.jsonl"))
        .expect("source regression registry must exist");
    let bin_dir = Path::new(env!("CARGO_BIN_EXE_atlas-agent"))
        .parent()
        .expect("production executable parent missing");
    let mut count = 0_usize;
    for line in registry.lines().filter(|line| !line.trim().is_empty()) {
        let entry: Value = serde_json::from_str(line).expect("invalid source test registry JSON");
        let source_path = entry["path"]
            .as_str()
            .expect("missing original source path");
        let golden_path = entry["golden"].as_str().expect("missing golden path");
        assert!(
            golden_path.starts_with("tests/source_parity/golden/")
                && !golden_path.contains("..")
                && !golden_path.contains('\\'),
            "golden fixture must stay inside the versioned test corpus"
        );
        let fixture: Value = serde_json::from_slice(
            &fs::read(repo.join(golden_path)).expect("original golden fixture unavailable"),
        )
        .expect("original golden JSON invalid");
        assert_eq!(fixture["source_path"], source_path);
        assert_eq!(fixture["source_sha256"], entry["source_sha256"]);
        let adapter = &entry["adapter"];
        let binary = adapter["binary"]
            .as_str()
            .expect("missing real Rust adapter binary");
        assert!(
            binary.starts_with("parity_")
                && binary
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'),
            "invalid Rust adapter name"
        );
        assert!(
            repo.join("src/bin").join(format!("{binary}.rs")).is_file(),
            "registered Rust production adapter source missing: {binary}"
        );
        let executable = bin_dir.join(format!("{binary}{}", std::env::consts::EXE_SUFFIX));
        assert!(
            executable.is_file(),
            "Rust adapter has not been built: {}",
            executable.display()
        );
        let key = adapter["input_key"].as_str().expect("missing input key");
        let pointer = adapter["observation_pointer"]
            .as_str()
            .expect("missing output JSON pointer");
        let source_ids = entry["source_ids"]
            .as_array()
            .expect("missing original AST function IDs");
        let cases = fixture["cases"]
            .as_array()
            .expect("missing source golden cases");
        assert!(cases.len() >= 2, "normal and boundary cases required");
        for case in cases {
            let mut input = serde_json::Map::new();
            input.insert(key.to_string(), case["input"].clone());
            let request = json!({"id":case["id"],"input":input});
            let mut child = Command::new(&executable)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("failed to execute the registered Rust adapter");
            child
                .stdin
                .take()
                .unwrap()
                .write_all(request.to_string().as_bytes())
                .unwrap();
            let observed = child.wait_with_output().unwrap();
            assert!(
                observed.status.success(),
                "{source_path}/{}: {}",
                case["id"],
                String::from_utf8_lossy(&observed.stderr)
            );
            let result: Value = serde_json::from_slice(&observed.stdout)
                .expect("Rust adapter did not return one JSON observation");
            assert_eq!(result["schema"], "source-rust-observation/v1");
            assert_eq!(result["case_id"], case["id"]);
            assert_eq!(result["executed_source_ids"], entry["source_ids"]);
            let actual = result.pointer(pointer).unwrap_or_else(|| {
                panic!("{source_path}: Rust adapter missing expected runtime output {pointer}")
            });
            assert_eq!(actual, &case["expected"], "{source_path}/{}", case["id"]);
        }
        count += 1;
    }
    assert!(count > 0, "source regression corpus cannot be empty");
}
