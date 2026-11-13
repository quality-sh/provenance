//! The model of law L3: the values that each record can hold after the
//! writes of a run.
//!
//! The model starts from the values of the initial state. A write that is
//! certain gives its record its own value. A write that is not certain (a
//! stop, an unknown outcome, or a failure after a landed publish) can have
//! landed or not, so its record holds its own value or a value that the
//! record could hold before the write. The engine law and the CLI law give
//! their writes to this model, so both use the same rule.

use crate::generate::{initial_values, Target, Value};
use crate::laws::Violation;
use crate::FinalRecords;
use std::collections::BTreeMap;

/// One write of a record.
#[derive(Debug, Clone)]
pub struct Write {
    pub target: Target,
    /// The value that the record holds when the write lands.
    pub value: Value,
    /// The write landed, as far as the run can tell.
    pub certain: bool,
}

/// The values that each record can hold.
#[derive(Debug)]
pub struct Expected {
    /// Each record, with the point of the last write that changed the
    /// values (`None` for the initial state) and the values.
    records: BTreeMap<Target, (Option<usize>, Vec<Value>)>,
}

impl Expected {
    /// The values of the initial state.
    pub fn initial() -> Self {
        let records = initial_values()
            .into_iter()
            .map(|(target, value)| (target, (None, vec![value])))
            .collect();
        Self { records }
    }

    /// Applies a group of writes at point `at`.
    ///
    /// The writes of one group ran at the same time, so they landed in an
    /// order that is not known. A record that the group writes holds a value
    /// of one of its writes. When no write of the group on the record is
    /// certain, the record can also hold a value that it could hold before
    /// the group. A certain write landed, so with one certain write the
    /// record cannot hold an earlier value.
    pub fn apply(&mut self, at: usize, group: impl IntoIterator<Item = Write>) {
        let mut changed: BTreeMap<Target, (bool, Vec<Value>)> = BTreeMap::new();
        for write in group {
            let (certain, values) = changed.entry(write.target).or_insert((false, Vec::new()));
            *certain |= write.certain;
            if !values.contains(&write.value) {
                values.push(write.value);
            }
        }
        for (target, (certain, mut values)) in changed {
            let earlier = self.records.remove(&target).map(|(_, values)| values);
            if !certain {
                for value in earlier.into_iter().flatten() {
                    if !values.contains(&value) {
                        values.push(value);
                    }
                }
            }
            self.records.insert(target, (Some(at), values));
        }
    }

    /// Breaks L3 when a record of `state` holds none of its values.
    ///
    /// The text names a point as "after <point> <at>".
    pub fn check(&self, state: &FinalRecords, point: &str) -> Result<(), Violation> {
        for (target, (at, values)) in &self.records {
            if values.iter().any(|value| state.holds(target, value)) {
                continue;
            }
            let when = at.map_or_else(
                || "with no write".to_owned(),
                |at| format!("after {point} {at}"),
            );
            return Err(Violation {
                law: "L3",
                detail: format!("{when}, {target:?} holds none of {values:?}"),
            });
        }
        Ok(())
    }
}
