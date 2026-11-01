//! Finds the review document of a record and builds reviewer-facing URLs from verified hosts.

use crate::{launch_key, RepositoryScope, LAUNCH_CODE_ROUTE};
use anyhow::Context as _;
use provenance_core::{NodeType, StableId};
use provenance_macros::rule;
use provenance_store::operations::{queries, read_policy::ReadPolicy};
use provenance_transport::local_host::DiscoveredLocalHost;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use url::Url;

/// A record that a write changed or that a caller wants to see in review.
#[derive(Clone, Debug)]
pub struct AffectedReviewRecord {
    pub kind: NodeType,
    pub id: String,
}

/// The review document of a record and the verified host that serves it, if one runs.
#[derive(Clone, Debug)]
pub struct ReviewLocation {
    pub root: StableId,
    pub focus: Option<String>,
    pub host: Option<DiscoveredLocalHost>,
}

/// What a connected page did with a review link, as the host reports it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PageResult {
    Shown,
    Waiting,
    Replaced,
    NoPage,
}

// The host ends its wait for the page after five seconds; this limit covers a host that does
// not answer.
const NAVIGATION_TIMEOUT: Duration = Duration::from_secs(15);

/// Finds the review document that holds the record and the host that can show it.
pub async fn review_location(
    place: &RepositoryScope,
    record: &AffectedReviewRecord,
) -> anyhow::Result<Option<ReviewLocation>> {
    let roots = queries::containing_review_documents(
        &place.open()?,
        ReadPolicy::default(),
        record.kind,
        &StableId::new(&record.id)?,
    )
    .await?
    .result;
    let Some(root) = roots.first() else {
        return Ok(None);
    };
    Ok(Some(ReviewLocation {
        root: root.clone(),
        focus: (root.as_str() != record.id).then(|| record.id.clone()),
        host: place.host()?,
    }))
}

impl ReviewLocation {
    /// The review URL of the document, with the record in focus, when a host runs.
    pub fn plain_link(&self) -> Option<Url> {
        self.host
            .as_ref()
            .map(|host| build_url(host.endpoint(), self.root.as_str(), self.focus.as_deref()))
    }

    /// Adds a single-use launch code that signs the review page in when the link opens.
    #[rule("rule_review_link_opens_signed_in")]
    pub fn launch_link(&self) -> anyhow::Result<Option<Url>> {
        let Some((host, mut link)) = self.host.as_ref().zip(self.plain_link()) else {
            return Ok(None);
        };
        link.set_fragment(Some(&format!("launch={}", issue_launch_code(host)?)));
        Ok(Some(link))
    }

    /// Asks the host to show the record in a connected page. Only a link that no page received
    /// uses the browser.
    pub async fn show_in_connected_page(&self) -> PageResult {
        let Some(host) = &self.host else {
            return PageResult::NoPage;
        };
        let Ok(key) = launch_key::read(host.instance_nonce()) else {
            return PageResult::NoPage;
        };
        let mut url = host.endpoint().clone();
        url.set_path("/review-navigation");
        let body = serde_json::json!({
            "launchKey": key,
            "rootId": self.root.as_str(),
            "focusId": self.focus,
        });
        tokio::task::spawn_blocking(move || {
            ureq::AgentBuilder::new()
                .redirects(0)
                .timeout(NAVIGATION_TIMEOUT)
                .build()
                .post(url.as_str())
                .set("Content-Type", "application/json")
                .send_string(&body.to_string())
                .ok()
                .and_then(|response| response.into_string().ok())
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .and_then(|reply| serde_json::from_value(reply["result"].clone()).ok())
                .unwrap_or(PageResult::NoPage)
        })
        .await
        .unwrap_or(PageResult::NoPage)
    }
}

/// Gets a single-use launch code from the host, with the key that only its user can read.
pub fn issue_launch_code(host: &DiscoveredLocalHost) -> anyhow::Result<String> {
    let key = launch_key::read(host.instance_nonce())?;
    let mut url = host.endpoint().clone();
    url.set_path(LAUNCH_CODE_ROUTE);
    let response = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout(Duration::from_secs(5))
        .build()
        .post(url.as_str())
        .set("Content-Type", "application/json")
        .send_string(&serde_json::json!({ "launchKey": key }).to_string())
        .context("the review host did not issue a launch code")?;
    let body: Value = serde_json::from_str(&response.into_string()?)?;
    let code = body["code"]
        .as_str()
        .filter(|code| launch_key::is_secret(code))
        .context("the review host sent a launch code that is not valid")?;
    Ok(code.to_owned())
}

/// Puts the review document and optional focused record into the review URL.
#[rule("rule_review_link_needs_no_record_id")]
fn build_url(endpoint: &Url, root: &str, focus: Option<&str>) -> Url {
    let mut url = endpoint.clone();
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("root", root);
        if let Some(focus) = focus {
            query.append_pair("focus", focus);
        }
    }
    url
}
