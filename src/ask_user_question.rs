//! Interactive, user-owned answer collection corresponding to 2.1.88
//! AskUserQuestionTool. Model-supplied "answers" are intentionally ignored.
use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::io::{self, BufRead, Write};

pub fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"AskUserQuestion",
        "description":"Ask the terminal user one to four multiple-choice questions and return their actual selections. Do not guess user answers.",
        "parameters":{"type":"object","properties":{
            "questions":{"type":"array","minItems":1,"maxItems":4,
                "items":{"type":"object","properties":{
                    "question":{"type":"string"},
                    "header":{"type":"string"},
                    "options":{"type":"array","minItems":2,"maxItems":4,
                        "items":{"type":"object","properties":{
                            "label":{"type":"string"},
                            "description":{"type":"string"},
                            "preview":{"type":"string"}
                        },"required":["label","description"]}},
                    "multiSelect":{"type":"boolean","default":false}
                },"required":["question","header","options"]}}
        },"required":["questions"],"additionalProperties":false}
    }})
}

/// The 2.1.88 HTML-preview mode rejects full documents, executable/style
/// tags, and non-HTML text. In the provider-neutral terminal runtime this
/// mode can be selected explicitly without vendor SDK dependencies.
fn html_preview_error(preview: &str) -> Option<&'static str> {
    let full_document = Regex::new(r"(?i)<\s*(html|body|!doctype)\b").expect("valid regex");
    if full_document.is_match(preview) {
        return Some("preview must be an HTML fragment, not a full document (no <html>, <body>, or <!DOCTYPE>)");
    }
    let executable = Regex::new(r"(?i)<\s*(script|style)\b").expect("valid regex");
    if executable.is_match(preview) {
        return Some("preview must not contain <script> or <style> tags. Use inline styles via the style attribute if needed.");
    }
    let has_html = Regex::new(r"(?i)<[a-z][^>]*>").expect("valid regex");
    if !has_html.is_match(preview) {
        return Some("preview must contain HTML (previewFormat is set to \"html\"). Wrap content in a tag like <div> or <pre>.");
    }
    None
}

fn validate(input: &Value) -> Result<&[Value]> {
    // The source uses z.strictObject at the tool input boundary. Preserve
    // the accepted optional SDK fields, but reject unexpected root keys.
    let object = input
        .as_object()
        .context("AskUserQuestion input must be an object")?;
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "questions" | "answers" | "annotations" | "metadata"
        ) {
            bail!("Unknown AskUserQuestion input field: {key}");
        }
    }
    if let Some(answers) = object.get("answers") {
        let values = answers.as_object().context("answers must be an object")?;
        if values.values().any(|value| !value.is_string()) {
            bail!("answers values must be strings");
        }
    }
    if let Some(annotations) = object.get("annotations") {
        let values = annotations
            .as_object()
            .context("annotations must be an object")?;
        for annotation in values.values() {
            let fields = annotation
                .as_object()
                .context("annotation must be an object")?;
            for field in ["preview", "notes"] {
                if fields.get(field).is_some_and(|value| !value.is_string()) {
                    bail!("annotation {field} must be a string");
                }
            }
        }
    }
    if let Some(metadata) = object.get("metadata") {
        let metadata = metadata.as_object().context("metadata must be an object")?;
        if metadata
            .get("source")
            .is_some_and(|value| !value.is_string())
        {
            bail!("metadata.source must be a string");
        }
    }
    let questions = input
        .get("questions")
        .and_then(Value::as_array)
        .context("questions must be an array")?;
    if !(1..=4).contains(&questions.len()) {
        bail!("AskUserQuestion requires 1-4 questions");
    }
    let mut seen_questions = HashSet::new();
    for q in questions {
        // The original Zod uniqueness refinement compares exact strings,
        // without trimming. Preserve user-visible question text verbatim.
        let question = q
            .get("question")
            .and_then(Value::as_str)
            .context("question text required")?;
        if !seen_questions.insert(question) {
            bail!("Question texts must be unique");
        }
        q.get("header")
            .and_then(Value::as_str)
            .context("header required")?;
        if q.get("multiSelect").is_some_and(|v| !v.is_boolean()) {
            bail!("multiSelect must be a boolean");
        }
        let options = q
            .get("options")
            .and_then(Value::as_array)
            .context("options required")?;
        if !(2..=4).contains(&options.len()) {
            bail!("Each question must have 2-4 options");
        }
        let mut labels = HashSet::new();
        for opt in options {
            let label = opt
                .get("label")
                .and_then(Value::as_str)
                .context("option label required")?;
            opt.get("description")
                .and_then(Value::as_str)
                .context("option description required")?;
            if opt.get("preview").is_some_and(|value| !value.is_string()) {
                bail!("option preview must be a string");
            }
            if std::env::var("ATLAS_QUESTION_PREVIEW_FORMAT")
                .is_ok_and(|format| format.eq_ignore_ascii_case("html"))
            {
                if let Some(preview) = opt.get("preview").and_then(Value::as_str) {
                    if let Some(error) = html_preview_error(preview) {
                        bail!("Option \"{label}\" in question \"{question}\": {error}");
                    }
                }
            }
            // The source schema checks only exact duplicate labels. "Other"
            // is UI guidance, not an additional schema-level rejection.
            if !labels.insert(label) {
                bail!("Option labels must be unique within each question");
            }
        }
    }
    Ok(questions.as_slice())
}

