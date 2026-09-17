#![allow(clippy::literal_string_with_formatting_args)]

use super::{
    schema::{self, Definition, HttpMethod, Parameter, ResponseKind},
    ArgumentAlias, BodyBinding, HandlerBinding, HeaderBinding, NullClearBinding, ParentBinding,
    PathBinding, QueryRequestBinding, QueryRoute, Registration, ResponseBinding, ResponseSelection,
    SelectorBinding,
};
use schemars::generate::Contract;
use serde_json::{json, Value};

#[allow(clippy::too_many_arguments)]
fn backed(
    name: &'static str,
    operation_id: &'static str,
    method: HttpMethod,
    path: &'static str,
    description: &'static str,
    backing: &'static str,
    kind: ResponseKind,
    parameters: Vec<Parameter>,
) -> Definition {
    let raw = schema::raw_for(backing);
    let inject_scope = raw.request_schema["properties"].get("scope_id").is_some();
    let mut registration = Registration::new(backing, raw.context, kind);
    registration.request.path = parameters
        .iter()
        .filter(|parameter| parameter.location == "path")
        .map(|parameter| PathBinding {
            parameter: parameter.name,
            field: parameter.name,
        })
        .collect();
    registration.request.query = parameters
        .iter()
        .filter(|parameter| parameter.location == "query")
        .cloned()
        .collect();
    registration.request.scope_field = inject_scope.then_some("scope_id");
    Definition {
        name,
        operation_id,
        method,
        path,
        description,
        mutates: raw.mutates,
        http_statuses: raw.http_statuses,
        parameters,
        request_schema: (!matches!(method, HttpMethod::Get))
            .then(|| schema::request_envelope(raw.request_schema)),
        success_schema: schema::response_envelope(raw.success_schema, kind),
        failure_schema: raw.failure_schema,
        registration,
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
    let mut registration = Registration::new(backing, raw.context, kind);
    registration.request.path = parameters
        .iter()
        .filter(|parameter| parameter.location == "path")
        .map(|parameter| PathBinding {
            parameter: parameter.name,
            field: parameter.name,
        })
        .collect();
    registration.request.query = parameters
        .iter()
        .filter(|parameter| parameter.location == "query")
        .cloned()
        .collect();
    Definition {
        name,
        operation_id,
        method: HttpMethod::Get,
        path,
        description,
        mutates: false,
        http_statuses: raw.http_statuses,
        parameters,
        request_schema: None,
        success_schema: schema::response_envelope(payload, kind),
        failure_schema: raw.failure_schema,
        registration,
    }
}

impl Definition {
    fn path_field(mut self, parameter: &'static str, field: &'static str) -> Self {
        if let Some(binding) = self
            .registration
            .request
            .path
            .iter_mut()
            .find(|binding| binding.parameter == parameter)
        {
            binding.field = field;
        }
        self
    }

    fn header(mut self, name: &'static str, field: &'static str, trim_quotes: bool) -> Self {
        self.parameters.push(schema::header(name));
        self.registration.controls.headers.push(HeaderBinding {
            name,
            field,
            trim_quotes,
            numeric: false,
        });
        self
    }

    fn numeric_header(mut self, name: &'static str, field: &'static str) -> Self {
        self.parameters.push(schema::header(name));
        self.registration.controls.headers.push(HeaderBinding {
            name,
            field,
            trim_quotes: true,
            numeric: true,
        });
        self
    }

    const fn body(mut self, body: BodyBinding) -> Self {
        self.registration.request.body = body;
        self
    }

    const fn parent(mut self, kind: &'static str) -> Self {
        self.registration.request.parent = Some(ParentBinding {
            kind,
            id_parameter: "id",
            field: "parent",
        });
        self
    }

    const fn selector(mut self, selector: SelectorBinding) -> Self {
        self.registration.request.selector = Some(selector);
        self
    }

    fn null_clears(mut self, fields: &[(&'static str, &'static str)]) -> Self {
        self.registration.request.null_clears = fields
            .iter()
            .map(|(field, clear_name)| NullClearBinding { field, clear_name })
            .collect();
        self
    }

    const fn pagination(mut self) -> Self {
        self.registration.controls.pagination = true;
        self
    }

    const fn with_etag(mut self) -> Self {
        self.registration.controls.returns_etag = true;
        self
    }

    const fn response_selection(mut self, selection: ResponseSelection) -> Self {
        self.registration.response.selection = selection;
        self
    }

    const fn items_field(mut self, field: &'static str) -> Self {
        self.registration.response.items_field = Some(field);
        self
    }
}

fn list_parameters(searchable: bool, rule: bool) -> Vec<Parameter> {
    if !searchable {
        return Vec::new();
    }
    let queries = if rule {
        vec!["search", "stale", "resolve-symbol"]
    } else {
        vec!["search"]
    };
    let mut parameters = vec![
        schema::query("query", json!({"type":"string","enum":queries})),
        schema::query("text", json!({"type":"string"})),
        schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200})),
        schema::query("cursor", json!({"type":"string"})),
    ];
    if rule {
        parameters.extend([
            schema::query("base", json!({"type":"string"})),
            schema::query("head", json!({"type":"string"})),
            schema::query("symbol", json!({"type":"string"})),
            schema::query("file", json!({"type":"string"})),
            schema::query("line", json!({"type":"integer","minimum":1})),
        ]);
    }
    parameters
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

