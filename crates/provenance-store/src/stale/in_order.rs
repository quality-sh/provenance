//! Runs one task for each item on a few threads and keeps the item order.

use std::sync::atomic::{AtomicUsize, Ordering};

/// The results of `task` for each item, in item order. Each thread takes
/// one run of at least `per_thread` items; a short list runs on the
/// calling thread. A panic in a task resumes on the calling thread.
pub fn map<T: Send, R: Send>(
    items: Vec<T>,
    per_thread: usize,
    task: impl Fn(T) -> R + Sync,
) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(items.len() / per_thread.max(1))
        .max(1);
    if threads == 1 {
        return items.into_iter().map(task).collect();
    }
    let size = items.len().div_ceil(threads);
    let mut runs = Vec::with_capacity(threads);
    let mut rest = items;
    while !rest.is_empty() {
        let tail = rest.split_off(size.min(rest.len()));
        runs.push(rest);
        rest = tail;
    }
    let task = &task;
    std::thread::scope(|scope| {
        // Every thread starts before the first join.
        let mut handles = Vec::with_capacity(runs.len());
        for run in runs {
            handles.push(scope.spawn(move || run.into_iter().map(task).collect::<Vec<_>>()));
        }
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    })
}

/// The results of `task` for each item, in item order, or the error of
/// the first item in item order that fails. Up to one thread for each
/// item takes the items in order. After a failure, no thread starts an
/// item that comes after the failed item. Each item before the first
/// failed item runs, so the error is the error that one run in item order
/// gives. A panic in a task resumes on the calling thread.
pub fn try_map<T: Sync, R: Send, E: Send>(
    items: &[T],
    task: impl Fn(&T) -> Result<R, E> + Sync,
) -> Result<Vec<R>, E> {
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(items.len())
        .max(1);
    if threads == 1 {
        return items.iter().map(task).collect();
    }
    let next = AtomicUsize::new(0);
    let failed = AtomicUsize::new(usize::MAX);
    let worker = || {
        let mut done = Vec::new();
        loop {
            let index = next.fetch_add(1, Ordering::SeqCst);
            if index >= items.len() || index > failed.load(Ordering::SeqCst) {
                return done;
            }
            let result = task(&items[index]);
            if result.is_err() {
                failed.fetch_min(index, Ordering::SeqCst);
            }
            done.push((index, result));
        }
    };
    let mut done = std::thread::scope(|scope| {
        // Every thread starts before the first join.
        let mut handles = Vec::with_capacity(threads);
        for _ in 0..threads {
            handles.push(scope.spawn(worker));
        }
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect::<Vec<_>>()
    });
    done.sort_unstable_by_key(|(index, _)| *index);
    done.into_iter().map(|(_, result)| result).collect()
}

#[cfg(test)]
mod tests {
    // Implementation aids: the order and the stop after a failure are not
    // visible through a public surface, which gives one answer or one
    // refusal.
    #[test]
    fn results_keep_the_item_order() {
        let items = (0..1000).collect::<Vec<u32>>();
        let expected = items.iter().map(|item| item * 3).collect::<Vec<_>>();
        for per_thread in [1, 7, 64, 2000] {
            assert_eq!(
                super::map(items.clone(), per_thread, |item| item * 3),
                expected
            );
        }
        assert_eq!(
            super::map(Vec::<u32>::new(), 1, |item| item),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn try_map_gives_the_first_failure_in_item_order_and_stops_starting_items() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let items = (0..1000).collect::<Vec<usize>>();
        assert_eq!(
            super::try_map(&items, |item| Ok::<_, usize>(item * 3)),
            Ok(items.iter().map(|item| item * 3).collect::<Vec<_>>())
        );
        for _ in 0..20 {
            let started = AtomicUsize::new(0);
            let result = super::try_map(&items, |&item| {
                started.fetch_add(1, Ordering::SeqCst);
                if item == 10 || item == 20 {
                    return Err(item);
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
                Ok(item)
            });
            assert_eq!(result, Err(10));
            let started = started.into_inner();
            assert!(started >= 11, "each item before the failure runs");
            assert!(started < items.len(), "{started} items started");
        }
        assert_eq!(
            super::try_map(&Vec::<usize>::new(), |item| Ok::<_, ()>(*item)),
            Ok(Vec::new())
        );
    }
}
