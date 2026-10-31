//! Original src/utils/cliArgs.ts early flag and double-dash parsing.
pub fn eager_parse_cli_flag(flag: &str, argv: &[String]) -> Option<String> {
    for (index, argument) in argv.iter().enumerate() {
        if let Some(value) = argument.strip_prefix(flag).and_then(|v| v.strip_prefix('=')) {
            return Some(value.to_owned());
        }
        if argument == flag {
            if let Some(value) = argv.get(index + 1) {
                return Some(value.clone());
            }
        }
    }
    None
}
pub fn extract_args_after_double_dash(command: &str, args: &[String]) -> (String, Vec<String>) {
    if command == "--" {
        if let Some((first, rest)) = args.split_first() {
            return (first.clone(), rest.to_vec());
        }
    }
    (command.into(), args.to_vec())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_early_flag_precedence_and_empty_values() {
        assert_eq!(eager_parse_cli_flag("--config", &["--config=first".into(),
            "--config=second".into()]),Some("first".into()));
        assert_eq!(eager_parse_cli_flag("--config", &["--config=".into()]), Some("".into()));
        assert_eq!(eager_parse_cli_flag("--config", &["--config".into()]), None);
    }
    #[test]
    fn original_double_dash_keeps_unchanged_commands_and_args() {
        assert_eq!(extract_args_after_double_dash("--", &["run".into(),"-v".into()]),
                   ("run".into(),vec!["-v".into()]));
        assert_eq!(extract_args_after_double_dash("--",&[]),("--".into(),vec![]));
        assert_eq!(extract_args_after_double_dash("run",&["a".into()]),
                   ("run".into(),vec!["a".into()]));
    }
}
