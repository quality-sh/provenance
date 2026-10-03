#![allow(clippy::too_many_lines, clippy::wildcard_imports)]

use super::*;
use crate::operations::catalog::{
    resource_members as members, resource_pages as pages, verification_resources as verification,
};

const NONE: &[CliDefault] = &[];
const NO_ALIASES: &[ArgumentAlias] = &[];
const CREATE_REQUIREMENT_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "actor",
        value: CliDefaultValue::String("cli"),
    },
    CliDefault {
        field: "status",
        value: CliDefaultValue::String("active"),
    },
    CliDefault {
        field: "depends_on",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "supersedes",
        value: CliDefaultValue::EmptyArray,
    },
];
const UPDATE_REQUIREMENT_DEFAULTS: &[CliDefault] = &[CliDefault {
    field: "actor",
    value: CliDefaultValue::String("cli"),
}];
const CREATE_SOURCE_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "source_type",
        value: CliDefaultValue::String("policy"),
    },
    CliDefault {
        field: "supersedes",
        value: CliDefaultValue::EmptyArray,
    },
];
const CREATE_RULE_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "requirement_ids",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "resolution_ids",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "status",
        value: CliDefaultValue::String("active"),
    },
    CliDefault {
        field: "severity",
        value: CliDefaultValue::String("high"),
    },
];
const CREATE_RESOLUTION_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "requirement_ids",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "supersedes",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "inputs",
        value: CliDefaultValue::EmptyArray,
    },
    CliDefault {
        field: "status",
        value: CliDefaultValue::String("proposed"),
    },
];
const OPEN_LINK_DEFAULTS: &[CliDefault] = &[
    CliDefault {
        field: "status",
        value: CliDefaultValue::String("open"),
    },
    CliDefault {
        field: "links",
        value: CliDefaultValue::EmptyArray,
    },
];
const RULE_ALIASES: &[ArgumentAlias] = &[
    ArgumentAlias {
        argument: "requirement_id",
        field: "requirement_ids",
        wrap_array: true,
    },
    ArgumentAlias {
        argument: "resolution_id",
        field: "resolution_ids",
        wrap_array: true,
    },
];
const RESOLUTION_ALIASES: &[ArgumentAlias] = &[ArgumentAlias {
    argument: "requirement_id",
    field: "requirement_ids",
    wrap_array: true,
}];
const QUESTION_ALIASES: &[ArgumentAlias] = &[ArgumentAlias {
    argument: "method",
    field: "resolution_method",
    wrap_array: false,
}];

macro_rules! family_route_definitions {
    ($out:ident, $record:ty, $catalog:tt, none) => {};
    (
        $out:ident,
        $record:ty,
        projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal,
            $member:ident, $member_wire:literal),
        requirements
    ) => {
        requirements(&mut $out);
    };
    (
        $out:ident,
        $record:ty,
        $kind:ident($($catalog:tt)*),
        read {
            mode: $mode:ident,
            plural: $plural:literal,
            singular: $singular:literal,
            singular_id: $singular_id:literal,
            plural_id: $plural_id:literal
        }
    ) => {
        family_read_route!($out, $record, [$($catalog)*], $mode, $plural, $singular,
            $singular_id, $plural_id);
    };
    (
        $out:ident,
        $record:ty,
        $kind:ident($($catalog:tt)*),
        writable {
            mode: $mode:ident,
            plural: $plural:literal,
            singular: $singular:literal,
            singular_id: $singular_id:literal,
            plural_id: $plural_id:literal,
            create: $create:ident,
            update: $update:ident,
            create_defaults: $create_defaults:ident,
            create_aliases: $create_aliases:ident,
            update_defaults: $update_defaults:ident,
            update_aliases: $update_aliases:ident,
            nullable: $nullable:expr,
            target: $target:expr
        }
    ) => {
        family_write_route!($out, $record, [$($catalog)*], $mode, $plural, $singular,
            $singular_id, $plural_id, $create, $update, $create_defaults,
            $create_aliases, $update_defaults, $update_aliases, $nullable, $target);
    };
    (
        $out:ident,
        $record:ty,
        $kind:ident($($catalog:tt)*),
        proposal {
            plural: $plural:literal,
            singular: $singular:literal,
            singular_id: $singular_id:literal,
            plural_id: $plural_id:literal
        }
    ) => {{
        family_proposal_route!($out, $record, [$($catalog)*], $plural, $singular,
            $singular_id, $plural_id);
    }};
}

