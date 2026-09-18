#![allow(clippy::literal_string_with_formatting_args)]

use super::{
    schema::{self, Definition, HttpMethod, Parameter, ResponseKind},
    ArgumentAlias, CliDefault, CliDefaultValue, EtagBinding, HandlerBinding, HeaderBinding,
    NullClearBinding, Operation, ParentBinding, PathBinding, QueryRequestBinding, QueryRoute,
    Registration, RequestAdapter, ResponseAdapter, ResponseBinding, SelectorBinding,
};
use schemars::generate::Contract;
use serde_json::{json, Value};

#[allow(clippy::too_many_arguments)]
fn backed<O: Operation>(
    name: &'static str,
    operation_id: &'static str,
    method: HttpMethod,
    path: &'static str,
    description: &'static str,
    kind: ResponseKind,
    parameters: Vec<Parameter>,
) -> Definition {
    let raw = schema::raw_definition::<O>();
    let response = response_binding(raw.success_schema.clone(), kind);
    let request_schema = (!matches!(method, HttpMethod::Get))
        .then(|| schema::request_envelope(raw.request_schema.clone()));
    let mut registration = Registration::new(
        HandlerBinding {
            operation: O::NAME,
            context: raw.context,
            mutates: raw.mutates,
            http_statuses: raw.http_statuses,
            failure_schema: raw.failure_schema,
        },
        request_schema,
        response,
    );
    registration.request.path = parameters
        .iter()
        .filter(|parameter| parameter.location == "path")
        .map(|parameter| PathBinding {
            parameter: parameter.name,
            field: parameter.name,
        })
        .collect();
    registration.request.parameters = parameters;
    Definition {
        name,
        operation_id,
        method,
        path,
        description,
        registration,
    }
}

fn read<T: schemars::JsonSchema, O: Operation>(
    name: &'static str,
    operation_id: &'static str,
    path: &'static str,
    description: &'static str,
    kind: ResponseKind,
    parameters: Vec<Parameter>,
) -> Definition {
    let raw = schema::raw_definition::<O>();
    let raw_response = raw.success_schema.clone();
    let payload = match kind {
        ResponseKind::Items => schema::type_schema::<Vec<T>>(Contract::Serialize),
        _ => schema::type_schema::<T>(Contract::Serialize),
    };
    let mut registration = Registration::new(
        HandlerBinding {
            operation: O::NAME,
            context: raw.context,
            mutates: raw.mutates,
            http_statuses: raw.http_statuses,
            failure_schema: raw.failure_schema,
        },
        None,
        ResponseBinding::direct(kind, raw_response, schema::response_envelope(payload, kind)),
    );
    registration.request.path = parameters
        .iter()
        .filter(|parameter| parameter.location == "path")
        .map(|parameter| PathBinding {
            parameter: parameter.name,
            field: parameter.name,
        })
        .collect();
    registration.request.parameters = parameters;
    Definition {
        name,
        operation_id,
        method: HttpMethod::Get,
        path,
        description,
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
        self.registration.controls.headers.push(HeaderBinding {
            name,
            field,
            trim_quotes,
            numeric: false,
        });
        self
    }

    fn numeric_header(mut self, name: &'static str, field: &'static str) -> Self {
        self.registration.controls.headers.push(HeaderBinding {
            name,
            field,
            trim_quotes: true,
            numeric: true,
        });
        self
    }

    const fn adapter(mut self, adapter: RequestAdapter) -> Self {
        self.registration.request.adapter = adapter;
        self
    }

    const fn scope(mut self, field: &'static str) -> Self {
        self.registration.request.scope_field = Some(field);
        self
    }

    fn parent(mut self, kind: &'static str) -> Self {
        self.registration
            .request
            .path
            .retain(|binding| binding.parameter != "id");
        self.registration.request.parent = Some(ParentBinding {
            kind,
            id_parameter: "id",
            field: "parent",
        });
        self
    }

    fn selector(mut self, selector: SelectorBinding) -> Self {
        let parameter = match &selector {
            SelectorBinding::Discussion { parameter, .. }
            | SelectorBinding::Legacy { parameter, .. } => *parameter,
        };
        self.registration
            .request
            .path
            .retain(|binding| binding.parameter != parameter);
        self.registration.request.selector = Some(selector);
        self
    }

    fn null_clears(mut self, fields: &[(&'static str, &'static str)]) -> Self {
        self.registration.request.null_clears = fields
            .iter()
            .map(|(field, clear_name)| NullClearBinding { field, clear_name })
            .collect();
        if !fields.is_empty() {
            self.registration.request.adapter = request::NULLABLE_PATCH;
        }
        self
    }

    const fn pagination(mut self) -> Self {
        self.registration.controls.pagination = true;
        self
    }

    const fn with_etag(mut self, pointer: &'static str, numeric: bool) -> Self {
        self.registration.controls.etag = Some(EtagBinding { pointer, numeric });
        self
    }

    fn items_field(mut self, field: &'static str) -> Self {
        self.registration.response.adapter = ResponseAdapter::ResultItems(field);
        let payload =
            schema::property_schema(&self.registration.response.raw_schema, &["result", field]);
        self.registration.response.schema = schema::response_envelope(payload, ResponseKind::Items);
        self
    }

    fn result(mut self) -> Self {
        self.registration.response.adapter = ResponseAdapter::Result;
        let payload = schema::property_schema(&self.registration.response.raw_schema, &["result"]);
        self.registration.response.schema =
            schema::response_envelope(payload, self.registration.response.kind);
        self
    }

    fn cli_default(mut self, field: &'static str, value: CliDefaultValue) -> Self {
        self.registration
            .cli
            .defaults
            .push(CliDefault { field, value });
        self
    }

    fn cli_defaults(mut self, defaults: &[CliDefault]) -> Self {
        self.registration.cli.defaults.extend_from_slice(defaults);
        self
    }

    fn argument_aliases(mut self, aliases: &[ArgumentAlias]) -> Self {
        self.registration
            .request
            .argument_aliases
            .extend_from_slice(aliases);
        self
    }
}

