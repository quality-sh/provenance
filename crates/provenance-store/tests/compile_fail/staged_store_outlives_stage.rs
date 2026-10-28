use provenance_store::repository::Repository;

fn main() {
    let repository = Repository::open("repo", "default").unwrap();
    let mut escaped = None;
    repository
        .stage(false, None, |staged| {
            escaped = Some(staged);
            Ok(())
        })
        .unwrap();
    let _ = escaped.map(|store| store.manifest());
}