macro_rules! family_read_route {
    (
        $out:ident, $record:ty,
        [$list:ident, $list_wire:literal, $page:ident, $page_wire:literal,
            $member:ident, $member_wire:literal],
        $mode:ident, $plural:literal, $singular:literal,
        $singular_id:literal, $plural_id:literal
    ) => {
        resource!(
            $out,
            $mode,
            $record,
            pages::$page,
            members::$member,
            $plural,
            $singular,
            $singular_id,
            $plural_id
        );
    };
    (
        $out:ident, $record:ty,
        [$list:ident, $list_wire:literal, $page:ident,
            $member:ident, $member_wire:literal],
        $mode:ident, $plural:literal, $singular:literal,
        $singular_id:literal, $plural_id:literal
    ) => {
        resource!(
            $out,
            $mode,
            $record,
            pages::$page,
            members::$member,
            $plural,
            $singular,
            $singular_id,
            $plural_id
        );
    };
}

macro_rules! family_write_route {
    (
        $out:ident, $record:ty,
        [$list:ident, $list_wire:literal, $page:ident, $page_wire:literal,
            $member:ident, $member_wire:literal],
        $mode:ident, $plural:literal, $singular:literal,
        $singular_id:literal, $plural_id:literal,
        $create:ident, $update:ident,
        $create_defaults:ident, $create_aliases:ident,
        $update_defaults:ident, $update_aliases:ident,
        $nullable:expr, $target:expr
    ) => {
        resource!(
            $out,
            $mode,
            $record,
            pages::$page,
            members::$member,
            $plural,
            $singular,
            $singular_id,
            $plural_id,
            super::super::$create,
            super::super::$update,
            $create_defaults,
            $create_aliases,
            $update_defaults,
            $update_aliases,
            $nullable,
            $target
        );
    };
}

macro_rules! family_proposal_route {
    (
        $out:ident, $record:ty,
        [$list:ident, $list_wire:literal, $page:ident, $page_wire:literal,
            $member:ident, $member_wire:literal],
        $plural:literal, $singular:literal,
        $singular_id:literal, $plural_id:literal
    ) => {{
        resource!(
            $out,
            plain,
            $record,
            pages::$page,
            members::$member,
            $plural,
            $singular,
            $singular_id,
            $plural_id
        );
        $out.push(
            backed::<super::super::CreateProposal>(
                "create-proposal",
                "createProposal",
                HttpMethod::Post,
                "/proposals",
                "Create one proposal in the bound scope.",
                ResponseKind::Resource,
                Vec::new(),
            )
            .scope("scope_id"),
        );
    }};
}

macro_rules! family_route {
    ($out:ident, $variant:ident, $record:ty, [$($catalog:tt)*], [none]) => {};
    (
        $out:ident,
        $variant:ident,
        $record:ty,
        [$($catalog:tt)*],
        [$($route:tt)+]
    ) => {{
        let mut definitions = Vec::new();
        family_route_definitions!(definitions, $record, $($catalog)*, $($route)+);
        let order = crate::cache::ProjectionFamily::$variant
            .meta()
            .route_order
            .expect("registered resource families have a route order");
        $out.push((order, definitions));
    }};
}

macro_rules! review_family_route_definitions {
    (
        $out:ident, $record:ty,
        projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, none),
        requirements
    ) => {
        requirements(&mut $out);
    };
    (
        $out:ident, $record:ty,
        projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal,
            $member:ident, $member_wire:literal),
        requirements
    ) => {
        requirements(&mut $out);
    };
    (
        $out:ident, $record:ty,
        projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal,
            $member:ident, $member_wire:literal),
        writable {
            mode: $mode:ident,
            plural: $plural:literal,
            singular: $singular:literal,
            singular_id: $singular_id:literal,
            plural_id: $plural_id:literal,
            create: $create:ident,
            update: $update:ident,
            create_defaults: $create_defaults:ident,
            create_aliases: $create_aliases:ident,
            update_defaults: $update_defaults:ident,
            update_aliases: $update_aliases:ident,
            nullable: $nullable:expr,
            target: $target:expr
        }
    ) => {
        review_resource!(
            $out,
            $mode,
            $record,
            pages::$page,
            members::$member,
            $plural,
            $singular,
            $singular_id,
            $plural_id,
            super::super::$create,
            super::super::$update,
            $create_defaults,
            $create_aliases,
            $update_defaults,
            $update_aliases,
            $nullable,
            $target
        );
    };
}

