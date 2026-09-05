use serde::{Deserialize, Serialize};

/// What a query answer reflects: the projection revision the rows came
/// from, the freshness step the reader ran, and which parts of the answer
/// the revision covers.
///
/// `serial` and `digest` name the latest `projection_revision` row and
/// `instance_id` the `projection_instance` row; serials compare only within
/// one instance. `attested` names the projection tables behind the answer.
/// `live` names what the stamp does not cover, from a closed list:
/// `canonical` (canonical shards), `scanned_sites` (a working-tree scan),
/// `verification_runs` (cache JSONL), and `diff` (git). A stamp never
/// implies freshness for anything it does not list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Stamp {
    pub serial: i64,
    pub digest: String,
    pub instance_id: String,
    /// The reader logic version. It moves when the reader answers
    /// differently over the same rows, never for a migration.
    pub derivation: u32,
    pub policy: StampPolicy,
    pub attested: Vec<String>,
    pub live: Vec<String>,
}

/// The freshness step a read ran before it answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StampPolicy {
    /// Catch-up ran under the publication guard and the answer is at or
    /// after the serial it committed.
    CatchUp,
    /// No freshness step; the answer is at the stored serial.
    AnnotateOnly,
    /// Reserved: a read refuses when the projection is behind.
    RefuseStale,
    /// Catch-up failed and the answer is at the stored serial; the error
    /// text travels in `freshness_error`.
    CatchUpFailed,
}

/// One answer with its stamp.
#[derive(Debug, Clone)]
pub struct Stamped<Result> {
    pub result: Result,
    pub stamp: Stamp,
    /// The failed freshness step's error, when the policy is
    /// `catch_up_failed`.
    pub freshness_error: Option<String>,
}
