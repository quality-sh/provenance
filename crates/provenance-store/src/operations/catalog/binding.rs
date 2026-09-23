use super::{ContextKind, Parameter, ResponseKind};
use provenance_core::NodeType;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct HandlerBinding {
    pub operation: &'static str,
    pub context: ContextKind,
    pub mutates: bool,
    pub http_statuses: Vec<u16>,
    pub failure_schema: Value,
}

#[derive(Clone)]
pub struct PathBinding {
    pub parameter: &'static str,
    pub field: &'static str,
}

#[derive(Clone)]
pub struct ParentBinding {
    pub kind: &'static str,
    pub id_parameter: &'static str,
    pub field: &'static str,
}

#[derive(Clone)]
pub enum SelectorBinding {
    Discussion {
        parameter: &'static str,
        field: &'static str,
    },
    Legacy {
        parameter: &'static str,
        field: &'static str,
    },
}

#[derive(Clone, Copy)]
pub struct RequestAdapter {
    pub object: bool,
    pub adapt: RequestAdapterFn,
}

pub type RequestAdapterFn =
    fn(&RequestBinding, Value, &BTreeMap<String, String>) -> Result<Value, RequestAdapterError>;

#[derive(Clone, Copy, Debug)]
pub struct RequestAdapterError {
    pub field: Option<&'static str>,
}

#[derive(Clone)]
pub struct NullClearBinding {
    pub field: &'static str,
    pub clear_name: &'static str,
}

#[derive(Clone)]
pub struct ArgumentAlias {
    pub argument: &'static str,
    pub field: &'static str,
    pub wrap_array: bool,
}

#[derive(Clone, Copy)]
pub enum CliDefaultValue {
    String(&'static str),
    EmptyArray,
}

#[derive(Clone)]
pub struct CliDefault {
    pub field: &'static str,
    pub value: CliDefaultValue,
}

#[derive(Clone, Default)]
pub struct CliBinding {
    pub defaults: Vec<CliDefault>,
}

#[derive(Clone)]
pub struct RequestBinding {
    /// The public request body projection, if the method carries one.
    pub schema: Option<Value>,
    /// The full deserialize-direction schema of the bound request type. It
    /// keeps every typed field, so parameter schemas derive from it before
    /// the body projection hides bound fields.
    pub raw: Option<Value>,
    pub adapter: RequestAdapter,
    pub path: Vec<PathBinding>,
    pub parent: Option<ParentBinding>,
    pub selector: Option<SelectorBinding>,
    pub scope_field: Option<&'static str>,
    pub parameters: Vec<Parameter>,
    pub null_clears: Vec<NullClearBinding>,
    pub argument_aliases: Vec<ArgumentAlias>,
}

impl Default for RequestBinding {
    fn default() -> Self {
        Self {
            schema: None,
            raw: None,
            adapter: super::routes::request::DIRECT,
            path: Vec::new(),
            parent: None,
            selector: None,
            scope_field: None,
            parameters: Vec::new(),
            null_clears: Vec::new(),
            argument_aliases: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct HeaderBinding {
    pub name: &'static str,
    pub field: &'static str,
    pub trim_quotes: bool,
    pub numeric: bool,
}

#[derive(Clone, Default)]
pub struct Controls {
    pub headers: Vec<HeaderBinding>,
    pub pagination: bool,
    pub etag: Option<EtagBinding>,
}

#[derive(Clone)]
pub struct EtagBinding {
    pub pointer: &'static str,
    pub numeric: bool,
}

#[derive(Clone)]
pub struct ResponseBinding {
    pub kind: ResponseKind,
    pub adapter: ResponseAdapter,
    pub raw_schema: Value,
    pub schema: Value,
}

#[derive(Clone, Copy)]
pub enum ResponseAdapter {
    Direct,
    Result,
    ArrayItems,
    ObjectItems(&'static str),
    ResultItems(&'static str),
}

impl ResponseBinding {
    pub const fn direct(kind: ResponseKind, raw_schema: Value, schema: Value) -> Self {
        Self {
            kind,
            adapter: match kind {
                ResponseKind::Resource | ResponseKind::Result => ResponseAdapter::Direct,
                ResponseKind::Items => ResponseAdapter::ArrayItems,
            },
            raw_schema,
            schema,
        }
    }
}

#[derive(Clone)]
pub struct QueryRequestBinding {
    pub node_type: Option<&'static str>,
    pub node_types: bool,
    pub adapter: RequestAdapter,
}

impl Default for QueryRequestBinding {
    fn default() -> Self {
        Self {
            node_type: None,
            node_types: false,
            adapter: super::routes::request::DIRECT,
        }
    }
}

#[derive(Clone)]
pub struct QueryRoute {
    pub name: &'static str,
    pub handler: HandlerBinding,
    pub parameters: Vec<Parameter>,
    pub request: QueryRequestBinding,
    pub response: ResponseBinding,
}

pub use provenance_core::TargetAction;

/// The porcelain action and record kind owned by one registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetBinding {
    pub action: TargetAction,
    pub kind: NodeType,
}

#[derive(Clone)]
pub struct Registration {
    pub handler: HandlerBinding,
    pub request: RequestBinding,
    pub controls: Controls,
    pub response: ResponseBinding,
    pub queries: Vec<QueryRoute>,
    pub cli: CliBinding,
    pub target: Option<TargetBinding>,
}

impl Registration {
    pub fn new(
        handler: HandlerBinding,
        request_schema: Option<Value>,
        response: ResponseBinding,
    ) -> Self {
        Self {
            handler,
            request: RequestBinding {
                schema: request_schema,
                ..RequestBinding::default()
            },
            controls: Controls::default(),
            response,
            queries: Vec::new(),
            cli: CliBinding::default(),
            target: None,
        }
    }
}
