//! Probe: the store's narrow family table expands in another crate.

macro_rules! count_families {
    ($($group:ident {
        $($variant:ident {
            record: $record:ty,
            order: $order:tt,
            node: [$($node:tt)*],
            catalog: [$($catalog:tt)*],
            route: [$($route:tt)*]
            $(, review: $review:ident)?
        };)*
    })*) => {
        pub const FAMILY_NAMES: &[&str] = &[$($(stringify!($variant),)*)*];
    };
}

provenance_store::record_family_catalog!(count_families);
