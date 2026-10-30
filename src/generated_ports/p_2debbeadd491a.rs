//! Rust exports derived from original: src/services/autoDream/consolidationPrompt.ts
//!
//! The consolidation prompt template is carried verbatim (no vendor terms).
//! DIR_EXISTS_GUIDANCE is injected by the caller from the memdir port once
//! that constant lands, matching the original import.

/// `buildConsolidationPrompt` — dream prompt over the memory root and
/// transcript directory, with extra caller guidance appended.
pub fn build_consolidation_prompt(
    memory_root: &str,
    transcript_dir: &str,
    dir_exists_guidance: &str,
    extra: &str,
) -> String {
    format!(
        "# Dream: Memory Consolidation\n\nYou are performing a dream \u{2014} a reflective pass over your memory files. Synthesize what you've learned recently into durable, well-organized memories so that future sessions can orient quickly.\n\nMemory directory: `{memory_root}`\n{dir_exists_guidance}\n\nSession transcripts: `{transcript_dir}` (large JSONL files \u{2014} grep narrowly, don't read whole files)\n\n---\n{extra}"
    )
}
