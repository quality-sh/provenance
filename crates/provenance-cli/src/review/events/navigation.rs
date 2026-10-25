//! Review links that a connected review page shows, and their acknowledgements.
use provenance_macros::rule;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

/// A page acknowledges a shown record at once. A page that holds the link back, for example
/// while a draft is open, must not hold the caller.
const ACKNOWLEDGEMENT_WAIT: Duration = Duration::from_secs(5);

/// What happened to a review link that the host sent to a connected page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Delivery {
    /// The page showed the record and acknowledged it.
    Shown,
    /// The page has the link and did not show it before the wait ended.
    Waiting,
    /// A newer link reached the same page before this link was shown.
    Replaced,
    /// No connected page has the link, or the page disconnected before it acknowledged it.
    NoPage,
}

/// Asks a connected page to show a record.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename = "show-record")]
pub(super) struct ShowRecord {
    #[serde(rename = "requestId")]
    request: String,
    #[serde(rename = "rootId")]
    root: String,
    #[serde(rename = "focusId", skip_serializing_if = "Option::is_none")]
    focus: Option<String>,
}

/// A frame from a page.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum PageFrame {
    Ack {
        #[serde(rename = "requestId")]
        request_id: String,
    },
}

#[derive(Clone)]
pub(super) struct Navigation {
    inner: Arc<Mutex<Pages>>,
    wait: Duration,
}

impl Default for Navigation {
    fn default() -> Self {
        Self::waiting(ACKNOWLEDGEMENT_WAIT)
    }
}

#[derive(Default)]
struct Pages {
    next: u64,
    /// Connected pages, oldest first.
    open: Vec<(u64, mpsc::UnboundedSender<ShowRecord>)>,
    /// Requests by id, with the page that must acknowledge each one.
    waiting: HashMap<String, (u64, oneshot::Sender<Delivery>)>,
}

/// One connected page. Its review links stop when it disconnects.
pub(super) struct Page {
    id: u64,
    pub links: mpsc::UnboundedReceiver<ShowRecord>,
    navigation: Navigation,
}

impl Navigation {
    /// Waits at most `wait` for each acknowledgement.
    fn waiting(wait: Duration) -> Self {
        Self {
            inner: Arc::default(),
            wait,
        }
    }

    pub fn attach(&self) -> Page {
        let (sender, links) = mpsc::unbounded_channel();
        let id = {
            let mut pages = self.inner.lock().unwrap();
            pages.next += 1;
            let id = pages.next;
            pages.open.push((id, sender));
            id
        };
        Page {
            id,
            links,
            navigation: self.clone(),
        }
    }

    /// Shows the record in the most recently connected page and waits a bounded time for its
    /// acknowledgement.
    ///
    /// The wait also ends when the page disconnects, when a newer link reaches the same page,
    /// or when the caller drops the request.
    #[rule("rule_review_link_reuses_connected_page")]
    pub async fn show(&self, root_id: String, focus_id: Option<String>) -> Delivery {
        let request_id = uuid::Uuid::new_v4().simple().to_string();
        let (acknowledge, acknowledged) = oneshot::channel();
        {
            let mut pages = self.inner.lock().unwrap();
            let Some((page, sender)) = pages.open.last() else {
                return Delivery::NoPage;
            };
            let page = *page;
            let link = ShowRecord {
                request: request_id.clone(),
                root: root_id,
                focus: focus_id,
            };
            if sender.send(link).is_err() {
                return Delivery::NoPage;
            }
            // The page shows only the newest link, so earlier waits on it end as replaced.
            let replaced: Vec<String> = pages
                .waiting
                .iter()
                .filter(|(_, (waiting, _))| *waiting == page)
                .map(|(request, _)| request.clone())
                .collect();
            for request in replaced {
                if let Some((_, earlier)) = pages.waiting.remove(&request) {
                    let _ = earlier.send(Delivery::Replaced);
                }
            }
            pages
                .waiting
                .insert(request_id.clone(), (page, acknowledge));
        }
        let _cancel = Waiting {
            navigation: self,
            request_id,
        };
        match tokio::time::timeout(self.wait, acknowledged).await {
            Ok(Ok(delivery)) => delivery,
            // The page disconnected and dropped the request.
            Ok(Err(_)) => Delivery::NoPage,
            Err(_) => Delivery::Waiting,
        }
    }
}

impl Page {
    /// Applies a page frame. Only the page that received a request can acknowledge it.
    pub fn receive(&self, text: &str) {
        let Ok(PageFrame::Ack { request_id }) = serde_json::from_str(text) else {
            return;
        };
        let mut pages = self.navigation.inner.lock().unwrap();
        if pages
            .waiting
            .get(&request_id)
            .is_some_and(|(page, _)| *page == self.id)
        {
            if let Some((_, acknowledge)) = pages.waiting.remove(&request_id) {
                let _ = acknowledge.send(Delivery::Shown);
            }
        }
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        let mut pages = self.navigation.inner.lock().unwrap();
        pages.open.retain(|(page, _)| *page != self.id);
        // Dropping the senders ends the waits of this page's requests.
        pages.waiting.retain(|_, (page, _)| *page != self.id);
    }
}

/// Removes a request when its caller stops waiting.
struct Waiting<'a> {
    navigation: &'a Navigation,
    request_id: String,
}

impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.navigation
            .inner
            .lock()
            .unwrap()
            .waiting
            .remove(&self.request_id);
    }
}

#[cfg(test)]
#[path = "navigation_tests.rs"]
mod tests;
