//! Labelled type information for record-family implementations.

macro_rules! project_families {
    ($consumer:ident; $argument:ident; $($rows:tt)*) => {
        $crate::cache::family_views::project_families!(
            @project $consumer [$argument;]; $($rows)*
        );
    };
    ($consumer:ident; $($rows:tt)*) => {
        $crate::cache::family_views::project_families!(
            @project $consumer []; $($rows)*
        );
    };
    (
        @project $consumer:ident [$($prefix:tt)*];
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
        $consumer! {
            $($prefix)*
            $(
                $group {
                    $(
                        $variant {
                            record: $record,
                            field: $field,
                            path: $path,
                            node: [$($node)*],
                            reader: $reader,
                            closed: [$($closed)*],
                            strategy: $strategy,
                            id: $id,
                            loader: [$($loader)*],
                            catalog: [$($catalog)*]
                            $(, review: $review)?
                        };
                    )*
                }
            )*
        }
    };
}

macro_rules! record_families {
    ($consumer:ident) => {
        $crate::cache::family_table::record_family_rows!(
            $crate::cache::family_views::project_families,
            $consumer
        );
    };
    ($consumer:ident, $argument:ident) => {
        $crate::cache::family_table::record_family_rows!(
            $crate::cache::family_views::project_families,
            $consumer,
            $argument
        );
    };
}

pub(crate) use project_families;
pub(crate) use record_families;
