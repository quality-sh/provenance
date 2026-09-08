//! Preserve closed failure types when unions have distinct required tags.
use serde_json::Value;
use std::collections::BTreeSet;

pub fn normalize(value: &mut Value, root: &Value) {
    match value {
        Value::Object(map) => {
            if let Some(branches) = map.get("anyOf").and_then(Value::as_array) {
                let expanded = branches
                    .iter()
                    .map(|branch| alternatives(branch, root, 0))
                    .collect::<Option<Vec<_>>>();
                if let Some(expanded) = expanded {
                    let mut unique = Vec::new();
                    for branch in expanded.into_iter().flatten() {
                        if !unique.contains(&branch) {
                            unique.push(branch);
                        }
                    }
                    if !unique.is_empty() && disjoint(&unique, root) {
                        map.remove("anyOf");
                        map.insert("oneOf".into(), Value::Array(unique));
                    }
                }
            }
            for child in map.values_mut() {
                normalize(child, root);
            }
        }
        Value::Array(values) => {
            for child in values {
                normalize(child, root);
            }
        }
        _ => {}
    }
}

fn disjoint(branches: &[Value], root: &Value) -> bool {
    let mut seen = BTreeSet::new();
    branches.iter().all(|branch| {
        tags(branch, root, 0).is_some_and(|values| values.into_iter().all(|tag| seen.insert(tag)))
    })
}

fn alternatives(value: &Value, root: &Value, depth: usize) -> Option<Vec<Value>> {
    if depth > 32 {
        return None;
    }
    if value == &Value::Bool(false) {
        return Some(Vec::new());
    }
    if value.as_object()?.len() == 1 {
        if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
            return alternatives(root.pointer(reference.strip_prefix('#')?)?, root, depth + 1);
        }
    }
    if let Some(branches) = value.get("oneOf").and_then(Value::as_array) {
        if !disjoint(branches, root) {
            return None;
        }
        // Union-only objects can expand without dropping sibling constraints.
        if value.as_object()?.len() != 1 {
            return None;
        }
        return branches
            .iter()
            .map(|branch| alternatives(branch, root, depth + 1))
            .collect::<Option<Vec<_>>>()
            .map(|parts| parts.into_iter().flatten().collect());
    }
    tags(value, root, depth)?;
    Some(vec![value.clone()])
}

fn tags(value: &Value, root: &Value, depth: usize) -> Option<BTreeSet<String>> {
    if depth > 32 {
        return None;
    }
    if value == &Value::Bool(false) {
        return Some(BTreeSet::new());
    }
    if let Some(reference) = value.get("$ref").and_then(Value::as_str) {
        return tags(root.pointer(reference.strip_prefix('#')?)?, root, depth + 1);
    }
    if let Some(branches) = value
        .get("oneOf")
        .or_else(|| value.get("anyOf"))
        .and_then(Value::as_array)
    {
        let mut values = BTreeSet::new();
        for branch in branches {
            values.extend(tags(branch, root, depth + 1)?);
        }
        return Some(values);
    }
    if value.get("type").and_then(Value::as_str) != Some("object") {
        return None;
    }
    if !value
        .get("required")?
        .as_array()?
        .iter()
        .any(|key| key == "kind")
    {
        return None;
    }
    let tag = value.get("properties")?.get("kind")?;
    if let Some(constant) = tag.get("const").and_then(Value::as_str) {
        return Some(BTreeSet::from([constant.to_owned()]));
    }
    tag.get("enum")?
        .as_array()?
        .iter()
        .map(|value| value.as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    fn branch(kind: &str) -> serde_json::Value {
        json!({"type":"object","required":["kind"],"properties":{"kind":{"const":kind}}})
    }
    #[test]
    fn disjoint_required_tags_are_exclusive_without_changing_branches() {
        let branches = json!([branch("common"), branch("read")]);
        let mut schema = json!({"anyOf":branches});
        let root = schema.clone();
        super::normalize(&mut schema, &root);
        assert_eq!(schema, json!({"oneOf":branches}));
    }
    #[test]
    fn identical_shared_tag_branches_can_be_deduplicated() {
        let common = branch("common");
        let handler = branch("handler");
        let shared = branch("uncertain_write");
        let mut schema = json!({"anyOf":[{"oneOf":[common,shared]},{"oneOf":[handler,shared]}]});
        let root = schema.clone();
        super::normalize(&mut schema, &root);
        let actual = schema["oneOf"]
            .as_array()
            .expect("exclusive deduplicated union");
        assert_eq!(actual.len(), 3);
        for expected in [common, handler, shared] {
            assert!(actual.contains(&expected));
        }
    }
    #[test]
    fn tags_without_object_type_do_not_prove_exclusivity() {
        let mut first = branch("first");
        let mut second = branch("second");
        first.as_object_mut().unwrap().remove("type");
        second.as_object_mut().unwrap().remove("type");
        let original = json!({"anyOf":[first,second]});
        for branch in original["anyOf"].as_array().unwrap() {
            assert!(jsonschema::JSONSchema::compile(branch)
                .unwrap()
                .is_valid(&json!(42)));
        }
        let mut schema = original.clone();
        super::normalize(&mut schema, &original);
        assert_eq!(schema, original);
    }
    #[test]
    fn different_overlapping_shapes_keep_the_original_union_semantics() {
        let mut second = branch("common");
        second["properties"]["detail"] = json!({"type":"string"});
        let original = json!({"anyOf":[branch("common"),second]});
        let mut schema = original.clone();
        super::normalize(&mut schema, &original);
        assert_eq!(schema, original);
    }
}
