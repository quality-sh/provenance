use super::initialized_store;
use crate::state_store::{PostMessageInput, StateStore};
use crate::{jsonl, shards};
use provenance_core::threads::choose_canonical_active_thread;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    Message, MessageRole, NodeType, ScopeId, StableId, Thread, ThreadParent, ThreadStatus,
};
use provenance_macros::verifies;

#[test]
fn list_messages_reads_all_month_shards() {
    let (_dir, store, scope) = initialized_store();
    let first_message = Message {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new("msg_july").unwrap(),
        thread_id: StableId::new("thread_source_source_schads").unwrap(),
        role: MessageRole::User,
        body: "July message".into(),
        created_at: 1,
        ai_metadata: None,
    };
    let second_message = Message {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new("msg_august").unwrap(),
        thread_id: StableId::new("thread_source_source_schads").unwrap(),
        role: MessageRole::Assistant,
        body: "August message".into(),
        created_at: 2,
        ai_metadata: None,
    };
    let threads_dir = store
        .layout
        .scopes_dir()
        .join(scope.as_str())
        .join("threads");
    std::fs::create_dir_all(&threads_dir).unwrap();
    std::fs::write(
        threads_dir.join("2026-07.jsonl"),
        format!("{}\n", serde_json::to_string(&first_message).unwrap()),
    )
    .unwrap();
    std::fs::write(
        threads_dir.join("2026-08.jsonl"),
        format!("{}\n", serde_json::to_string(&second_message).unwrap()),
    )
    .unwrap();

    let messages = store.list_messages(&scope).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].id, first_message.id);
    assert_eq!(messages[1].id, second_message.id);
}

// The records a generated thread can hang off. Three is enough to tell
// "archives the posted record's siblings" apart from "archives every active
// thread in the shard".
const PARENT_POOL: [(NodeType, &str); 3] = [
    (NodeType::Requirement, "req_alpha"),
    (NodeType::Rule, "rule_beta"),
    (NodeType::Source, "source_gamma"),
];

const MAX_THREADS_PER_PARENT: usize = 3;

// created_at is drawn from a range smaller than the thread count per record,
// so the canonical choice hits its id tiebreak often.
const CREATED_AT_SPREAD: u64 = 3;

/// `SplitMix64`. A deterministic generator, so a failing run replays from its
/// seed on any machine.
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

fn parent(index: usize) -> ThreadParent {
    let (node_type, node_id) = PARENT_POOL[index];
    ThreadParent {
        node_type,
        node_id: StableId::new(node_id).unwrap(),
    }
}

// Seeded thread ids stay clear of `thread_<parent_type>_<parent_id>`, the id
// the write path mints, so a generated thread is never confused with a created
// one.
fn seeded_threads(rng: &mut Rng, scope: &ScopeId) -> Vec<Thread> {
    let mut threads = Vec::new();
    for index in 0..PARENT_POOL.len() {
        let parent = parent(index);
        for ordinal in 0..rng.index(MAX_THREADS_PER_PARENT + 1) {
            let status = match rng.below(3) {
                0 => ThreadStatus::Resolved,
                1 => ThreadStatus::Archived,
                _ => ThreadStatus::Active,
            };
            threads.push(Thread {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope.clone(),
                id: StableId::new(format!("thread_seed_{index}_{ordinal}")).unwrap(),
                parent: parent.clone(),
                status,
                created_at: i64::try_from(rng.below(CREATED_AT_SPREAD)).unwrap(),
            });
        }
    }
    // Shard order is not the order the rule is allowed to depend on.
    rng.shuffle(&mut threads);
    threads
}

fn of_record<'a>(threads: &'a [Thread], parent: &ThreadParent) -> Vec<&'a Thread> {
    threads
        .iter()
        .filter(|thread| thread.parent == *parent)
        .collect()
}

fn active_ids(threads: &[Thread], parent: &ThreadParent) -> Vec<String> {
    of_record(threads, parent)
        .into_iter()
        .filter(|thread| thread.status == ThreadStatus::Active)
        .map(|thread| thread.id.as_str().to_string())
        .collect()
}

