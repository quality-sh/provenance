use super::{ContextKind, Parameter, ResponseKind};

#[derive(Clone)]
pub struct HandlerBinding {
    pub operation: &'static str,
    pub context: ContextKind,
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

#[derive(Clone, Copy, Default)]
pub enum BodyBinding {
    #[default]
    Direct,
    Null,
    DiscussionStart,
    DiscussionReply,
    DiscussionStatus,
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
}

#[derive(Clone, Default)]
pub struct RequestBinding {
    pub body: BodyBinding,
    pub path: Vec<PathBinding>,
    pub parent: Option<ParentBinding>,
    pub selector: Option<SelectorBinding>,
    pub scope_field: Option<&'static str>,
    pub query: Vec<Parameter>,
    pub null_clears: Vec<NullClearBinding>,
    pub argument_aliases: Vec<ArgumentAlias>,
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
    pub returns_etag: bool,
}

#[derive(Clone)]
pub enum ResponseSelection {
    Direct,
    ArrayItems {
        owner_parameter: &'static str,
        owner_field: &'static str,
    },
    ArrayMember {
        id_parameter: &'static str,
        owner_parameter: Option<(&'static str, &'static str)>,
    },
    PageMember {
        id_parameter: &'static str,
        id_pointer: &'static str,
    },
}

#[derive(Clone)]
pub struct ResponseBinding {
    pub kind: ResponseKind,
    pub selection: ResponseSelection,
    pub items_field: Option<&'static str>,
}

impl ResponseBinding {
    pub const fn direct(kind: ResponseKind) -> Self {
        Self {
            kind,
            selection: ResponseSelection::Direct,
            items_field: None,
        }
    }
}

#[derive(Clone, Default)]
pub struct QueryRequestBinding {
    pub node_type: Option<&'static str>,
    pub node_types: bool,
}

#[derive(Clone)]
pub struct QueryRoute {
    pub name: &'static str,
    pub handler: HandlerBinding,
    pub parameters: Vec<Parameter>,
    pub request: QueryRequestBinding,
    pub response: ResponseBinding,
}

#[derive(Clone)]
pub struct Registration {
    pub handler: HandlerBinding,
    pub request: RequestBinding,
    pub controls: Controls,
    pub response: ResponseBinding,
    pub queries: Vec<QueryRoute>,
}

impl Registration {
    pub fn new(operation: &'static str, context: ContextKind, response: ResponseKind) -> Self {
        Self {
            handler: HandlerBinding { operation, context },
            request: RequestBinding::default(),
            controls: Controls::default(),
            response: ResponseBinding::direct(response),
            queries: Vec::new(),
        }
    }
}