fn read_selection(reader: &mut impl BufRead, writer: &mut impl Write) -> Result<String> {
    write!(
        writer,
        "Choose a number (or comma-separated numbers), 0 for Other: "
    )?;
    writer.flush()?;
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        bail!("Question cancelled: terminal input closed");
    }
    Ok(line.trim().to_string())
}

/// The model is not allowed to inject purported user answers via input.answers.
pub fn execute(input: &Value, reader: &mut impl BufRead, writer: &mut impl Write) -> Result<Value> {
    let questions = validate(input)?;
    let mut answers = Map::new();
    let mut annotations = Map::new();
    for q in questions {
        let question = q["question"].as_str().expect("validated");
        let opts = q["options"].as_array().expect("validated");
        let multiple = q["multiSelect"].as_bool().unwrap_or(false);
        writeln!(
            writer,
            "\n{}: {}",
            q["header"].as_str().unwrap_or(""),
            question
        )?;
        for (i, option) in opts.iter().enumerate() {
            writeln!(
                writer,
                "  {}) {} — {}",
                i + 1,
                option["label"].as_str().expect("validated"),
                option["description"].as_str().expect("validated")
            )?;
            if let Some(preview) = option.get("preview").and_then(Value::as_str) {
                writeln!(writer, "     {}", preview)?;
            }
        }
        writeln!(writer, "  0) Other (enter a custom answer)")?;
        let (answer, selected_indices, custom_note) = loop {
            let raw = read_selection(reader, writer)?;
            let selections: Vec<&str> = raw.split(',').map(str::trim).collect();
            if selections.len() > 1 && !multiple {
                writeln!(writer, "Select exactly one option.")?;
                continue;
            }
            if selections.iter().any(|s| s.is_empty()) {
                writeln!(writer, "Enter a valid option number.")?;
                continue;
            }
            let mut selected = Vec::<usize>::new();
            let mut valid = true;
            for item in selections {
                match item.parse::<usize>() {
                    Ok(n) if n <= opts.len() => {
                        if !selected.contains(&n) {
                            selected.push(n);
                        }
                    }
                    _ => {
                        valid = false;
                        break;
                    }
                }
            }
            if !valid || selected.is_empty() {
                writeln!(writer, "Invalid choice; use the listed numbers.")?;
                continue;
            }
            if selected.contains(&0) {
                // QuestionView.tsx SelectMulti includes __other__ alongside
                // regular options and appends the typed value to the answers.
                // Multi-select permits Other together with listed choices.
                write!(writer, "Your answer: ")?;
                writer.flush()?;
                let mut custom = String::new();
                if reader.read_line(&mut custom)? == 0 {
                    bail!("Question cancelled: terminal input closed");
                }
                if custom.trim().is_empty() {
                    writeln!(writer, "Answer cannot be empty.")?;
                    continue;
                }
                let custom = custom.trim().to_owned();
                let selected_indices = selected.into_iter().filter(|n| *n > 0).collect::<Vec<_>>();
                let mut labels = selected_indices
                    .iter()
                    .map(|n| opts[*n - 1]["label"].as_str().expect("validated"))
                    .collect::<Vec<_>>();
                labels.push(&custom);
                let answer = labels.join(", ");
                break (answer, selected_indices, Some(custom));
            }
            let answer = selected
                .iter()
                .map(|n| opts[n - 1]["label"].as_str().expect("validated"))
                .collect::<Vec<_>>()
                .join(", ");
            break (answer, selected, None);
        };
        // The original AskUserQuestion result carries the selected preview and
        // user-authored notes as per-question annotations. Never trust the
        // model-supplied input.annotations: collect both from the actual
        // terminal interaction instead.
        let previews = selected_indices
            .iter()
            .filter_map(|index| opts[index - 1].get("preview").and_then(Value::as_str))
            .filter(|preview| !preview.is_empty())
            .collect::<Vec<_>>();
        if !previews.is_empty() || custom_note.is_some() {
            let mut annotation = Map::new();
            if !previews.is_empty() {
                annotation.insert("preview".to_owned(), Value::String(previews.join("\n\n")));
                write!(
                    writer,
                    "Optional notes for this selection (Enter to skip): "
                )?;
                writer.flush()?;
                let mut note = String::new();
                // Closed input must not discard the already confirmed choice.
                if reader.read_line(&mut note)? > 0 && !note.trim().is_empty() {
                    annotation.insert("notes".to_owned(), Value::String(note.trim().to_owned()));
                }
            }
            // The original permission dialog carries custom "Other" text
            // in annotations.notes as well as in the answer map.
            if let Some(note) = custom_note {
                annotation.insert("notes".to_owned(), Value::String(note));
            }
            annotations.insert(question.to_owned(), Value::Object(annotation));
        }
        answers.insert(question.to_owned(), Value::String(answer));
    }
    let mut result = json!({"questions":questions,"answers":answers});
    if !annotations.is_empty() {
        result["annotations"] = Value::Object(annotations);
    }
    Ok(result)
}