fn find<'a>(threads: &'a [Thread], id: &StableId) -> &'a Thread {
    threads
        .iter()
        .find(|thread| thread.id == *id)
        .unwrap_or_else(|| panic!("thread {} vanished from the shard", id.as_str()))
}

fn seed_threads(store: &StateStore, scope: &ScopeId, threads: &[Thread]) {
    jsonl::write_jsonl_atomic(&shards::threads_path(&store.layout, scope), threads).unwrap();
}

fn post(store: &StateStore, scope: &ScopeId, parent: &ThreadParent) -> Thread {
    store
        .post_thread_message(PostMessageInput {
            scope_id: scope.clone(),
            parent: parent.clone(),
            role: MessageRole::User,
            body: "posted".into(),
        })
        .unwrap()
        .thread
}

/// Independent statement of the decision, checked against the shard the write
/// path left behind rather than against the primary implementation: the posted record
/// keeps exactly one active thread, it is the one `rule_canonical_thread`
/// picks out of what was there, every other thread of that record that was
/// active is now archived, and nothing else in the shard moved.
fn assert_converged(before: &[Thread], after: &[Thread], target: &ThreadParent, posted: &Thread) {
    let survivors = active_ids(after, target);
    assert_eq!(
        survivors,
        vec![posted.id.as_str().to_string()],
        "record {target:?} did not converge to the posted thread; before {before:?}, after {after:?}"
    );
    assert_eq!(posted.status, ThreadStatus::Active);

    let seeded_of_record: Vec<Thread> = of_record(before, target).into_iter().cloned().collect();
    match choose_canonical_active_thread(&seeded_of_record) {
        Some(canonical) => assert_eq!(
            posted.id, canonical.id,
            "posted thread is not the canonical one from {seeded_of_record:?}"
        ),
        None => assert!(
            before.iter().all(|thread| thread.id != posted.id),
            "a thread was created for {target:?} even though {seeded_of_record:?} held an active one"
        ),
    }

    for seeded in before {
        let landed = find(after, &seeded.id);
        let expected = if seeded.parent == *target
            && seeded.status == ThreadStatus::Active
            && seeded.id != posted.id
        {
            &ThreadStatus::Archived
        } else {
            &seeded.status
        };
        assert_eq!(
            &landed.status,
            expected,
            "thread {} ended {:?}, expected {expected:?}; before {before:?}, after {after:?}",
            landed.id.as_str(),
            landed.status
        );
        assert_eq!(
            (&landed.parent, landed.created_at),
            (&seeded.parent, seeded.created_at),
            "the write path rewrote more than the status of {}",
            landed.id.as_str()
        );
    }
}

#[test]
#[verifies("rule_thread_siblings_archived", property)]
fn posting_leaves_one_active_thread_on_the_posted_record() {
    let mut rng = Rng::new(0x0BAD_5EED_C0FF_EE03);
    let mut seen_archiving = 0_u32;
    let mut seen_creation = 0_u32;
    let mut seen_duplicates_left_elsewhere = 0_u32;

    for _ in 0..600 {
        let (_dir, store, scope) = initialized_store();
        let before = seeded_threads(&mut rng, &scope);
        seed_threads(&store, &scope, &before);
        let target = parent(rng.index(PARENT_POOL.len()));

        let posted = post(&store, &scope, &target);
        let after = store.list_threads(&scope).unwrap();

        assert_converged(&before, &after, &target, &posted);

        let before_active = active_ids(&before, &target).len();
        if before_active > 1 {
            seen_archiving += 1;
        }
        if before_active == 0 {
            seen_creation += 1;
        }
        if (0..PARENT_POOL.len())
            .map(parent)
            .any(|other| other != target && active_ids(&after, &other).len() > 1)
        {
            seen_duplicates_left_elsewhere += 1;
        }
    }

    // The property is worth only as much as the inputs it saw.
    assert!(
        seen_archiving > 40,
        "generator produced too few records holding several active threads: {seen_archiving}"
    );
    assert!(
        seen_creation > 40,
        "generator produced too few records with no active thread to adopt: {seen_creation}"
    );
    // Not a corner of the rule but of its reach: a posting write converges the
    // record it posts to and no other, so duplicates elsewhere survive it.
    assert!(
        seen_duplicates_left_elsewhere > 40,
        "no run left another record holding several active threads: \
         {seen_duplicates_left_elsewhere}"
    );
}