fn response_binding(raw_schema: Value, kind: ResponseKind) -> ResponseBinding {
    let payload = raw_schema.clone();
    ResponseBinding::direct(kind, raw_schema, schema::response_envelope(payload, kind))
}

fn list_parameters(searchable: bool, rule: bool) -> Vec<Parameter> {
    let mut parameters = vec![
        schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200})),
        schema::query("cursor", json!({"type":"string"})),
    ];
    if !searchable {
        return parameters;
    }
    let queries = if rule {
        vec!["search", "stale", "resolve-symbol"]
    } else {
        vec!["search"]
    };
    parameters.splice(
        0..0,
        [
            schema::query("query", json!({"type":"string","enum":queries})),
            schema::query("text", json!({"type":"string"})),
        ],
    );
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

fn with_query_results(mut definition: Definition, queries: Vec<QueryRoute>) -> Definition {
    if queries.is_empty() {
        return definition;
    }
    if definition.registration.response.kind == ResponseKind::Items {
        definition
            .registration
            .request
            .parameters
            .retain(|parameter| {
                parameter.location == "path"
                    || matches!(parameter.name, "limit" | "cursor" | "rule")
            });
    } else {
        definition
            .registration
            .request
            .parameters
            .retain(|parameter| parameter.location == "path");
    }
    definition.registration.queries = queries;
    definition
}

fn query_route<O: Operation>(
    definition: &Definition,
    kind: ResponseKind,
    node_type: Option<&'static str>,
    node_types: bool,
    adapter: ResponseAdapter,
    payload_path: &[&str],
) -> QueryRoute {
    let raw = schema::raw_definition::<O>();
    let request = QueryRequestBinding {
        node_type,
        node_types,
        adapter: request::DIRECT,
    };
    let parameters = query::parameters(
        &raw.request_schema,
        &definition.registration.request,
        &request,
    );
    let payload = if payload_path.is_empty() {
        raw.success_schema.clone()
    } else {
        schema::property_schema(&raw.success_schema, payload_path)
    };
    QueryRoute {
        name: O::NAME,
        handler: HandlerBinding {
            operation: O::NAME,
            context: raw.context,
            mutates: raw.mutates,
            http_statuses: raw.http_statuses,
            failure_schema: raw.failure_schema,
        },
        parameters,
        request,
        response: ResponseBinding {
            kind,
            adapter,
            raw_schema: raw.success_schema,
            schema: schema::response_envelope(payload, kind),
        },
    }
}

fn searchable_queries(definition: &Definition, node_type: &'static str) -> Vec<QueryRoute> {
    vec![query_route::<super::Search>(
        definition,
        ResponseKind::Items,
        Some(node_type),
        true,
        ResponseAdapter::ObjectItems("nodes"),
        &["nodes"],
    )]
}

fn member_queries(definition: &Definition, node_type: &'static str) -> Vec<QueryRoute> {
    vec![
        query_route::<super::Trace>(
            definition,
            ResponseKind::Result,
            Some(node_type),
            false,
            ResponseAdapter::Direct,
            &[],
        ),
        query_route::<super::Neighbors>(
            definition,
            ResponseKind::Result,
            Some(node_type),
            false,
            ResponseAdapter::Direct,
            &[],
        ),
        query_route::<super::Impact>(
            definition,
            ResponseKind::Result,
            Some(node_type),
            false,
            ResponseAdapter::Direct,
            &[],
        ),
    ]
}

mod actions;
mod finalize;
mod query;
pub(super) mod request;
#[macro_use]
mod resource;
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
