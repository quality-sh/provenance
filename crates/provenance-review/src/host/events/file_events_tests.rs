use super::*;
use notify::event::{DataChange, EventKind, Flag, ModifyKind};
use provenance_macros::verifies;

fn layout() -> ProvenanceLayout {
    ProvenanceLayout::new("/repository")
}

fn write_event(layout: &ProvenanceLayout) -> Event {
    Event::new(EventKind::Modify(ModifyKind::Data(DataChange::Content)))
        .add_path(layout.manifest_path().into_std_path_buf())
}

fn rescan_event() -> Event {
    Event::new(EventKind::Other).set_flag(Flag::Rescan)
}

/// Calls the handler with a write event at each `interval`, from the start of the test.
fn write_every(
    interval: Duration,
    mut handler: impl FnMut(notify::Result<Event>) + Send + 'static,
) {
    let layout = layout();
    tokio::spawn(async move {
        loop {
            handler(Ok(write_event(&layout)));
            tokio::time::sleep(interval).await;
        }
    });
}

#[tokio::test(start_paused = true)]
async fn a_quiet_time_ends_the_wait() {
    let (mut handler, mut events) = FileEvents::new(layout());
    let (_stop, mut stopped) = watch::channel(false);
    handler(Ok(write_event(&layout())));
    let start = Instant::now();

    events.settle(&mut stopped).await;

    assert_eq!(start.elapsed(), QUIET);
}

#[tokio::test(start_paused = true)]
#[verifies("rule_review_host_reads_settled_state", examples)]
async fn events_that_continue_do_not_move_the_time_limit() {
    let (handler, mut events) = FileEvents::new(layout());
    let (_stop, mut stopped) = watch::channel(false);
    write_every(Duration::from_millis(100), handler);
    let start = Instant::now();

    events.settle(&mut stopped).await;

    assert_eq!(start.elapsed(), SETTLE_LIMIT);
}

#[tokio::test(start_paused = true)]
#[verifies("rule_review_host_reads_settled_state", examples)]
async fn a_host_stop_ends_the_wait() {
    let (handler, mut events) = FileEvents::new(layout());
    let (stop, mut stopped) = watch::channel(false);
    write_every(Duration::from_millis(100), handler);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        stop.send(true).unwrap();
    });
    let start = Instant::now();

    events.settle(&mut stopped).await;

    assert_eq!(start.elapsed(), Duration::from_secs(1));
}

#[tokio::test(start_paused = true)]
async fn a_rescan_during_the_wait_marks_the_events_as_lost() {
    let (mut handler, mut events) = FileEvents::new(layout());
    let (_stop, mut stopped) = watch::channel(false);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        handler(Ok(rescan_event()));
        std::future::pending::<()>().await;
    });

    let notice = events.settle(&mut stopped).await;

    assert!(notice.lost);
}

#[tokio::test(start_paused = true)]
async fn an_overflow_during_the_wait_is_given_one_time() {
    let (mut handler, mut events) = FileEvents::new(layout());
    let (_stop, mut stopped) = watch::channel(false);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        for _ in 0..300 {
            handler(Ok(write_event(&layout())));
        }
        std::future::pending::<()>().await;
    });

    let notice = events.settle(&mut stopped).await;
    let next = tokio::time::timeout(SETTLE_LIMIT, events.next(&mut stopped)).await;

    assert!(notice.lost);
    assert!(next.is_err(), "the overflow gave a second notice");
}
