//! The operation wire contract: routes, wire schemas, HTTP, MCP and CLI bindings, and
//! dispatch by operation name. The typed operations stay in `provenance-store`.

mod binding;
mod dispatch;
mod routes;
mod schema;
mod schema_page;
mod schema_values;
#[cfg(test)]
mod tests;

use provenance_store::operations::catalog::{ContextKind, Operation};

pub use binding::{
    ArgumentAlias, CliBinding, CliDefault, CliDefaultValue, CliExecution, Controls,
    DiscussionWriteKind, EtagBinding, FixedBinding, HandlerBinding, HeaderBinding,
    NullClearBinding, ParentBinding, PathBinding, QueryRequestBinding, QueryRoute, Registration,
    RequestAdapter, RequestAdapterError, RequestBinding, ResponseAdapter, ResponseBinding,
    SelectorBinding, TargetAction, TargetBinding,
};
pub use dispatch::{contains, invoke_with, mutates};
pub use schema::{
    cli_name, definitions, json_flag, operation_request_schema, operation_success_schema,
    parse_parameter_value, parse_schema_value, parse_schema_value_in, serialize_parameter_value,
    target_definition, target_definitions, Definition, HttpMethod, Parameter, ParseValueError,
    QueryVariant, ResponseKind,
};
