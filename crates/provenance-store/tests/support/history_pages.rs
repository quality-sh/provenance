//! A read of the history of `req_a` that follows `next_cursor` from the
//! first page to the end of the history.

use provenance_store::operations::read_operations::{InProcessReadOperations, ReadOperations};
use serde_json::{json, Value};

/// The most pages that one read of a history follows.
const FOLLOWS: usize = 10;

/// The ids of the entries of each page of `review-history` for `req_a`
/// through `operations` of the target `folder`, each id given to `name`.
/// The read follows `next_cursor` from the first page. A failed page gives
/// the pages before it and its error.
pub async fn follow(
    operations: &InProcessReadOperations,
    name: impl Fn(&str) -> String,
) -> (Vec<Vec<String>>, Option<Value>) {
    let mut request = json!({"record_kind": "requirement", "record_id": "req_a", "limit": 200});
    let mut pages = Vec::new();
    for _ in 0..FOLLOWS {
        let call = json!({
            "context": {"repository": "folder", "scope": "default"},
            "request": request,
        });
        let page = match operations.run("review-history", call).await {
            Ok(page) => page,
            Err(failure) => return (pages, Some(failure.error)),
        };
        let ids: Vec<String> = page["result"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| name(entry["id"].as_str().unwrap()))
            .collect();
        let next = &page["result"]["next_cursor"];
        assert!(
            !(ids.is_empty() && next.is_string()),
            "page {} is empty and gives a cursor; the cursor it was read with: {}",
            pages.len() + 1,
            request["cursor"]
        );
        pages.push(ids);
        if next.is_null() {
            return (pages, None);
        }
        request["cursor"] = next.clone();
    }
    panic!("the history has more than {FOLLOWS} pages");
}
