    use super::*;
    use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
    use provenance_store::layout::ProvenanceLayout;

    #[test]
    #[provenance_macros::verifies("rule_init_validates_planned_repository", examples)]
    fn planned_manifest_validation_runs_publication_recovery_before_reading_state() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(repo.clone());
        std::fs::create_dir_all(layout.scopes_dir()).unwrap();
        std::fs::write(layout.manifest_path(), "not the planned manifest").unwrap();
        std::fs::create_dir_all(layout.cache_dir()).unwrap();
        std::fs::write(layout.publication_marker_path(), "not a publication marker").unwrap();
        let manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );

        let error = validate_repository_with_manifest(&repo, &manifest).unwrap_err();

        assert!(format!("{error:#}").contains("expected ident"));
        assert!(layout.publication_lock_path().exists());
        assert!(layout.import_transactions_dir().exists());
    }

    #[cfg(unix)]
    #[test]
    #[provenance_macros::verifies("rule_init_validates_planned_repository", examples)]
    fn planned_manifest_validation_refuses_a_symlinked_publication_cache() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
        let outside = directory.path().join("outside");
        let layout = ProvenanceLayout::new(repo.clone());
        std::fs::create_dir_all(layout.provenance_dir()).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, layout.cache_dir()).unwrap();
        let manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );

        let error = validate_repository_with_manifest(&repo, &manifest).unwrap_err();

        assert!(format!("{error:#}").contains("symlink component"));
    }

    #[test]
    fn planned_manifest_validation_locks_an_existing_state_tree() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(repo.clone());
        std::fs::create_dir_all(layout.scopes_dir()).unwrap();
        let manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );

        validate_repository_with_manifest(&repo, &manifest).unwrap();

        assert!(layout.publication_lock_path().exists());
    }

    #[test]
    fn planned_manifest_validation_keeps_a_new_repository_read_only() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
        let layout = ProvenanceLayout::new(repo.clone());
        let manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );

        validate_repository_with_manifest(&repo, &manifest).unwrap();

        assert!(!layout.provenance_dir().exists());
    }

    #[tokio::test]
    async fn scoped_check_does_not_read_another_scope() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(repo.clone());
        std::fs::create_dir_all(layout.state_dir()).unwrap();
        let mut manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );
        manifest.scopes.push(provenance_core::Scope {
            id: ScopeId::new("other").unwrap(),
            path_prefix: RepoPathPrefix::new("other"),
        });
        std::fs::write(
            layout.manifest_path(),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let foreign = layout
            .scopes_dir()
            .join("other")
            .join("requirements")
            .join("req.jsonl");
        std::fs::create_dir_all(foreign.parent().unwrap()).unwrap();
        std::fs::write(foreign, "not JSON\n").unwrap();

        let port = RepositoryCheckPort::new(repo, false, None);
        let run = port.run(Category::Graph, Some("default")).await.unwrap();

        let CategoryRun::Graph { findings, .. } = run else {
            panic!("graph run")
        };
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn binding_check_refuses_an_absent_selected_scope() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(repo.clone());
        std::fs::create_dir_all(layout.state_dir()).unwrap();
        let manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );
        std::fs::write(layout.manifest_path(), serde_json::to_vec(&manifest).unwrap()).unwrap();

        let port = RepositoryCheckPort::new(repo, false, None);
        let error = port.run(Category::Bindings, Some("absent")).await.unwrap_err();

        assert!(error.contains("scope absent does not exist"), "{error}");
    }