/// Mirror the original tool-result message, preserving the order in which
/// questions were asked and carrying user-selected previews and notes back
/// into the model's next turn. This is produced only from terminal answers.
pub fn format_result_for_model(result: &Value) -> String {
    let answers = result.get("answers").and_then(Value::as_object);
    let annotations = result.get("annotations").and_then(Value::as_object);
    let mut parts = Vec::new();
    if let Some(questions) = result.get("questions").and_then(Value::as_array) {
        for question in questions {
            let Some(text) = question.get("question").and_then(Value::as_str) else {
                continue;
            };
            let Some(answer) = answers
                .and_then(|answers| answers.get(text))
                .and_then(Value::as_str)
            else {
                continue;
            };
            let mut part = format!("\"{text}\"=\"{answer}\"");
            if let Some(annotation) = annotations.and_then(|values| values.get(text)) {
                if let Some(preview) = annotation.get("preview").and_then(Value::as_str) {
                    part.push_str(&format!(" selected preview:\n{preview}"));
                }
                if let Some(notes) = annotation.get("notes").and_then(Value::as_str) {
                    part.push_str(&format!(" user notes: {notes}"));
                }
            }
            parts.push(part);
        }
    }
    format!(
        "User has answered your questions: {}. You can now continue with the user's answers in mind.",
        parts.join(", ")
    )
}

pub fn ask_from_terminal(input: &Value) -> Result<Value> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stderr = io::stderr();
    let mut writer = stderr.lock();
    execute(input, &mut reader, &mut writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn questions(multi: bool) -> Value {
        json!({"questions":[{
            "question":"Which backend?","header":"Backend",
            "options":[{"label":"SQLite","description":"Local storage"},
                       {"label":"Postgres","description":"Remote storage"}],
            "multiSelect":multi
        }]})
    }
    #[test]
    fn reads_real_choice_and_ignores_forged_model_answers() {
        let mut input = questions(false);
        input["answers"] = json!({"Which backend?":"Injected"});
        let result = execute(&input, &mut "9\n2\n".as_bytes(), &mut Vec::new()).unwrap();
        assert_eq!(result["answers"]["Which backend?"], "Postgres");
    }
    #[test]
    fn multiple_and_custom_answer() {
        let multiple =
            execute(&questions(true), &mut "2,1,2\n".as_bytes(), &mut Vec::new()).unwrap();
        assert_eq!(multiple["answers"]["Which backend?"], "Postgres, SQLite");
        let custom = execute(
            &questions(false),
            &mut "0\nDuckDB\n".as_bytes(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(custom["answers"]["Which backend?"], "DuckDB");
    }
    #[test]
    fn invalid_schema_and_closed_input() {
        let mut duplicate = questions(false);
        duplicate["questions"][0]["options"][1]["label"] = json!("SQLite");
        assert!(validate(&duplicate).is_err());
        assert!(execute(&questions(false), &mut "".as_bytes(), &mut Vec::new()).is_err());
    }
}
