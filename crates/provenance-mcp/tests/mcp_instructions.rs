//! The full instructions text of hosts with different grants: the opening sentences, the
//! order of the command lines, and the text of the change line for the writes that the
//! host grants. These tests have no Rule binding: no Rule states the text or the order,
//! and the tests in `mcp_instruction_names.rs` are the evidence for the names.
#![cfg(feature = "test-fixture")]

use crate::support::first_calls::{without_first_calls, BEFORE_FIRST_CALLS};
use crate::support::{fixture_host, mcp::Session, records::Repository};
use provenance_porcelain::action::Action;
use provenance_store::operations::catalog::{self, Operation as _};
use provenance_transport::{repository::RepoContext, StatementHost};
use rmcp::model::Tool;

/// The domain text before the action lines.
const DOMAIN: &str = "# Provenance\n\n\
Provenance records requirements, decisions, and the rules that connect them to code.\n\n\
## Records\n\n\
- Requirement: an obligation the system must satisfy. Requirements can refine other Requirements.\n\
- Rule: an identified atomic behavioural obligation that refines one or more Requirements. A Rule can exist before code or verification.\n\
- Resolution: a recorded decision that removes ambiguity. A Resolution can produce a Rule when it establishes a precise obligation.\n\
- Source: identified evidence that graph records can cite. A citation records where an obligation came from.\n\
- Topic: a claimable shaping work area attached to a Requirement.\n\
- Question: a concern to resolve, attached to a Topic and a Requirement, with a selected resolution method.\n\
- Discussion: one concern identified by a root Message and its replies inside a Thread.\n\
- Description: on a Requirement or Rule, how the product behaves in the present tense: states, what changes them, what the person sees, one example. History goes on the thread; unknowns go in a Question.\n\n\
## Code and evidence\n\n\
An Implementation binding connects a Rule to production code that realizes it.\n\
Verification is evidence that a Rule holds. A Verification binding identifies intended evidence; a Verification run records one execution and its result.\n\
Rule lifecycle, decision grounding, implementation, and verification are separate facts. An active Rule can precede its implementation.\n\
Unimplemented and Unverified describe absent bindings or evidence; they are not stored Rule statuses. A graph read does not scan code or establish a coverage verdict.\n\
A source citation does not count as an Implementation binding. Coverage checks evaluate code bindings separately from graph reads.\n\n\
## Actions\n\n";

const SEARCH_THEN_GET: &str =
    "Use search to find IDs, then get to read the selected record and its related views.";
const CREATE_TYPE: &str =
    "Create names an explicit record type; actions on an existing record infer its type.";
const CREATE_ONLY: &str = "Create names an explicit record type.";
const ETAG_BEFORE_UPDATE: &str = "Before an update, read the record with get. Give its \
`edit.etag` unchanged to the update as the if-match value.";
const ACCESS: &str = "Available actions depend on the interface and its access. \
Each interface lists the inputs and actions it supports.\n\n";

