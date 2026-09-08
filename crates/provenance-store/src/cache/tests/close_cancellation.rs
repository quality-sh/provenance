//! Cancellation and panic must not release the lock during `SQLite` close.
use super::wal_behavior::wal_files;
use crate::cache::{open_cache, CacheConnection};
use crate::layout::ProvenanceLayout;
use fs2::FileExt;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

struct Release(Arc<(Mutex<bool>, Condvar)>);
impl Drop for Release {
    fn drop(&mut self) {
        let (mutex, cv) = &*self.0;
        *mutex.lock().unwrap() = true;
        cv.notify_all();
    }
}
struct PauseOnDrop {
    signal: Option<tokio::sync::oneshot::Sender<()>>,
    gate: Arc<(Mutex<bool>, Condvar)>,
}
impl Drop for PauseOnDrop {
    fn drop(&mut self) {
        if let Some(signal) = self.signal.take() {
            let _ = signal.send(());
        }
        let (mutex, cv) = &*self.gate;
        let mut released = mutex.lock().unwrap();
        while !*released {
            released = cv.wait(released).unwrap();
        }
        drop(released);
    }
}

async fn pause_physical_close(
    pool: &CacheConnection,
) -> (Release, tokio::sync::oneshot::Receiver<()>) {
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let (signal, started) = tokio::sync::oneshot::channel();
    let pause = PauseOnDrop {
        signal: Some(signal),
        gate: Arc::clone(&gate),
    };
    let mut connection = pool.pool().acquire().await.unwrap();
    let mut handle = connection.lock_handle().await.unwrap();
    // SQLx removes this handler during ConnectionState::drop, before the
    // SQLite handle field drops. The closure destructor pauses that close.
    handle.set_progress_handler(1, move || {
        std::hint::black_box(&pause);
        true
    });
    drop(handle);
    drop(connection);
    (Release(gate), started)
}

async fn cancellation_probe(panic_instead: bool) {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(
        camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap(),
    );
    let first = open_cache(&layout).await.unwrap();
    sqlx::query("CREATE TABLE sample(value INTEGER)")
        .execute(first.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO sample VALUES (1)")
        .execute(first.pool())
        .await
        .unwrap();
    let second = open_cache(&layout).await.unwrap();
    sqlx::query("SELECT * FROM sample")
        .fetch_all(second.pool())
        .await
        .unwrap();
    let (release, started) = pause_physical_close(&first).await;
    let (panic_now, panic_rx) = tokio::sync::oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        tokio::select! {
            result = first.close() => result.unwrap(),
            _ = panic_rx => panic!("verification: panic while the close future is pending"),
        }
    });
    tokio::time::timeout(Duration::from_secs(5), started)
        .await
        .unwrap()
        .unwrap();
    let path = layout.cache_db_path().with_extension("db.close.lock");
    let lock_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap();
    assert!(
        lock_file.try_lock_exclusive().is_err(),
        "first close must own the lock before cancellation"
    );
    if panic_instead {
        panic_now.send(()).unwrap();
        assert!(task.await.unwrap_err().is_panic());
    } else {
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        drop(panic_now);
    }
    let lock_released_early = lock_file.try_lock_exclusive().is_ok();
    println!("panic={panic_instead} first_physical_close_paused=true close_lock_released={lock_released_early}");
    if lock_released_early {
        FileExt::unlock(&lock_file).unwrap();
    }
    assert!(
        !lock_released_early,
        "the close must keep ownership after cancellation"
    );
    let next = second.close();
    tokio::pin!(next);
    let early = tokio::time::timeout(Duration::from_millis(50), &mut next).await;
    assert!(
        early.is_err(),
        "the second close must wait for SQLite to finish"
    );
    drop(release);
    tokio::time::timeout(Duration::from_secs(5), next)
        .await
        .unwrap()
        .unwrap();
    assert!(
        wal_files(&layout).is_empty(),
        "the -wal and -shm files remain"
    );
    println!("panic={panic_instead} close_order_preserved=true files_remaining=0");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_close_keeps_the_lock_until_sqlite_finishes() {
    cancellation_probe(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn panicking_close_keeps_the_lock_until_sqlite_finishes() {
    cancellation_probe(true).await;
}
