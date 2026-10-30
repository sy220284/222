use std::{collections::HashSet, fs, path::Path};

#[test]
fn partial_source_mappings_exist_and_generated_only_cannot_pass_parity() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix = fs::read_to_string(root.join("docs/FILE_PORT_MATRIX.md")).unwrap();
    let rust_path = regex::Regex::new(r"src/[A-Za-z0-9_./-]+\.rs").unwrap();
    let mut seen = HashSet::new();
    let mut partial = 0usize;
    for line in matrix.lines() {
        if !line.starts_with("| \x60src/") && !line.starts_with("| \x60vendor/") {
            continue;
        }
        let columns: Vec<_> = line.split('|').map(str::trim).collect();
        assert!(columns.len() >= 8, "Invalid source inventory row: {line}");
        let source = columns[1];
        assert!(seen.insert(source), "Duplicate source path: {source}");
        let targets: Vec<_> = rust_path
            .find_iter(columns[5])
            .map(|found| found.as_str())
            .collect();
        if columns[6].contains("部分实现") || columns[6].contains("部分覆盖") {
            partial += 1;
            assert!(
                !targets.is_empty(),
                "No Rust implementation target: {source}"
            );
            for target in &targets {
                assert!(
                    root.join(target).is_file(),
                    "Missing Rust target for {source}: {target}"
                );
            }
        } else if columns[6].starts_with("已验收：") {
            assert!(
                targets
                    .iter()
                    .any(|target| !target.starts_with("src/generated_ports/")),
                "Generated-only structure is not functional parity: {source}"
            );
        }
    }
    assert!(partial <= 1884, "Invalid source inventory count");
}
