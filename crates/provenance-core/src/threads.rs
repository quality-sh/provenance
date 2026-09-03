use crate::{StableId, Thread, ThreadParent, ThreadStatus};
use provenance_macros::rule;

/// Picks the canonical thread for a record out of the threads it carries.
///
/// A record can end up with several threads. The oldest thread that is still
/// active is the canonical one; when two active threads share a `created_at`,
/// the lower id wins. Threads that are resolved or archived never win. A
/// record with no active thread has no canonical thread.
#[rule("rule_canonical_thread")]
pub fn choose_canonical_active_thread(threads: &[Thread]) -> Option<&Thread> {
    threads
        .iter()
        .filter(|thread| thread.status == ThreadStatus::Active)
        .min_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then(a.id.as_str().cmp(b.id.as_str()))
        })
}

/// Archives every active thread of a record apart from the canonical one.
///
/// A record can carry several active threads. When a write picks the canonical
/// one, the same write archives every other active thread of that record, so
/// the record converges to one active thread. Threads that are resolved or
/// archived keep the status they have, and threads belonging to other records
/// are left alone.
///
/// Posting reconciles rather than refuses: nothing sweeps the whole shard. A
/// record that somehow gained active siblings is repaired the next time anyone
/// posts to it, and a record nobody posts to keeps them. Whole-scope import is
/// stricter and rejects active siblings rather than choosing on the importer's
/// behalf.
#[rule("rule_thread_siblings_archived")]
pub fn archive_non_canonical_siblings(
    threads: &mut [Thread],
    parent: &ThreadParent,
    canonical: &StableId,
) {
    for thread in threads.iter_mut() {
        if thread.parent == *parent
            && thread.status == ThreadStatus::Active
            && thread.id != *canonical
        {
            thread.status = ThreadStatus::Archived;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::SUPPORTED_SCHEMA_VERSION;
    use provenance_macros::verifies;

    use super::*;
    use crate::{NodeType, ScopeId, StableId, ThreadParent};

    fn thread(id: &str, status: ThreadStatus, created_at: i64) -> Thread {
        Thread {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scope_id: ScopeId::new("default").unwrap(),
            id: StableId::new(id).unwrap(),
            parent: ThreadParent {
                node_type: NodeType::Rule,
                node_id: StableId::new("rule_schads_pay_001").unwrap(),
            },
            status,
            created_at,
        }
    }

    /// `SplitMix64`. A deterministic generator so the property runs are the same
    /// on every machine and a failure can be replayed from its seed.
    struct Rng(u64);

    impl Rng {
        const fn new(seed: u64) -> Self {
            Self(seed)
        }

        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn below(&mut self, bound: u64) -> u64 {
            self.next_u64() % bound
        }

        fn index(&mut self, bound: usize) -> usize {
            usize::try_from(self.below(u64::try_from(bound).unwrap())).unwrap()
        }

        fn shuffle<T>(&mut self, items: &mut [T]) {
            for i in (1..items.len()).rev() {
                items.swap(i, self.index(i + 1));
            }
        }
    }

    // Distinct ids, handed out in shuffled order so that id order and
    // created_at order disagree often enough to exercise the tiebreak.
    const ID_POOL: [&str; 9] = [
        "thread_a", "thread_b", "thread_c", "thread_d", "thread_e", "thread_f", "thread_g",
        "thread_h", "thread_i",
    ];

    // created_at is drawn from a range far smaller than the list length, so
    // collisions are common rather than rare.
    const CREATED_AT_SPREAD: u64 = 4;

    fn random_threads(rng: &mut Rng) -> Vec<Thread> {
        let mut ids = ID_POOL.to_vec();
        rng.shuffle(&mut ids);
        ids.truncate(rng.index(ID_POOL.len() + 1));
        ids.iter()
            .map(|id| {
                let status = match rng.below(3) {
                    0 => ThreadStatus::Resolved,
                    1 => ThreadStatus::Archived,
                    _ => ThreadStatus::Active,
                };
                let created_at = i64::try_from(rng.below(CREATED_AT_SPREAD)).unwrap() - 1;
                thread(id, status, created_at)
            })
            .collect()
    }

    fn active_count(threads: &[Thread]) -> usize {
        threads
            .iter()
            .filter(|thread| thread.status == ThreadStatus::Active)
            .count()
    }

    fn has_created_at_tie_among_active(threads: &[Thread]) -> bool {
        let mut active: Vec<i64> = threads
            .iter()
            .filter(|thread| thread.status == ThreadStatus::Active)
            .map(|thread| thread.created_at)
            .collect();
        active.sort_unstable();
        active.windows(2).any(|pair| pair[0] == pair[1])
    }

    /// Independent statement of the decision: the chosen thread is an active
    /// member of the list that no other active thread beats on
    /// `(created_at, id)`, and nothing is chosen exactly when nothing is
    /// active. Written as a minimality check over pairs, so it does not lean
    /// on the same minimum-search the rule uses.
    fn assert_canonical(threads: &[Thread], chosen: Option<&Thread>) {
        let Some(chosen) = chosen else {
            assert_eq!(
                active_count(threads),
                0,
                "no canonical thread chosen from {threads:?}, but some thread is active"
            );
            return;
        };

        assert!(
            threads.iter().any(|thread| thread.id == chosen.id),
            "chosen thread {:?} is not in {threads:?}",
            chosen.id
        );
        assert_eq!(
            chosen.status,
            ThreadStatus::Active,
            "chosen thread {:?} is not active",
            chosen.id
        );

        for other in threads
            .iter()
            .filter(|thread| thread.status == ThreadStatus::Active && thread.id != chosen.id)
        {
            assert!(
                (chosen.created_at, chosen.id.as_str()) < (other.created_at, other.id.as_str()),
                "chosen thread {:?} at {} loses to active thread {:?} at {}",
                chosen.id,
                chosen.created_at,
                other.id,
                other.created_at
            );
        }
    }

    #[test]
    #[verifies("rule_canonical_thread", property)]
    fn canonical_thread_is_the_least_active_thread_by_created_at_then_id() {
        let mut rng = Rng::new(0x0BAD_5EED_C0FF_EE01);
        let mut seen_empty_choice = 0_u32;
        let mut seen_created_at_tie = 0_u32;

        for _ in 0..5_000 {
            let threads = random_threads(&mut rng);
            let chosen = choose_canonical_active_thread(&threads);

            assert_canonical(&threads, chosen);

            if chosen.is_none() {
                seen_empty_choice += 1;
            }
            if has_created_at_tie_among_active(&threads) {
                seen_created_at_tie += 1;
            }
        }

        // The property is only worth as much as the inputs it saw, so check
        // that the generator reached the corners the decision is about.
        assert!(
            seen_empty_choice > 100,
            "generator produced too few lists with no active thread: {seen_empty_choice}"
        );
        assert!(
            seen_created_at_tie > 100,
            "generator produced too few created_at ties among active threads: \
             {seen_created_at_tie}"
        );
    }

    #[test]
    #[verifies("rule_canonical_thread", property)]
    fn canonical_thread_does_not_depend_on_input_order() {
        let mut rng = Rng::new(0x0BAD_5EED_C0FF_EE02);

        for _ in 0..5_000 {
            let threads = random_threads(&mut rng);
            let expected = choose_canonical_active_thread(&threads).map(|thread| thread.id.clone());

            let mut shuffled = threads.clone();
            rng.shuffle(&mut shuffled);
            let actual = choose_canonical_active_thread(&shuffled).map(|thread| thread.id.clone());

            assert_eq!(
                expected, actual,
                "reordering changed the canonical thread for {threads:?}"
            );
        }
    }

    #[test]
    #[verifies("rule_canonical_thread", examples)]
    fn threads_choose_oldest_active_thread_as_canonical() {
        let threads = vec![
            thread("thread_new", ThreadStatus::Active, 20),
            thread("thread_old", ThreadStatus::Active, 10),
            thread("thread_archived", ThreadStatus::Archived, 1),
        ];

        assert_eq!(
            choose_canonical_active_thread(&threads)
                .unwrap()
                .id
                .as_str(),
            "thread_old"
        );
    }

    #[test]
    #[verifies("rule_canonical_thread", examples)]
    fn ties_on_created_at_go_to_the_lower_id() {
        let threads = vec![
            thread("thread_b", ThreadStatus::Active, 10),
            thread("thread_a", ThreadStatus::Active, 10),
            thread("thread_aa", ThreadStatus::Resolved, 5),
        ];

        assert_eq!(
            choose_canonical_active_thread(&threads)
                .unwrap()
                .id
                .as_str(),
            "thread_a"
        );
    }

    #[test]
    #[verifies("rule_canonical_thread", examples)]
    fn no_active_thread_means_no_canonical_thread() {
        let threads = vec![
            thread("thread_resolved", ThreadStatus::Resolved, 1),
            thread("thread_archived", ThreadStatus::Archived, 2),
        ];

        assert!(choose_canonical_active_thread(&threads).is_none());
        assert!(choose_canonical_active_thread(&[]).is_none());
    }
}
