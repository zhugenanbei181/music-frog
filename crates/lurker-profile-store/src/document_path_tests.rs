use crate::layout::ProfileStoreLayout;
use crate::store::ProfileDocumentExtension;
use crate::test_core::{FIXTURE_ENGINE, TestCore};
use lurker_core_contract::profile_document_identity::ProfileId;
use lurker_core_ports::error::PortError;
use std::fs::write;
use tokio::fs::symlink;

#[tokio::test]
async fn document_paths_follow_the_injected_layout_and_extension() {
    let yaml_core = TestCore::new(ProfileDocumentExtension::Yaml);
    let json_core = TestCore::new(ProfileDocumentExtension::Json);
    let profile = ProfileId::new("main").unwrap();

    let yaml = yaml_core
        .core()
        .profile_document_path(&profile)
        .await
        .unwrap();
    let json = json_core
        .core()
        .profile_document_path(&profile)
        .await
        .unwrap();

    assert_eq!(yaml, yaml_core.config_dir().join("main.yaml"));
    assert_eq!(json, json_core.config_dir().join("main.json"));
}

#[tokio::test]
async fn document_paths_derive_every_engine_directory_from_the_layout() {
    let layout = ProfileStoreLayout {
        engine_dir_name: "engine-namespaces".to_owned(),
        ..ProfileStoreLayout::default()
    };
    let fixture = TestCore::with_layout(ProfileDocumentExtension::Yaml, layout);
    let profile = ProfileId::new("primary").unwrap();

    let path = fixture
        .core()
        .profile_document_path(&profile)
        .await
        .unwrap();

    assert_eq!(
        path,
        fixture
            .root()
            .join("engine-namespaces")
            .join(FIXTURE_ENGINE)
            .join("primary.yaml")
    );
}

#[tokio::test]
async fn document_paths_reject_relative_segments_before_access() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);

    for name in ["release..backup", "name.", "..leading"] {
        let error = fixture
            .core()
            .profile_document_path(&ProfileId::new(name).unwrap())
            .await
            .unwrap_err();
        assert!(matches!(error, PortError::Rejected(_)));
    }
    assert!(
        !fixture
            .root()
            .join(fixture.layout().engine_dir_name.as_str())
            .exists()
    );
}

#[tokio::test]
async fn existing_document_path_reports_missing_document_and_rejects_escape_symlink() {
    let fixture = TestCore::new(ProfileDocumentExtension::Yaml);
    let profile = ProfileId::new("main").unwrap();

    assert!(matches!(
        fixture
            .core()
            .existing_profile_document_path(&profile)
            .await,
        Err(PortError::NotFound(_))
    ));

    #[cfg(unix)]
    {
        let document = fixture
            .core()
            .profile_document_path(&profile)
            .await
            .unwrap();
        let outside = fixture.root().join("outside.yaml");
        write(&outside, "external").unwrap();
        symlink(&outside, &document).await.unwrap();
        assert!(matches!(
            fixture
                .core()
                .existing_profile_document_path(&profile)
                .await,
            Err(PortError::Rejected(_))
        ));
    }
}
