use super::*;

fn navigation() -> Navigation {
    Navigation::waiting(Duration::from_millis(100))
}

fn show(navigation: &Navigation) -> tokio::task::JoinHandle<Delivery> {
    let navigation = navigation.clone();
    tokio::spawn(async move {
        navigation
            .show("req_example".to_owned(), Some("rule_example".to_owned()))
            .await
    })
}

fn acknowledge(page: &Page, link: &ShowRecord) {
    page.receive(&serde_json::json!({"type": "ack", "requestId": link.request}).to_string());
}

#[tokio::test]
/// Implementation aid: with no connected page the browser gets the link.
async fn a_link_without_a_page_reaches_no_page() {
    assert_eq!(navigation().show("req_example".to_owned(), None).await, Delivery::NoPage);
}

#[tokio::test]
/// Implementation aid: the newest page shows the link and only it can acknowledge.
async fn the_newest_page_acknowledges_the_link() {
    let navigation = navigation();
    let older = navigation.attach();
    let mut newer = navigation.attach();
    let delivery = show(&navigation);
    let link = newer.links.recv().await.unwrap();
    assert_eq!(link.focus.as_deref(), Some("rule_example"));
    acknowledge(&older, &link);
    acknowledge(&newer, &link);

    assert_eq!(delivery.await.unwrap(), Delivery::Shown);
}

#[tokio::test]
/// Implementation aid: a page that keeps the link without an answer ends the wait as waiting.
async fn a_page_without_an_answer_keeps_the_link() {
    let navigation = navigation();
    let mut page = navigation.attach();
    let delivery = show(&navigation);
    page.links.recv().await.unwrap();

    assert_eq!(delivery.await.unwrap(), Delivery::Waiting);
    assert!(navigation.inner.lock().unwrap().waiting.is_empty());
}

#[tokio::test]
/// Implementation aid: a newer link ends the wait of the link it replaces at once.
async fn a_newer_link_replaces_the_waiting_one() {
    let navigation = Navigation::waiting(Duration::from_secs(60));
    let mut page = navigation.attach();
    let replaced = show(&navigation);
    page.links.recv().await.unwrap();
    let newest = show(&navigation);
    let link = page.links.recv().await.unwrap();

    assert_eq!(replaced.await.unwrap(), Delivery::Replaced);
    acknowledge(&page, &link);
    assert_eq!(newest.await.unwrap(), Delivery::Shown);
}

#[tokio::test]
/// Implementation aid: a page that disconnects before it answers leaves the link to the browser.
async fn a_disconnected_page_reaches_no_page() {
    let navigation = Navigation::waiting(Duration::from_secs(60));
    let mut page = navigation.attach();
    let delivery = show(&navigation);
    page.links.recv().await.unwrap();
    drop(page);

    assert_eq!(delivery.await.unwrap(), Delivery::NoPage);
}
