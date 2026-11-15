//! The server instructions name only the commands that the server shows. These tests
//! have no Rule binding: the Rule that states which commands the instructions name
//! waits for the approval of the owner.
#![cfg(feature = "test-fixture")]

use crate::support::{fixture_host, mcp::Session, records::Repository};
use provenance_porcelain::action::Action;
use provenance_store::operations::catalog::{self, Operation as _};
use provenance_transport::StatementHost;

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
import, dictionary import, or swarm-backtrace land on the repository files. A call that \
lacks a field that the action and record kind need gets the schema of their fields.";
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

/// The first steps that a server adds when it lists search and get.
const FIRST_STEPS: &str = "\n\nFor a focused start, call search with {\"text\":\"<text>\"}, \
then get with {\"target\":\"<id>\",\"view\":\"children\",\"depth\":<N>}.";

/// The expected instructions: the domain text, the opening sentences on one line, the
/// access sentence, the command lines, and the first steps when `first_steps` is true.
fn expected(opening: &[&str], lines: &[&str], first_steps: bool) -> String {
    let mut text = DOMAIN.to_owned();
    if !opening.is_empty() {
        text = text + &opening.join(" ") + "\n";
    }
    text = text + ACCESS + &lines.join("\n");
    if first_steps {
        text += FIRST_STEPS;
    }
    text
}

/// The tool names and the instructions of one served host.
async fn served(host: StatementHost) -> (Vec<String>, String) {
    let session = Session::start(host).await;
    let instructions = session
        .client
        .peer_info()
        .and_then(|info| info.instructions.clone())
        .expect("server instructions");
    let names = session
        .client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|tool| tool.name.into_owned())
        .collect();
    session.close().await;
    (names, instructions)
}

/// A host that writes the repository, without the operations that `denied` names.
fn host_without(repository: &Repository, denied: &[&str]) -> StatementHost {
    let access = fixture_host::access(repository).allow_writes();
    StatementHost::with_fixture_access(
        denied
            .iter()
            .fold(access, |access, operation| access.deny_operation(operation)),
    )
}

/// The operations of the record actions in `actions`.
fn operations_of(actions: &[Action]) -> Vec<&'static str> {
    actions
        .iter()
        .flat_map(|action| provenance_contract::target_definitions(*action))
        .map(|(_, definition)| definition.name)
        .collect()
}

#[tokio::test]
async fn a_host_with_every_tool_gives_the_shared_guidance_and_first_steps() {
    let repository = Repository::new("The shared graph is readable.");
    let (names, instructions) = served(fixture_host::full(&repository)).await;
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
    assert_eq!(
        instructions,
        provenance_porcelain::guidance::guidance().guidance + FIRST_STEPS
    );
    assert_eq!(instructions, provenance_mcp::instructions());
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
    let lines = [API, CHANGE, CREATE, DISCUSS, REPLY];
    assert_eq!(
        served(host).await,
        (
            vec!["api".to_owned(), "change".to_owned()],
            expected(&[CREATE_TYPE], &lines, false)
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
    let lines = [API, CHANGE, CREATE, DISCUSSIONS, DISCUSSION, DISCUSS, REPLY];
    assert_eq!(
        served(host).await,
        (
            vec![
                "api".to_owned(),
                "change".to_owned(),
                "discussions".to_owned()
            ],
            expected(&[CREATE_TYPE], &lines, false)
        )
    );
}

#[tokio::test]
async fn a_host_with_get_and_without_search_gives_no_first_steps() {
    let repository = Repository::new("The shared graph is readable.");
    let host = host_without(&repository, &[catalog::Search::NAME]);
    let lines = [
        GET,
        TRACE,
        API,
        CHANGE,
        CREATE,
        UPDATE,
        ANSWER,
        CLAIM,
        RELEASE,
        SUBMIT,
        DISCUSSIONS,
        DISCUSSION,
        DISCUSS,
        REPLY,
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
    let lines = [GET, TRACE, SEARCH, CHECK, API, DISCUSSIONS, DISCUSSION];
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
async fn a_change_tool_with_create_and_reply_gives_only_their_action_lines() {
    let repository = Repository::new("The shared graph is readable.");
    let mut denied = operations_of(&[
        Action::Update,
        Action::Answer,
        Action::Claim,
        Action::Release,
        Action::Submit,
    ]);
    denied.push(catalog::WriteDiscussion::NAME);
    let session = Session::start(host_without(&repository, &denied)).await;
    let tools = session.client.list_all_tools().await.unwrap();
    session.close().await;
    let change = tools.iter().find(|tool| tool.name == "change").unwrap();
    let actions: Vec<&str> = change.input_schema["properties"]["action"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|action| action.as_str())
        .filter(|action| Action::parse(action).is_some())
        .collect();
    assert_eq!(actions, ["create", "reply"]);
    let lines = [
        GET,
        TRACE,
        SEARCH,
        API,
        CHANGE,
        CREATE,
        DISCUSSIONS,
        DISCUSSION,
        REPLY,
    ];
    assert_eq!(
        served(host_without(&repository, &denied)).await.1,
        expected(&[SEARCH_THEN_GET, CREATE_TYPE], &lines, true)
    );
}

#[tokio::test]
async fn a_host_with_only_api_gives_only_the_api_line() {
    let (names, instructions) = served(StatementHost::default()).await;
    assert_eq!(names, ["api"]);
    assert_eq!(instructions, expected(&[], &[API], false));
}
