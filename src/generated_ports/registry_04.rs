//! Hand-ported runtime functions that were absent from generated registries.
use serde_json::Value;
pub fn inspect(source:&str, export:&str, args:&[Value]) -> Option<Value> {
    if source == "src/utils/truncate.ts" { return super::terminal_truncate::inspect(export, args); }
    match (source, export) {
        ("src/hooks/renderPlaceholder.ts", "renderPlaceholder") =>
            Some(super::text_input_semantics::render_placeholder(args.first()?)),
        ("src/utils/textHighlighting.ts", "segmentTextByHighlights") =>
            Some(serde_json::json!(super::text_input_semantics::segment_text_by_highlights(
                args.first()?.as_str()?,args.get(1)?.as_array()?))),
        _ => None,
    }
}
pub fn catalog() -> Vec<(&'static str,Vec<&'static str>)> {
    vec![
      ("src/hooks/renderPlaceholder.ts",vec!["renderPlaceholder"]),
      ("src/utils/truncate.ts",vec!["truncatePathMiddle","truncateToWidth","truncateStartToWidth","truncateToWidthNoEllipsis","truncate","wrapText"]),
      ("src/utils/textHighlighting.ts",vec!["segmentTextByHighlights"]),
    ]
}
