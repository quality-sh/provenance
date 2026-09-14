use super::schema::{self, Definition, HttpMethod, Parameter, ResponseKind};
use schemars::generate::Contract;
use serde_json::{json, Value};

fn backed(
    name: &'static str,
    operation_id: &'static str,
    method: HttpMethod,
    path: &'static str,
    description: &'static str,
    backing: &'static str,
    kind: ResponseKind,
    strip: &[&str],
    mut parameters: Vec<Parameter>,
) -> Definition {
    let raw = schema::raw_for(backing);
    if raw.mutates && matches!(method, HttpMethod::Post | HttpMethod::Patch) {
        if name.starts_with("create-requirement")
            || name.starts_with("update-requirement")
            || name.contains("discussion")
            || name.contains("requirement-review")
        {
            parameters.push(schema::header("Idempotency-Key"));
        }
        if name.starts_with("update-requirement")
            || name.starts_with("update-discussion")
            || name.ends_with("create-discussion-message")
        {
            parameters.push(schema::header("If-Match"));
        }
    }
    Definition {
        name,
        operation_id,
        method,
        path,
        description,
        backing,
        context: raw.context,
        inject_scope: strip.contains(&"scope_id"),
        mutates: raw.mutates,
        http_statuses: raw.http_statuses,
        parameters,
        request_schema: (!matches!(method, HttpMethod::Get))
            .then(|| schema::request_envelope(raw.request_schema, strip)),
        success_schema: schema::response_envelope(raw.success_schema, kind),
        failure_schema: raw.failure_schema,
        response_kind: kind,
    }
}

fn read<T: schemars::JsonSchema>(
    name: &'static str,
    operation_id: &'static str,
    path: &'static str,
    description: &'static str,
    backing: &'static str,
    kind: ResponseKind,
    parameters: Vec<Parameter>,
) -> Definition {
    let raw = schema::raw_for(backing);
    let payload = match kind {
        ResponseKind::Items => schema::type_schema::<Vec<T>>(Contract::Serialize),
        _ => schema::type_schema::<T>(Contract::Serialize),
    };
    Definition {
        name,
        operation_id,
        method: HttpMethod::Get,
        path,
        description,
        backing,
        context: raw.context,
        inject_scope: false,
        mutates: false,
        http_statuses: raw.http_statuses,
        parameters,
        request_schema: None,
        success_schema: schema::response_envelope(payload, kind),
        failure_schema: raw.failure_schema,
        response_kind: kind,
    }
}

fn list_parameters(searchable: bool, rule: bool) -> Vec<Parameter> {
    if !searchable {
        return Vec::new();
    }
    let queries = if rule {
        vec!["search", "stale", "resolve-symbol"]
    } else if searchable {
        vec!["search"]
    } else {
        Vec::new()
    };
    vec![
        schema::query("query", json!({"type":"string","enum":queries})),
        schema::query("text", json!({"type":"string"})),
        schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200})),
        schema::query("cursor", json!({"type":"string"})),
        schema::query("base", json!({"type":"string"})),
        schema::query("head", json!({"type":"string"})),
        schema::query("symbol", json!({"type":"string"})),
        schema::query("file", json!({"type":"string"})),
        schema::query("line", json!({"type":"integer","minimum":1})),
    ]
}

fn member_parameters(searchable: bool) -> Vec<Parameter> {
    let mut parameters = vec![schema::path("id")];
    if searchable {
        parameters.extend([
            schema::query(
                "query",
                json!({"type":"string","enum":["trace","neighbors","impact"]}),
            ),
            schema::query(
                "direction",
                json!({"type":"string","enum":["in","out","both"]}),
            ),
            schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200})),
            schema::query(
                "max_depth",
                json!({"type":"integer","minimum":1,"maximum":10}),
            ),
        ]);
    }
    parameters
}