const GET: &str = "- get: Read one repository record by its repository-local ID. Select its \
record view, children view (records below this record), trace view (records above this \
record, up to its Sources), impact view, or review view.";
const TRACE: &str = "- trace: Read the records above this record, up to its Sources. \
The get trace view gives the same records.";
const SEARCH: &str = "- search: Find records across the permitted kinds in the bound scope, \
or the Rules bound to a source file and symbol.";
const CHECK: &str = "- check: Check graph validity, statement quality, and binding coverage. \
Give a statement to check that text with the project dictionary before a write.";
const API: &str = "- api: Call one public API path with an optional method, query, headers, \
and JSON body. The body is the data object, without the HTTP {\"data\": ...} wrapper. Omit \
the path to list the catalog routes with their methods and inputs. A filter that is the \
method and path of one route, such as GET /rules, gives the request and response schemas \
of that route.";
const CHANGE: &str = "- change: Write to the graph. The action argument selects the write: \
create, update, answer, claim, release, or submit on the target record; discuss or reply \
for a Discussion; notify to register a client thread for review changes; or materialize, \
import, dictionary_import, or swarm_backtrace_land on the repository files. A call that \
lacks a field that the action and record kind need gets the schema of their fields.";
const CHANGE_WITHOUT_REPOSITORY: &str = "- change: Write to the graph. The action argument \
selects the write: create, update, answer, claim, release, or submit on the target record; \
discuss or reply for a Discussion; or notify to register a client thread for review changes. \
A call that lacks a field that the action and record kind need gets the schema of their \
fields.";
const CHANGE_RECORD_AND_DISCUSSION: &str = "- change: Write to the graph. The action argument \
selects the write: create, update, answer, claim, release, or submit on the target record; or \
discuss or reply for a Discussion. A call that lacks a field that the action and record kind \
need gets the schema of their fields.";
const CHANGE_CREATE_DISCUSS_REPLY: &str = "- change: Write to the graph. The action argument \
selects the write: create on the target record; or discuss or reply for a Discussion. A call \
that lacks a field that the action and record kind need gets the schema of their fields.";
const CHANGE_CREATE_REPLY: &str = "- change: Write to the graph. The action argument selects \
the write: create on the target record; or reply for a Discussion. A call that lacks a field \
that the action and record kind need gets the schema of their fields.";
const CREATE: &str = "- create: Create the target ID as an explicit record type.";
const UPDATE: &str = "- update: Update the existing target while preserving omitted fields.";
const ANSWER: &str = "- answer: Answer the target Question.";
const CLAIM: &str = "- claim: Claim the target Topic.";
const RELEASE: &str = "- release: Release the target Topic claim.";
const SUBMIT: &str = "- submit: Submit the target Source, Requirement, Resolution, Rule, \
Topic, or Question for review.";
const DISCUSSIONS: &str = "- discussions: List addressed Discussions in a scope or under a record.";
const DISCUSSION: &str = "- discussion: Read the addressed Discussion.";
const DISCUSS: &str = "- discuss: Start a Discussion under the target record.";
const REPLY: &str = "- reply: Reply to the addressed Discussion.";
const MATERIALIZE: &str = "- materialize: Build the local query cache again from the graph \
files of the repository.";
const IMPORT: &str = "- import: Replace the records of one scope with a scope export file and \
delete each record that is not in the file.";
const DICTIONARY_IMPORT: &str =
    "- dictionary_import: Import the ASD-STE100 Issue 9 dictionary from one local PDF file.";
const SWARM_BACKTRACE_LAND: &str = "- swarm_backtrace_land: Add the results of one swarm \
backtrace run to the graph as proposals.";

/// The first calls that a server gives after the words before them when it lists search
/// and get.
const FIRST_CALLS: &str = "call search with {\"text\":\"<text>\"}, then get with \
{\"target\":\"<id>\",\"view\":\"children\",\"depth\":<N>}.";

/// The expected instructions: the domain text, the opening sentences on one line, the
/// `edit.etag` sentence when the lines hold get and update, the access sentence, the command
/// lines, and the words before the first calls when `first_steps` is true.
fn expected(opening: &[&str], lines: &[&str], first_steps: bool) -> String {
    let mut text = DOMAIN.to_owned();
    if !opening.is_empty() {
        text = text + &opening.join(" ") + "\n";
    }
    if lines.contains(&GET) && lines.contains(&UPDATE) {
        text = text + ETAG_BEFORE_UPDATE + "\n";
    }
    text = text + ACCESS + &lines.join("\n");
    if first_steps {
        text += BEFORE_FIRST_CALLS;
    }
    text
}

/// The listed tools and the instructions of one served host.
pub async fn served_tools(host: StatementHost) -> (Vec<Tool>, String) {
    let session = Session::start(host).await;
    let instructions = session
        .client
        .peer_info()
        .and_then(|info| info.instructions.clone())
        .expect("server instructions");
    let tools = session.client.list_all_tools().await.unwrap();
    session.close().await;
    (tools, instructions)
}

/// The tool names and the instructions of one served host.
async fn served(host: StatementHost) -> (Vec<String>, String) {
    let (tools, instructions) = served_tools(host).await;
    let instructions = without_first_calls(&instructions);
    let names = tools
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect();
    (names, instructions)
}

/// A host that writes the repository, without the operations that `denied` names.
pub fn host_without(repository: &Repository, denied: &[&str]) -> StatementHost {
    let access = fixture_host::access(repository).allow_writes();
    StatementHost::with_fixture_access(
        denied
            .iter()
            .fold(access, |access, operation| access.deny_operation(operation)),
    )
}

/// The operations of the record actions in `actions`.
pub fn operations_of(actions: &[Action]) -> Vec<&'static str> {
    actions
        .iter()
        .flat_map(|action| provenance_contract::target_definitions(*action))
        .map(|(_, definition)| definition.name)
        .collect()
}

