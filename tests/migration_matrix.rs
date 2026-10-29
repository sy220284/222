use std::{collections::HashSet, fs, path::Path};

#[test]
fn every_original_path_is_tracked_once_and_no_partial_work_is_called_complete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ledger = fs::read_to_string(root.join("docs/FILE_PORT_MATRIX.md")).unwrap();
    let rust_paths = regex::Regex::new(r"src/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*\.rs").unwrap();
    let mut paths = HashSet::new();
    let mut omitted = 0usize;
    let mut tracked = 0usize;
    let mut partial = 0usize;
    let mut unported = 0usize;
    let mut accepted = 0usize;
    for line in ledger.lines() {
        if !line.starts_with("| `src/") && !line.starts_with("| `vendor/") {
            continue;
        }
        let fields = line.split('|').map(str::trim).collect::<Vec<_>>();
        assert!(fields.len() >= 8, "Invalid migration row {line}");
        let path = fields[1];
        assert!(
            paths.insert(path.to_owned()),
            "Duplicate original path {path}"
        );
        assert!(!fields[4].is_empty(), "Missing action for {path}");
        assert!(!fields[5].is_empty(), "Missing Rust target for {path}");
        if fields[4].starts_with("排除") {
            omitted += 1
        } else if fields[6].contains("部分实现") || fields[6].contains("部分覆盖") {
            partial += 1;
        } else if fields[6].contains("未迁移") {
            unported += 1;
        } else if fields[6].starts_with("已验收：") {
            assert!(
                fields[6].contains("证据"),
                "Missing parity evidence: {path}"
            );
            let targets = rust_paths.find_iter(fields[5]).collect::<Vec<_>>();
            assert!(
                !targets.is_empty(),
                "Accepted source has no Rust target: {path}"
            );
            for target in targets {
                assert!(
                    root.join(target.as_str()).is_file(),
                    "Accepted source {path} references nonexistent Rust target {}",
                    target.as_str()
                );
            }
            accepted += 1;
        } else {
            panic!("Unclassified original file: {path}");
        }
        if fields[6].contains("已验收") && !fields[6].contains("证据") {
            panic!("File-level parity cannot be claimed without evidence: {path}");
        }
        tracked += 1;
    }
    assert_eq!(
        tracked, 1906,
        "Original source inventory changed without reconciliation"
    );
    assert_eq!(omitted, 22, "Explicitly excluded source paths changed");
    assert_eq!(
        partial + unported + accepted + omitted,
        tracked,
        "The source inventory contains unclassified paths"
    );
    let status = fs::read_to_string(root.join("docs/PORT_STATUS.md")).unwrap();
    assert!(
        status.contains(&format!(
            "现有 Rust 模块覆盖部分行为、但未完成逐文件等价：{partial} 个原始文件路径。"
        )),
        "PORT_STATUS partial count disagrees with the matrix"
    );
    assert!(
        status.contains(&format!("当前台账仍标注“未迁移”的文件路径：{unported} 个")),
        "PORT_STATUS unported count disagrees with the matrix"
    );
    assert!(
        status.contains(&format!(
            "达到原始文件纯函数行为/边界等价且有验证证据的路径：{accepted} 个"
        )),
        "PORT_STATUS accepted count disagrees with the matrix"
    );
    assert!(
        status.contains(&format!(
            "非排除项的原始文件级等价验收仍未闭环：{} 个",
            partial + unported
        )),
        "PORT_STATUS remaining count disagrees with the matrix"
    );
}