fn with_query_results(mut definition: Definition, queries: &[(&str, ResponseKind)]) -> Definition {
    if queries.is_empty() {
        return definition;
    }
    let mut variants = vec![definition.success_schema.clone()];
    let mut defs = serde_json::Map::new();
    for schema in variants.iter_mut() {
        if let Some(Value::Object(found)) = schema
            .as_object_mut()
            .and_then(|object| object.remove("$defs"))
        {
            defs.extend(found);
        }
    }
    for (backing, kind) in queries {
        let raw = schema::raw_for(backing);
        let mut response = schema::response_envelope(raw.success_schema, *kind);
        let prefix = backing
            .split('-')
            .map(|part| {
                let mut chars = part.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_uppercase().chain(chars).collect()
                })
            })
            .collect::<String>();
        schema::namespace_defs(&mut response, &prefix);
        if let Some(Value::Object(found)) = response
            .as_object_mut()
            .and_then(|object| object.remove("$defs"))
        {
            for (name, value) in found {
                defs.entry(name).or_insert(value);
            }
        }
        variants.push(response);
    }
    // The ordinary list and a query can have the same JSON shape. Both are
    // valid results for this route, so validation must accept overlap.
    definition.success_schema = json!({"anyOf":variants});
    if !defs.is_empty() {
        definition.success_schema["$defs"] = Value::Object(defs);
    }
    definition
}

macro_rules! resource {
    ($out:ident, $ty:ty, $plural:literal, $singular:literal, $singular_id:literal, $plural_id:literal, $create:literal, $update:literal) => {{
        let searchable = matches!(
            $plural,
            "sources"
                | "requirements"
                | "resolutions"
                | "rules"
                | "domains"
                | "boundaries"
                | "topics"
                | "questions"
        );
        let list = read::<$ty>(
            concat!("list-", $plural),
            concat!("list", $plural_id),
            concat!("/", $plural),
            concat!("List ", $plural, " in the bound scope."),
            concat!("list-", $plural),
            ResponseKind::Items,
            list_parameters(searchable, $plural == "rules"),
        );
        let list_queries = if $plural == "rules" {
            vec![
                ("search", ResponseKind::Items),
                ("stale", ResponseKind::Items),
                ("resolve-symbol", ResponseKind::Items),
            ]
        } else if searchable {
            vec![("search", ResponseKind::Items)]
        } else {
            Vec::new()
        };
        $out.push(with_query_results(list, &list_queries));
        let member = read::<$ty>(
            concat!("get-", $singular),
            concat!("get", $singular_id),
            concat!("/", $plural, "/{id}"),
            concat!("Read one ", $singular, " in the bound scope."),
            concat!("list-", $plural),
            ResponseKind::Resource,
            member_parameters(searchable),
        );
        let member_queries = if searchable {
            vec![
                ("trace", ResponseKind::Result),
                ("neighbors", ResponseKind::Result),
                ("impact", ResponseKind::Result),
            ]
        } else {
            Vec::new()
        };
        $out.push(with_query_results(member, &member_queries));
        if !$create.is_empty() {
            $out.push(backed(
                concat!("create-", $singular),
                concat!("create", $singular_id),
                HttpMethod::Post,
                concat!("/", $plural),
                concat!("Create one ", $singular, " in the bound scope."),
                $create,
                ResponseKind::Resource,
                &["scope_id"],
                Vec::new(),
            ));
        }
        if !$update.is_empty() {
            $out.push(backed(
                concat!("update-", $singular),
                concat!("update", $singular_id),
                HttpMethod::Patch,
                concat!("/", $plural, "/{id}"),
                concat!("Apply a partial change to one ", $singular, "."),
                $update,
                ResponseKind::Resource,
                &["scope_id", "id"],
                vec![schema::path("id")],
            ));
        }
    }};
}

mod actions;
mod resources;
mod subresources;

pub(super) fn definitions() -> Vec<Definition> {
    let mut definitions = Vec::new();
    resources::register(&mut definitions);
    subresources::register(&mut definitions);
    actions::register(&mut definitions);
    definitions
}
