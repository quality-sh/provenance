//! Runs one task for each item on a few threads and keeps the item order.

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

#[cfg(test)]
mod tests {
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
}