#[test]
#[verifies("rule_thread_siblings_archived", examples)]
fn posting_archives_the_losing_active_siblings_of_that_record() {
    let (_dir, store, scope) = initialized_store();
    let target = parent(0);
    let other = parent(1);
    let seeded = vec![
        thread(&scope, "thread_seed_old", &target, ThreadStatus::Active, 1),
        thread(&scope, "thread_seed_new", &target, ThreadStatus::Active, 5),
        thread(
            &scope,
            "thread_seed_done",
            &target,
            ThreadStatus::Resolved,
            0,
        ),
        thread(
            &scope,
            "thread_seed_gone",
            &target,
            ThreadStatus::Archived,
            0,
        ),
        thread(&scope, "thread_seed_other", &other, ThreadStatus::Active, 2),
    ];
    seed_threads(&store, &scope, &seeded);

    let posted = post(&store, &scope, &target);

    let after = store.list_threads(&scope).unwrap();
    assert_eq!(posted.id.as_str(), "thread_seed_old");
    assert_eq!(
        active_ids(&after, &target),
        vec!["thread_seed_old".to_string()]
    );
    assert_eq!(
        find(&after, &StableId::new("thread_seed_new").unwrap()).status,
        ThreadStatus::Archived
    );
    assert_eq!(
        find(&after, &StableId::new("thread_seed_done").unwrap()).status,
        ThreadStatus::Resolved
    );
    assert_eq!(
        find(&after, &StableId::new("thread_seed_gone").unwrap()).status,
        ThreadStatus::Archived
    );
    assert_eq!(
        find(&after, &StableId::new("thread_seed_other").unwrap()).status,
        ThreadStatus::Active,
        "posting to one record archived a thread of another"
    );
}

#[test]
#[verifies("rule_thread_siblings_archived", examples)]
fn posting_twice_to_a_record_reuses_its_one_active_thread() {
    let (_dir, store, scope) = initialized_store();
    let target = parent(0);

    let first = post(&store, &scope, &target);
    let second = post(&store, &scope, &target);

    assert_eq!(first.id, second.id);
    let after = store.list_threads(&scope).unwrap();
    assert_eq!(after.len(), 1);
    assert_eq!(
        active_ids(&after, &target),
        vec![first.id.as_str().to_string()]
    );
}

#[test]
fn posting_after_terminal_history_mints_then_reuses_a_distinct_thread_id() {
    for status in [ThreadStatus::Resolved, ThreadStatus::Archived] {
        let (_dir, store, scope) = initialized_store();
        let target = parent(0);
        let terminal = thread(
            &scope,
            "thread_requirement_req_alpha",
            &target,
            status.clone(),
            1,
        );
        seed_threads(&store, &scope, std::slice::from_ref(&terminal));

        let first = post(&store, &scope, &target);
        let second = post(&store, &scope, &target);
        let after = store.list_threads(&scope).unwrap();

        assert_ne!(first.id, terminal.id);
        assert_eq!(second.id, first.id);
        assert_eq!(after.len(), 2);
        assert_eq!(find(&after, &terminal.id).status, status);
        assert_eq!(
            active_ids(&after, &target),
            vec![first.id.as_str().to_string()]
        );
    }
}

fn thread(
    scope: &ScopeId,
    id: &str,
    parent: &ThreadParent,
    status: ThreadStatus,
    created_at: i64,
) -> Thread {
    Thread {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        parent: parent.clone(),
        status,
        created_at,
    }
}

#[test]
fn thread_parents_stay_within_the_six_original_kinds() {
    let (_dir, store, scope) = initialized_store();
    for kind in [NodeType::Domain, NodeType::Boundary] {
        let error = store
            .post_thread_message(PostMessageInput {
                scope_id: scope.clone(),
                parent: ThreadParent {
                    node_type: kind,
                    node_id: StableId::new("domain_x").unwrap(),
                },
                role: MessageRole::User,
                body: "Hello".into(),
            })
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("thread parent"),
            "kind {kind:?} must be refused with a thread-parent error, got: {error}"
        );
    }
}