#[tokio::test]
async fn a_host_with_every_tool_names_its_writes_and_gives_first_steps() {
    let repository = Repository::new("The shared graph is readable.");
    let (tools, instructions) = served_tools(fixture_host::full(&repository)).await;
    assert_eq!(
        instructions
            .split_once(BEFORE_FIRST_CALLS)
            .map(|(_, calls)| calls),
        Some(FIRST_CALLS),
        "{instructions}"
    );
    let instructions = without_first_calls(&instructions);
    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    assert_eq!(
        names,
        [
            "get",
            "search",
            "api",
            "check",
            "trace",
            "change",
            "discussions"
        ]
    );
    // The fixture host has no repository files, so the change tool has no
    // repository write. It registers notify threads, so the change tool has notify.
    let change = tools.iter().find(|tool| tool.name == "change").unwrap();
    assert_eq!(
        change.description.as_deref(),
        CHANGE_WITHOUT_REPOSITORY.strip_prefix("- change: ")
    );
    let lines = [
        GET,
        SEARCH,
        API,
        CHECK,
        TRACE,
        CHANGE_WITHOUT_REPOSITORY,
        CREATE,
        UPDATE,
        ANSWER,
        CLAIM,
        RELEASE,
        SUBMIT,
        DISCUSS,
        REPLY,
        DISCUSSIONS,
        DISCUSSION,
    ];
    assert_eq!(
        instructions,
        expected(&[SEARCH_THEN_GET, CREATE_TYPE], &lines, true)
    );
}

/// A host with repository files adds the repository writes to the change line, and one
/// line for each repository write after the other change actions.
#[tokio::test]
async fn a_host_with_repository_files_names_each_repository_write() {
    let repository = Repository::new("The shared graph is readable.");
    let root = repository.dir.path().to_str().unwrap().to_owned();
    let host = fixture_host::full(&repository).with_repository(RepoContext::new(root, "default"));
    let (_, instructions) = served(host).await;
    let lines = [
        GET,
        SEARCH,
        API,
        CHECK,
        TRACE,
        CHANGE,
        CREATE,
        UPDATE,
        ANSWER,
        CLAIM,
        RELEASE,
        SUBMIT,
        DISCUSS,
        REPLY,
        MATERIALIZE,
        IMPORT,
        DICTIONARY_IMPORT,
        SWARM_BACKTRACE_LAND,
        DISCUSSIONS,
        DISCUSSION,
    ];
    assert_eq!(
        instructions,
        expected(&[SEARCH_THEN_GET, CREATE_TYPE], &lines, true)
    );
}

#[tokio::test]
async fn a_host_with_api_and_change_names_no_other_command() {
    let repository = Repository::new("The shared graph is readable.");
    let host = host_without(
        &repository,
        &[
            catalog::ResolveRecord::NAME,
            catalog::Search::NAME,
            catalog::ListDiscussions::NAME,
            catalog::GetDiscussionConversation::NAME,
        ],
    );
    // An action on an existing record resolves the record first, so without
    // resolve-record the change tool has only create, discuss, and reply.
    let lines = [API, CHANGE_CREATE_DISCUSS_REPLY, CREATE, DISCUSS, REPLY];
    assert_eq!(
        served(host).await,
        (
            vec!["api".to_owned(), "change".to_owned()],
            expected(&[CREATE_ONLY], &lines, false)
        )
    );
}

#[tokio::test]
async fn a_host_without_search_or_get_gives_no_first_steps() {
    let repository = Repository::new("The shared graph is readable.");
    let host = host_without(
        &repository,
        &[catalog::ResolveRecord::NAME, catalog::Search::NAME],
    );
    let lines = [
        API,
        CHANGE_CREATE_DISCUSS_REPLY,
        CREATE,
        DISCUSS,
        REPLY,
        DISCUSSIONS,
        DISCUSSION,
    ];
    assert_eq!(
        served(host).await,
        (
            vec![
                "api".to_owned(),
                "change".to_owned(),
                "discussions".to_owned()
            ],
            expected(&[CREATE_ONLY], &lines, false)
        )
    );
}