fn with_query_results(
    mut definition: Definition,
    queries: &[(&'static str, ResponseKind)],
    node_type: &'static str,
) -> Definition {
    if queries.is_empty() {
        return definition;
    }
    if definition.registration.response.kind == ResponseKind::Items {
        definition
            .registration
            .request
            .query
            .retain(|parameter| matches!(parameter.name, "limit" | "cursor" | "rule"));
    } else {
        definition.registration.request.query.clear();
    }
    let mut variants = vec![definition.success_schema.clone()];
    let mut defs = serde_json::Map::new();
    for schema in &mut variants {
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
        let request = QueryRequestBinding {
            node_type: matches!(*backing, "search" | "trace" | "neighbors" | "impact")
                .then_some(node_type),
            node_types: *backing == "search",
        };
        let parameters = query::parameters(
            &raw.request_schema,
            &definition.registration.request,
            &request,
        );
        definition.registration.queries.push(QueryRoute {
            name: backing,
            handler: HandlerBinding {
                operation: backing,
                context: raw.context,
            },
            parameters,
            request,
            response: ResponseBinding {
                kind: *kind,
                selection: ResponseSelection::Direct,
                items_field: match *backing {
                    "search" => Some("nodes"),
                    "stale" => Some("sites"),
                    "resolve-symbol" => Some("rules"),
                    _ => None,
                },
            },
        });
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
        let verification = matches!($plural, "verification-runs" | "verification-bindings");
        let mut parameters = list_parameters(searchable, $plural == "rules");
        if verification {
            parameters.push(schema::query(
                "rule",
                json!({"type":"string","minLength":1}),
            ));
        }
        let backing = if verification {
            $plural
        } else if searchable {
            concat!("page-", $plural, "-v2")
        } else {
            concat!("list-", $plural)
        };
        let mut list = read::<$ty>(
            concat!("list-", $plural),
            concat!("list", $plural_id),
            concat!("/", $plural),
            concat!("List ", $plural, " in the bound scope."),
            backing,
            ResponseKind::Items,
            parameters,
        );
        if backing.starts_with("list-") {
            list.registration.request.body = BodyBinding::Null;
        } else {
            list.registration.response.items_field = Some("items");
            list.registration.controls.pagination = true;
        }
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
        $out.push(with_query_results(list, &list_queries, $singular));
        let member = read::<$ty>(
            concat!("get-", $singular),
            concat!("get", $singular_id),
            concat!("/", $plural, "/{id}"),
            concat!("Read one ", $singular, " in the bound scope."),
            concat!("list-", $plural),
            ResponseKind::Resource,
            member_parameters(searchable),
        )
        .body(BodyBinding::Null)
        .response_selection(ResponseSelection::ArrayMember {
            id_parameter: "id",
            owner_parameter: None,
        });
        let member_queries = if searchable {
            vec![
                ("trace", ResponseKind::Result),
                ("neighbors", ResponseKind::Result),
                ("impact", ResponseKind::Result),
            ]
        } else {
            Vec::new()
        };
        $out.push(with_query_results(member, &member_queries, $singular));
        if !$create.is_empty() {
            $out.push(backed(
                concat!("create-", $singular),
                concat!("create", $singular_id),
                HttpMethod::Post,
                concat!("/", $plural),
                concat!("Create one ", $singular, " in the bound scope."),
                $create,
                ResponseKind::Resource,
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
                vec![schema::path("id")],
            ));
        }
    }};
}

mod actions;
mod finalize;
mod query;
mod resources;
mod subresources;

pub(super) fn definitions() -> Vec<Definition> {
    let mut definitions = Vec::new();
    resources::register(&mut definitions);
    subresources::register(&mut definitions);
    actions::register(&mut definitions);
    finalize::request_schemas(&mut definitions);
    definitions
}
