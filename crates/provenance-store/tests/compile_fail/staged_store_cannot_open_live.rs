use provenance_store::repository::Repository;

fn main() {
    let repository = Repository::open("repo", "default").unwrap();
    let _ = repository.stage(false, None, |staged| {
        let live = staged.repository().unwrap().store();
        live.manifest()
    });
}
