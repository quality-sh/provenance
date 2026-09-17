use super::{schema, Definition, SelectorBinding};

pub(super) fn request_schemas(definitions: &mut [Definition]) {
    for definition in definitions {
        let registration = &definition.registration;
        let mut fields = registration
            .request
            .path
            .iter()
            .map(|binding| binding.field)
            .collect::<Vec<_>>();
        fields.extend(registration.request.scope_field);
        fields.extend(
            registration
                .request
                .parent
                .iter()
                .map(|binding| binding.field),
        );
        fields.extend(
            registration
                .request
                .selector
                .iter()
                .map(|binding| match binding {
                    SelectorBinding::Discussion { field, .. }
                    | SelectorBinding::Legacy { field, .. } => *field,
                }),
        );
        fields.extend(
            registration
                .controls
                .headers
                .iter()
                .map(|binding| binding.field),
        );
        fields.push("action");
        fields.sort_unstable();
        fields.dedup();
        for field in fields {
            schema::hide_bound_request_field(&mut definition.registration.request.schema, field);
        }
    }
}
