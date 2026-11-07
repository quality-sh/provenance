//! The CLI spelling of the recovery inputs that a porcelain refusal gives.

use serde_json::Value;

/// One command invocation for `input`: each field is a `--kebab-case` option, and each
/// item of a list is one more use of the option.
pub(crate) fn command_form(command: &str, input: &Value) -> String {
    let mut form = format!("provenance {command}");
    for (field, value) in input.as_object().into_iter().flatten() {
        let option = format!("--{}", field.replace('_', "-"));
        let values = match value {
            Value::Array(items) => items.iter().collect(),
            other => vec![other],
        };
        for value in values {
            match value {
                Value::Bool(true) => form.push_str(&format!(" {option}")),
                Value::String(text) => form.push_str(&format!(" {option} {text}")),
                other => form.push_str(&format!(" {option} {other}")),
            }
        }
    }
    form
}

/// `message` followed by the command forms of `inputs`, when there are any.
pub(crate) fn with_recovery(message: &str, command: &str, inputs: &[Value]) -> String {
    if inputs.is_empty() {
        return message.to_owned();
    }
    let forms = inputs
        .iter()
        .map(|input| format!("`{}`", command_form(command, input)))
        .collect::<Vec<_>>();
    format!("{message}; run {}", forms.join(" or "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_field_is_one_option_and_each_list_item_one_use() {
        assert_eq!(
            command_form(
                "search",
                &json!({"kind": ["<kind>"], "include_terminal": true})
            ),
            "provenance search --include-terminal --kind <kind>"
        );
        assert_eq!(
            with_recovery("no query", "search", &[json!({"text": "<query>"})]),
            "no query; run `provenance search --text <query>`"
        );
        assert_eq!(with_recovery("denied", "search", &[]), "denied");
    }
}