#[tokio::test]
async fn a_host_with_get_and_without_search_gives_no_first_steps() {
    let repository = Repository::new("The shared graph is readable.");
    let host = host_without(&repository, &[catalog::Search::NAME]);
    let lines = [
        GET,
        API,
        TRACE,
        CHANGE_RECORD_AND_DISCUSSION,
        CREATE,
        UPDATE,
        ANSWER,
        CLAIM,
        RELEASE,
        SUBMIT,
        DISCUSS,
        REPLY,
        DISCUSSIONS,
        DISCUSSION,
    ];
    let names = ["get", "api", "trace", "change", "discussions"];
    assert_eq!(
        served(host).await,
        (
            names.map(str::to_owned).to_vec(),
            expected(&[CREATE_TYPE], &lines, false)
        )
    );
}

#[tokio::test]
async fn a_read_only_host_gives_no_change_line() {
    let repository = Repository::new("The shared graph is readable.");
    let lines = [GET, SEARCH, API, CHECK, TRACE, DISCUSSIONS, DISCUSSION];
    let names = ["get", "search", "api", "check", "trace", "discussions"];
    assert_eq!(
        served(fixture_host::read_only(&repository)).await,
        (
            names.map(str::to_owned).to_vec(),
            expected(&[SEARCH_THEN_GET], &lines, true)
        )
    );
}

#[tokio::test]
async fn a_change_tool_with_create_and_reply_names_only_those_actions() {
    let repository = Repository::new("The shared graph is readable.");
    let mut denied = operations_of(&[
        Action::Update,
        Action::Answer,
        Action::Claim,
        Action::Release,
        Action::Submit,
    ]);
    denied.push(catalog::WriteDiscussion::NAME);
    let (tools, instructions) = served_tools(host_without(&repository, &denied)).await;
    let instructions = without_first_calls(&instructions);
    let change = tools.iter().find(|tool| tool.name == "change").unwrap();
    let actions: Vec<&str> = change.input_schema["properties"]["action"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|action| action.as_str())
        .filter(|action| Action::parse(action).is_some())
        .collect();
    assert_eq!(actions, ["create", "reply"]);
    assert_eq!(
        change.description.as_deref(),
        CHANGE_CREATE_REPLY.strip_prefix("- change: ")
    );
    let lines = [
        GET,
        SEARCH,
        API,
        TRACE,
        CHANGE_CREATE_REPLY,
        CREATE,
        REPLY,
        DISCUSSIONS,
        DISCUSSION,
    ];
    assert_eq!(
        instructions,
        expected(&[SEARCH_THEN_GET, CREATE_ONLY], &lines, true)
    );
}

/// The lines of a host that writes and reads, without notify and check, and without
/// the Discussion read that `without` names.
fn lines_without(without: &str) -> Vec<&'static str> {
    [
        GET,
        SEARCH,
        API,
        TRACE,
        CHANGE_RECORD_AND_DISCUSSION,
        CREATE,
        UPDATE,
        ANSWER,
        CLAIM,
        RELEASE,
        SUBMIT,
        DISCUSS,
        REPLY,
        DISCUSSIONS,
        DISCUSSION,
    ]
    .into_iter()
    .filter(|line| *line != without)
    .collect()
}

#[tokio::test]
async fn a_host_without_the_discussion_list_gives_no_discussions_line() {
    let repository = Repository::new("The shared graph is readable.");
    let host = host_without(&repository, &[catalog::ListDiscussions::NAME]);
    let (names, instructions) = served(host).await;
    assert!(names.iter().any(|name| name == "discussions"), "{names:?}");
    assert_eq!(
        instructions,
        expected(
            &[SEARCH_THEN_GET, CREATE_TYPE],
            &lines_without(DISCUSSIONS),
            true
        )
    );
}

#[tokio::test]
async fn a_host_with_the_discussion_list_and_without_one_discussion_gives_no_discussion_line() {
    let repository = Repository::new("The shared graph is readable.");
    let host = host_without(&repository, &[catalog::GetDiscussionConversation::NAME]);
    let (names, instructions) = served(host).await;
    assert!(names.iter().any(|name| name == "discussions"), "{names:?}");
    assert_eq!(
        instructions,
        expected(
            &[SEARCH_THEN_GET, CREATE_TYPE],
            &lines_without(DISCUSSION),
            true
        )
    );
}

#[tokio::test]
async fn a_host_with_only_api_gives_only_the_api_line() {
    let (names, instructions) = served(StatementHost::default()).await;
    assert_eq!(names, ["api"]);
    assert_eq!(instructions, expected(&[], &[API], false));
}
