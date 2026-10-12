use super::{read_optional, read_optional_bytes, remove_optional, write_document};
use infiltrator_ports::error::PortError;
use std::fs::{create_dir, read_dir, read_to_string, write};
use tempfile::TempDir;

#[tokio::test]
async fn durable_document_write_replaces_a_complete_value_and_cleans_temporaries() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("profiles/main.json");

    write_document(&path, "first").await.unwrap();
    write_document(&path, "second complete value")
        .await
        .unwrap();

    assert_eq!(read_to_string(&path).unwrap(), "second complete value");
    let mut entries = read_dir(path.parent().unwrap()).unwrap();
    assert_eq!(entries.next().unwrap().unwrap().file_name(), "main.json");
    assert!(entries.next().is_none());
}

#[tokio::test]
async fn optional_reads_distinguish_absence_from_invalid_text() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("state.json");
    assert_eq!(read_optional(&path).await.unwrap(), None);
    assert_eq!(read_optional_bytes(&path).await.unwrap(), None);

    write(&path, [0xff]).unwrap();
    assert_eq!(read_optional_bytes(&path).await.unwrap(), Some(vec![0xff]));
    assert!(matches!(read_optional(&path).await, Err(PortError::Io(_))));
}

#[tokio::test]
async fn optional_removal_is_idempotent_and_preserves_non_file_targets() {
    let directory = TempDir::new().unwrap();
    let path = directory.path().join("state.json");
    remove_optional(&path).await.unwrap();

    create_dir(&path).unwrap();
    write(path.join("sentinel"), b"keep").unwrap();
    assert!(matches!(
        remove_optional(&path).await,
        Err(PortError::Io(_))
    ));
    assert_eq!(std::fs::read(path.join("sentinel")).unwrap(), b"keep");
}
