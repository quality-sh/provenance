use provenance_store::repository::Repository;

fn main() {
    let repository = Repository::open("repo", "default").unwrap();
    let escaped = repository.stage(false, None, |staged| Ok(staged)).unwrap();
    let _ = escaped.manifest();
}
