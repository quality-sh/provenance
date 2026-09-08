// Generated from OpenAPI. Do not edit.
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
pub struct NeighborsSuccessOutputNeighbor {
    pub direction: NeighborsSuccessOutputDirection,
    pub node: NeighborsSuccessOutputGraphNode,
    pub relation: ::std::string::String,
}
