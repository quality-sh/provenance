// Generated from OpenAPI. Do not edit.
pub mod defaults {
    pub(super) fn default_nzu64<T, const V: u64>() -> T
    where
        T: ::std::convert::TryFrom<::std::num::NonZeroU64>,
        <T as ::std::convert::TryFrom<::std::num::NonZeroU64>>::Error: ::std::fmt::Debug,
    {
        T::try_from(::std::num::NonZeroU64::try_from(V).unwrap()).unwrap()
    }
    pub(super) fn neighbors_request_input_neighbors_query_direction() -> super::NeighborsRequestInputDirection {
        super::NeighborsRequestInputDirection::Both
    }
    pub(super) fn trace_request_input_trace_query_direction() -> super::TraceRequestInputDirection {
        super::TraceRequestInputDirection::Both
    }
}
