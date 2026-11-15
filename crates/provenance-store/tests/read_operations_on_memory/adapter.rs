//! The read database contract of the store, on the `SQLite` database in
//! memory that the other tests of this binary read through.
//!
//! Each case runs on the database alone and behind the contract's `Other`
//! wrapper, as the other tests use it. The cases of the projection and of
//! the tables that the store owns run a catch-up, so they need `host`.

use crate::sqlite_database::MemorySqlite;
use provenance_store::read_database::contract::{self, Harness, Other};
use provenance_store::read_database::{ReadDatabase, SqlFuture, SqlValue, Transaction};

/// One database in memory that nothing has written.
struct Memory(MemorySqlite);

impl Memory {
    async fn new() -> Self {
        Self(MemorySqlite::new().await)
    }
}

impl Harness for Memory {
    fn database(&self) -> &dyn ReadDatabase {
        &self.0
    }

    fn reopen(&self) -> Box<dyn ReadDatabase> {
        Box::new(self.0.clone())
    }

    /// The one connection runs one transaction at a time, so a write that
    /// starts beside a read runs after the read.
    fn reads_interleave(&self) -> bool {
        false
    }

    fn fail_commit<'a>(&'a self, transaction: &'a mut Transaction) -> SqlFuture<'a, ()> {
        Box::pin(dangling_deferred_key(transaction))
    }
}

/// The database in memory behind another transaction type.
struct Wrapped(Memory, Other<MemorySqlite>);

impl Wrapped {
    async fn new() -> Self {
        let memory = Memory::new().await;
        let wrapped = Other::new(memory.0.clone());
        Self(memory, wrapped)
    }
}

impl Harness for Wrapped {
    fn database(&self) -> &dyn ReadDatabase {
        &self.1
    }

    fn reopen(&self) -> Box<dyn ReadDatabase> {
        Box::new(Other::new(self.0 .0.clone()))
    }

    fn reads_interleave(&self) -> bool {
        self.0.reads_interleave()
    }

    fn fail_commit<'a>(&'a self, transaction: &'a mut Transaction) -> SqlFuture<'a, ()> {
        self.0.fail_commit(transaction)
    }
}

/// Leaves a row whose deferred foreign key has no parent, so `SQLite`
/// refuses the commit and rolls the transaction back.
async fn dangling_deferred_key(transaction: &mut Transaction) -> anyhow::Result<()> {
    assert_eq!(
        transaction.fetch("PRAGMA foreign_keys", Vec::new()).await?,
        vec![vec![SqlValue::Integer(1)]],
        "the database checks foreign keys, so a dangling deferred key fails the commit"
    );
    for sql in [
        "CREATE TABLE IF NOT EXISTS commit_parent (id INTEGER PRIMARY KEY)",
        "CREATE TABLE IF NOT EXISTS commit_child (parent INTEGER \
         REFERENCES commit_parent (id) DEFERRABLE INITIALLY DEFERRED)",
        "INSERT INTO commit_child (parent) VALUES (1)",
    ] {
        transaction.execute(sql, Vec::new()).await?;
    }
    Ok(())
}

macro_rules! cases {
    ($($(#[$cfg:meta])* $module:ident :: $case:ident),* $(,)?) => {
        $(
            $(#[$cfg])*
            #[tokio::test]
            async fn $case() {
                contract::$module::$case(Memory::new().await).await;
                contract::$module::$case(Wrapped::new().await).await;
            }
        )*
    };
}

cases!(
    handles::the_list_handles_read_the_projection,
    handles::the_document_handle_reads_the_projection,
    handles::the_discussion_handle_reads_the_projection,
    handles::the_message_handle_reads_the_projection,
    limits::a_bounded_read_runs_the_statements_within_its_bound,
    limits::a_statement_far_past_the_bound_gives_over_budget,
    limits::a_statement_well_within_the_bound_gives_its_rows,
    limits::a_limit_bounds_the_rows,
    limits::a_list_of_the_most_values_runs,
    #[cfg(feature = "host")]
    projection::a_catch_up_makes_and_follows_the_projection,
    #[cfg(feature = "host")]
    projection::a_schema_that_is_not_current_is_made_again,
    #[cfg(feature = "host")]
    projection::a_rebuild_keeps_the_catalog_and_the_rows,
    #[cfg(feature = "host")]
    projection::a_rebuild_keeps_a_table_that_the_store_does_not_own,
    range::an_integer_past_the_range_is_refused_and_nothing_is_written,
    #[cfg(feature = "host")]
    ownership::a_trigger_on_a_table_of_the_store_is_removed,
    #[cfg(feature = "host")]
    ownership::an_index_on_a_table_of_the_store_is_removed,
    #[cfg(feature = "host")]
    ownership::an_object_that_takes_a_name_of_the_store_is_removed,
    sequence::a_read_answers_from_one_committed_state,
    sequence::a_write_then_read_reads_its_write,
    sequence::a_failed_write_stops_the_read,
    sequence::a_failed_read_keeps_the_write,
    sequence::a_write_whose_commit_fails_keeps_nothing,
);

macro_rules! top_cases {
    ($($case:ident),* $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                contract::$case(Memory::new().await).await;
                contract::$case(Wrapped::new().await).await;
            }
        )*
    };
}

top_cases!(
    a_missing_table_is_reported_as_missing,
    a_write_keeps_each_value_class,
    a_failed_write_keeps_nothing,
    a_write_without_its_commit_keeps_nothing,
    a_write_that_fails_after_its_commit_keeps_the_commit,
    a_read_keeps_no_writes,
    an_attempt_keeps_all_or_none,
    the_projection_statement_forms_run,
);

#[tokio::test]
async fn a_read_of_an_absent_database_refuses_before_its_work() {
    contract::a_read_of_an_absent_database_refuses_before_its_work(&Memory::new().await).await;
    contract::a_read_of_an_absent_database_refuses_before_its_work(&Wrapped::new().await).await;
}

#[tokio::test]
async fn the_cursor_key_stays_the_same() {
    contract::the_cursor_key_stays_the_same(&Memory::new().await);
    contract::the_cursor_key_stays_the_same(&Wrapped::new().await);
}