macro_rules! register_family_route {
    ($out:ident, $variant:ident, $record:ty, [$($catalog:tt)*], [$($route:tt)*],
        [$review:ident]) => {{
        let mut definitions = Vec::new();
        review_family_route_definitions!(definitions, $record, $($catalog)*, $($route)*);
        let order = crate::cache::ProjectionFamily::$variant
            .meta()
            .route_order
            .expect("registered resource families have a route order");
        $out.push((order, definitions));
    }};
    ($out:ident, $variant:ident, $record:ty, [$($catalog:tt)*], [$($route:tt)*], []) => {
        family_route!($out, $variant, $record, [$($catalog)*], [$($route)*]);
    };
}

macro_rules! register_family_routes {
    (
        $out:ident;
        $(
            $group:ident {
                $(
                    $variant:ident {
                        record: $record:ty,
                        field: $field:ident,
                        path: $path:ident,
                        meta: $meta:tt,
                        node: [$($node:tt)*],
                        reader: {
                            open: $reader:ident,
                            closed: [$($closed:tt)*],
                            strategy: $strategy:ident
                        },
                        id: $id:ident,
                        loader: [$($loader:tt)*],
                        graph: [$($graph:tt)*],
                        import: [$($import:tt)*],
                        catalog: [$($catalog:tt)*],
                        route: [$($route:tt)*]
                        $(, review: $review:ident)?
                    };
                )*
            }
        )*
    ) => {
        $($(register_family_route!(
            $out, $variant, $record, [$($catalog)*], [$($route)*], [$($review)?]
        );)*)*
    };
}

pub(super) fn register(out: &mut Vec<Definition>) {
    let mut families = Vec::new();
    crate::cache::family_table::record_family_rows!(register_family_routes, families);
    let mut verification_runs = Vec::new();
    resource!(
        verification_runs,
        verification,
        provenance_core::VerificationRun,
        verification::PageVerificationRuns,
        verification::GetVerificationRun,
        "verification-runs",
        "verification-run",
        "VerificationRun",
        "VerificationRuns"
    );
    families.push((120, verification_runs));
    families.sort_by_key(|(order, _)| *order);
    for (_, definitions) in families {
        out.extend(definitions);
    }
}

fn requirements(out: &mut Vec<Definition>) {
    let list = read::<provenance_core::Requirement, pages::PageRequirements>(
        "list-requirements",
        "listRequirements",
        "/requirements",
        "List requirements in the bound scope.",
        ResponseKind::Items,
        list_parameters(true, false),
    )
    .items_field("items")
    .pagination();
    let queries = searchable_queries(&list, "requirement");
    out.push(with_query_results(list, queries));
    let member = backed::<members::GetReviewedResource>(
        "get-requirement",
        "getRequirement",
        HttpMethod::Get,
        "/requirements/{id}",
        "Read one Requirement with its edit and decision state.",
        ResponseKind::Resource,
        member_parameters(true),
    )
    .reviewed_result::<provenance_core::Requirement>()
    .fixed("record_kind", NodeType::Requirement.as_str())
    .with_etag("/edit/etag", false);
    let queries = member_queries(&member, "requirement");
    out.push(with_query_results(member, queries));
    out.push(
        backed::<super::super::CreateRequirementResource>(
            "create-requirement",
            "createRequirement",
            HttpMethod::Post,
            "/requirements",
            "Create one Requirement through the guarded review journal.",
            ResponseKind::Resource,
            Vec::new(),
        )
        .cli_defaults(CREATE_REQUIREMENT_DEFAULTS)
        .target(TargetAction::Create, Some(NodeType::Requirement))
        .review_link(Some(NodeType::Requirement))
        .with_etag("/edit/etag", false),
    );
    out.push(
        backed::<super::super::UpdateRequirementResource>(
            "update-requirement",
            "updateRequirement",
            HttpMethod::Patch,
            "/requirements/{id}",
            "Apply one guarded Requirement text and relationship delta.",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .header("If-Match", "expected_etag", true)
        .cli_defaults(UPDATE_REQUIREMENT_DEFAULTS)
        .public_patch(&[
            ("description", "description"),
            ("fog", "fog"),
            ("domain_id", "domain_id"),
        ])
        .target(TargetAction::Update, Some(NodeType::Requirement))
        .review_link(Some(NodeType::Requirement))
        .with_etag("/edit/etag", false),
    );
}
