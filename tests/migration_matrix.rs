use std::{collections::HashSet,fs,path::Path};

#[test]
fn every_original_path_is_tracked_once_and_no_partial_work_is_called_complete(){
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));
    let ledger=fs::read_to_string(root.join("docs/FILE_PORT_MATRIX.md")).unwrap();
    let mut paths=HashSet::new();
    let mut omitted=0usize;
    let mut tracked=0usize;
    for line in ledger.lines(){
        if !line.starts_with("| `src/") && !line.starts_with("| `vendor/"){continue}
        let fields=line.split('|').map(str::trim).collect::<Vec<_>>();
        assert!(fields.len()>=8,"Invalid migration row {line}");
        let path=fields[1];
        assert!(paths.insert(path.to_owned()),"Duplicate original path {path}");
        assert!(!fields[4].is_empty(),"Missing action for {path}");
        assert!(!fields[5].is_empty(),"Missing Rust target for {path}");
        if fields[4].starts_with("排除") {omitted+=1}
        if fields[6].contains("完成") && !fields[6].contains("未完成") {
            panic!("File-level parity cannot be claimed without evidence: {path}");
        }
        tracked+=1;
    }
    assert_eq!(tracked,1906);
    assert_eq!(omitted,22);
}
