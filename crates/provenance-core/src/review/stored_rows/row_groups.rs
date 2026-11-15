//! The lines of one row ID.

use std::collections::HashMap;

/// The lines of one row ID, in line order.
#[derive(Debug)]
pub(super) struct IdGroup {
    pub(super) positions: Vec<usize>,
    /// The first line whose contents are not the contents of the first line.
    pub(super) differing: Option<usize>,
}

/// The lines of each ID that `id` gives, in the order of the first line of
/// each ID. A row for which `id` gives `None` is in no group.
pub(super) fn group_by_id<'a, T: PartialEq>(
    rows: &'a [T],
    id: impl Fn(&'a T) -> Option<&'a str>,
) -> Vec<IdGroup> {
    let mut groups: Vec<IdGroup> = Vec::new();
    let mut index = HashMap::new();
    for (position, row) in rows.iter().enumerate() {
        let Some(key) = id(row) else {
            continue;
        };
        if let Some(&group) = index.get(key) {
            let group: &mut IdGroup = &mut groups[group];
            if group.differing.is_none() && rows[group.positions[0]] != *row {
                group.differing = Some(position);
            }
            group.positions.push(position);
        } else {
            index.insert(key, groups.len());
            groups.push(IdGroup {
                positions: vec![position],
                differing: None,
            });
        }
    }
    groups
}
