//! Interactive, user-owned answer collection corresponding to 2.1.88
//! AskUserQuestionTool. Model-supplied "answers" are intentionally ignored.
use anyhow::{bail, Context, Result};
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
        },"required":["questions"]}
    }})
}

fn validate(input: &Value) -> Result<&[Value]> {
    let questions = input
        .get("questions")
        .and_then(Value::as_array)
        .context("questions must be an array")?;
    if !(1..=4).contains(&questions.len()) {
        bail!("AskUserQuestion requires 1-4 questions");
    }
    let mut seen_questions = HashSet::new();
    for q in questions {
        let question = q
            .get("question")
            .and_then(Value::as_str)
            .context("question text required")?
            .trim();
        if question.is_empty() || !seen_questions.insert(question) {
            bail!("Question texts must be nonempty and unique");
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
                .context("option label required")?
                .trim();
            opt.get("description")
                .and_then(Value::as_str)
                .context("option description required")?;
            if label.is_empty() || label.eq_ignore_ascii_case("other") || !labels.insert(label) {
                bail!("Option labels must be nonempty, unique and cannot be Other");
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
        let (answer, selected_indices) = loop {
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
            if !valid || selected.is_empty() || (selected.contains(&0) && selected.len() > 1) {
                writeln!(writer, "Invalid choice; use the listed numbers.")?;
                continue;
            }
            if selected == [0] {
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
                break (custom.trim().to_string(), Vec::new());
            }
            let answer = selected
                .iter()
                .map(|n| opts[n - 1]["label"].as_str().expect("validated"))
                .collect::<Vec<_>>()
                .join(", ");
            break (answer, selected);
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
        if !previews.is_empty() {
            let mut annotation = Map::new();
            annotation.insert("preview".to_owned(), Value::String(previews.join("\n\n")));
            write!(writer, "Optional notes for this selection (Enter to skip): ")?;
            writer.flush()?;
            let mut note = String::new();
            // A closed input stream means there are no additional notes;
            // the user's already-confirmed choice must not be discarded.
            if reader.read_line(&mut note)? > 0 && !note.trim().is_empty() {
                annotation.insert("notes".to_owned(), Value::String(note.trim().to_owned()));
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
