macro_rules! collection {
    (searchable, $ty:ty, $list:ty, $plural:literal, $plural_id:literal, $singular:literal) => {{
        let definition = read::<$ty, $list>(
            concat!("list-", $plural), concat!("list", $plural_id), concat!("/", $plural),
            concat!("List ", $plural, " in the bound scope."), ResponseKind::Items,
            list_parameters(true, false),
        ).items_field("items").pagination();
        let queries = searchable_queries(&definition, $singular);
        with_query_results(definition, queries)
    }};
    (rules, $ty:ty, $list:ty, $plural:literal, $plural_id:literal, $singular:literal) => {{
        let definition = read::<$ty, $list>(
            concat!("list-", $plural), concat!("list", $plural_id), concat!("/", $plural),
            concat!("List ", $plural, " in the bound scope."), ResponseKind::Items,
            list_parameters(true, true),
        ).items_field("items").pagination();
        let queries = vec![
            query_route::<$crate::operations::catalog::Search>(&definition, ResponseKind::Items, Some($singular), true, ResponseAdapter::ObjectItems("nodes"), &["nodes"]),
            query_route::<$crate::operations::catalog::Stale>(&definition, ResponseKind::Items, None, false, ResponseAdapter::ObjectItems("sites"), &["sites"]),
            query_route::<$crate::operations::catalog::ResolveSymbol>(&definition, ResponseKind::Items, None, false, ResponseAdapter::ObjectItems("rules"), &["rules"]),
        ];
        with_query_results(definition, queries)
    }};
    (plain, $ty:ty, $list:ty, $plural:literal, $plural_id:literal, $singular:literal) => {
        read::<$ty, $list>(
            concat!("list-", $plural), concat!("list", $plural_id), concat!("/", $plural),
            concat!("List ", $plural, " in the bound scope."), ResponseKind::Items,
            list_parameters(false, false),
        ).items_field("items").pagination()
    };
    (verification, $ty:ty, $list:ty, $plural:literal, $plural_id:literal, $singular:literal) => {
        read::<$ty, $list>(
            concat!("list-", $plural), concat!("list", $plural_id), concat!("/", $plural),
            concat!("List ", $plural, " in the bound scope."), ResponseKind::Items,
            vec![
                schema::query("rule", json!({"type":"string","minLength":1})),
                schema::query("limit", json!({"type":"integer","minimum":1,"maximum":200})),
                schema::query("cursor", json!({"type":"string"})),
            ],
        ).items_field("items").pagination()
    };
}

macro_rules! member_parameters_for {
    (searchable) => {
        member_parameters(true)
    };
    (rules) => {
        member_parameters(true)
    };
    (plain) => {
        member_parameters(false)
    };
    (verification) => {
        member_parameters(false)
    };
}

macro_rules! registered_member_queries {
    (searchable, $definition:expr, $singular:expr) => {
        member_queries($definition, $singular)
    };
    (rules, $definition:expr, $singular:expr) => {
        member_queries($definition, $singular)
    };
    (plain, $definition:expr, $singular:expr) => {
        Vec::new()
    };
    (verification, $definition:expr, $singular:expr) => {
        Vec::new()
    };
}

macro_rules! resource {
    ($out:ident, $mode:ident, $ty:ty, $list:ty, $member:ty, $plural:literal, $singular:literal, $singular_id:literal, $plural_id:literal) => {{
        $out.push(collection!(
            $mode, $ty, $list, $plural, $plural_id, $singular
        ));
        let definition = read::<$ty, $member>(
            concat!("get-", $singular),
            concat!("get", $singular_id),
            concat!("/", $plural, "/{id}"),
            concat!("Read one ", $singular, " in the bound scope."),
            ResponseKind::Resource,
            member_parameters_for!($mode),
        )
        .result();
        let queries = registered_member_queries!($mode, &definition, $singular);
        $out.push(with_query_results(definition, queries));
    }};
    ($out:ident, $mode:ident, $ty:ty, $list:ty, $member:ty, $plural:literal, $singular:literal, $singular_id:literal, $plural_id:literal, $create:ty, $update:ty, $create_defaults:expr, $create_aliases:expr, $update_defaults:expr, $update_aliases:expr, $nullable:expr, $target_kind:expr) => {{
        resource!(
            $out,
            $mode,
            $ty,
            $list,
            $member,
            $plural,
            $singular,
            $singular_id,
            $plural_id
        );
        $out.push(
            backed::<$create>(
                concat!("create-", $singular),
                concat!("create", $singular_id),
                HttpMethod::Post,
                concat!("/", $plural),
                concat!("Create one ", $singular, " in the bound scope."),
                ResponseKind::Resource,
                Vec::new(),
            )
            .scope("scope_id")
            .cli_defaults($create_defaults)
            .argument_aliases($create_aliases)
            .target(TargetAction::Create, $target_kind),
        );
        $out.push(
            backed::<$update>(
                concat!("update-", $singular),
                concat!("update", $singular_id),
                HttpMethod::Patch,
                concat!("/", $plural, "/{id}"),
                concat!("Apply a partial change to one ", $singular, "."),
                ResponseKind::Resource,
                vec![schema::path("id")],
            )
            .scope("scope_id")
            .cli_defaults($update_defaults)
            .argument_aliases($update_aliases)
            .public_patch($nullable)
            .target(TargetAction::Update, $target_kind),
        );
    }};
}
