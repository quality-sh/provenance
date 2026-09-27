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
    ($out:ident, $record:ty, projection($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, none), requirements) => {
        requirements(&mut $out);
    };
    ($out:ident, $record:ty, $kind:ident($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, $member:ident, $member_wire:literal), read { mode: $mode:ident, plural: $plural:literal, singular: $singular:literal, singular_id: $singular_id:literal, plural_id: $plural_id:literal }) => {
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
    ($out:ident, $record:ty, verification($list:ident, $list_wire:literal, $page:ident, $member:ident, $member_wire:literal), read { mode: $mode:ident, plural: $plural:literal, singular: $singular:literal, singular_id: $singular_id:literal, plural_id: $plural_id:literal }) => {
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
    ($out:ident, $record:ty, $kind:ident($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, $member:ident, $member_wire:literal), writable { mode: $mode:ident, plural: $plural:literal, singular: $singular:literal, singular_id: $singular_id:literal, plural_id: $plural_id:literal, create: $create:ident, update: $update:ident, create_defaults: $create_defaults:ident, create_aliases: $create_aliases:ident, update_defaults: $update_defaults:ident, update_aliases: $update_aliases:ident, nullable: $nullable:expr, target: $target:expr }) => {
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
    ($out:ident, $record:ty, $kind:ident($list:ident, $list_wire:literal, $page:ident, $page_wire:literal, $member:ident, $member_wire:literal), proposal { plural: $plural:literal, singular: $singular:literal, singular_id: $singular_id:literal, plural_id: $plural_id:literal }) => {{
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
    ($out:ident, $record:ty, [$($catalog:tt)*], [none]) => {};
    ($out:ident, $record:ty, [$($catalog:tt)*], [order: $order:literal, $($route:tt)+]) => {{
        let mut definitions = Vec::new();
        family_route_definitions!(definitions, $record, $($catalog)*, $($route)+);
        $out.push(($order, definitions));
    }};
}

macro_rules! register_family_routes {
    (
        $out:ident;
        export { $($export_variant:ident { record: $export_type:ty, field: $export_field:ident, shard: { path: $export_path:ident, suffix: $export_suffix:literal, table: $export_table:literal }, node: [$($export_node:tt)*], reader: { open: $export_reader:ident, closed: [$($export_closed:tt)*], strategy: $export_strategy:ident }, id: $export_id:ident, loader: [$($export_loader:tt)*], graph: [$($export_graph:tt)*], import: [$($export_import:tt)*], catalog: [$($export_catalog:tt)*], route: [$($export_route:tt)*] };)* }
        canonical { $($canonical_variant:ident { record: $canonical_type:ty, field: $canonical_field:ident, shard: { path: $canonical_path:ident, suffix: $canonical_suffix:literal, table: $canonical_table:literal }, node: [$($canonical_node:tt)*], reader: { open: $canonical_reader:ident, closed: [$($canonical_closed:tt)*], strategy: $canonical_strategy:ident }, id: $canonical_id:ident, loader: [$($canonical_loader:tt)*], graph: [$($canonical_graph:tt)*], import: [$($canonical_import:tt)*], catalog: [$($canonical_catalog:tt)*], route: [$($canonical_route:tt)*] };)* }
        bindings { $($binding_variant:ident { record: $binding_type:ty, field: $binding_field:ident, shard: { path: $binding_path:ident, suffix: $binding_suffix:literal, table: $binding_table:literal }, node: [$($binding_node:tt)*], reader: { open: $binding_reader:ident, closed: [$($binding_closed:tt)*], strategy: $binding_strategy:ident }, id: $binding_id:ident, loader: [$($binding_loader:tt)*], graph: [$($binding_graph:tt)*], import: [$($binding_import:tt)*], catalog: [$($binding_catalog:tt)*], route: [$($binding_route:tt)*] };)* }
        internal { $($internal_variant:ident { record: $internal_type:ty, field: $internal_field:ident, shard: { path: $internal_path:ident, suffix: $internal_suffix:literal, table: $internal_table:literal }, node: [$($internal_node:tt)*], reader: { open: $internal_reader:ident, closed: [$($internal_closed:tt)*], strategy: $internal_strategy:ident }, id: $internal_id:ident, loader: [$($internal_loader:tt)*], graph: [$($internal_graph:tt)*], import: [$($internal_import:tt)*], catalog: [$($internal_catalog:tt)*], route: [$($internal_route:tt)*] };)* }
    ) => {
        $(family_route!($out, $export_type, [$($export_catalog)*], [$($export_route)*]);)*
        $(family_route!($out, $canonical_type, [$($canonical_catalog)*], [$($canonical_route)*]);)*
        $(family_route!($out, $binding_type, [$($binding_catalog)*], [$($binding_route)*]);)*
        $(family_route!($out, $internal_type, [$($internal_catalog)*], [$($internal_route)*]);)*
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
        verification::PageVerificationRunsV2,
        verification::GetVerificationRunV2,
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
    let list = read::<provenance_core::Requirement, pages::PageRequirementsV2>(
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
    let member = backed::<super::super::GetRequirementV2>(
        "get-requirement",
        "getRequirement",
        HttpMethod::Get,
        "/requirements/{id}",
        "Read one Requirement with its edit and decision state.",
        ResponseKind::Resource,
        member_parameters(true),
    )
    .with_etag("/edit/etag", false);
    let queries = member_queries(&member, "requirement");
    out.push(with_query_results(member, queries));
    out.push(
        backed::<super::super::CreateRequirementV2>(
            "create-requirement",
            "createRequirement",
            HttpMethod::Post,
            "/requirements",
            "Create one Requirement through the guarded review journal.",
            ResponseKind::Resource,
            Vec::new(),
        )
        .header("Idempotency-Key", "request_id", false)
        .cli_defaults(CREATE_REQUIREMENT_DEFAULTS)
        .target(TargetAction::Create, Some(NodeType::Requirement))
        .with_etag("/edit/etag", false),
    );
    out.push(
        backed::<super::super::UpdateRequirementV2>(
            "update-requirement",
            "updateRequirement",
            HttpMethod::Patch,
            "/requirements/{id}",
            "Apply one guarded Requirement text and relationship delta.",
            ResponseKind::Resource,
            vec![schema::path("id")],
        )
        .header("Idempotency-Key", "request_id", false)
        .header("If-Match", "expected_etag", true)
        .cli_defaults(UPDATE_REQUIREMENT_DEFAULTS)
        .public_patch(&[
            ("description", "description"),
            ("fog", "fog"),
            ("domain_id", "domain_id"),
        ])
        .target(TargetAction::Update, Some(NodeType::Requirement))
        .with_etag("/edit/etag", false),
    );
}
