use super::*;

pub(crate) struct TestStore;

#[async_trait::async_trait]
impl SecureStore for TestStore {
    async fn get(&self, _namespace: &str, _key: &str) -> Result<Option<String>, PortError> {
        Ok(None)
    }

    async fn set(&self, _namespace: &str, _key: &str, _value: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn delete(&self, _namespace: &str, _key: &str) -> Result<(), PortError> {
        Ok(())
    }
}

pub(crate) fn manager(directory: &TempDir) -> ConfigManager<TestStore> {
    manager_at(directory.path())
}

pub(crate) fn manager_at(home: &Path) -> ConfigManager<TestStore> {
    ConfigManager::with_home_and_store(home.to_path_buf(), TestStore)
        .expect("isolated config manager")
}

pub(crate) fn spawn_test_child(test_name: &str, environment: &[(&str, &str)]) -> Child {
    let mut command = Command::new(env::current_exe().expect("test binary path"));
    command.arg(test_name).arg("--nocapture");
    for (name, value) in environment {
        command.env(name, value);
    }
    command.spawn().expect("spawn lock protocol test child")
}

pub(crate) async fn wait_for_marker(path: &Path) {
    timeout(Duration::from_secs(30), async {
        loop {
            if path.exists() {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for subprocess marker");
}

pub(crate) async fn wait_child(mut child: Child) -> ExitStatus {
    spawn_blocking(move || child.wait())
        .await
        .expect("join lock protocol child waiter")
        .expect("wait for lock protocol child")
}

pub(crate) fn identity(engine: &str, instance: &str) -> CoreIdentity {
    CoreIdentity::new(
        EngineId::new(engine).expect("valid engine id"),
        CoreInstanceId::new(instance).expect("valid core instance id"),
        None,
    )
}

pub(crate) fn stopped_snapshot(identity: CoreIdentity, generation: u64) -> CoreLifecycleSnapshot {
    CoreLifecycleSnapshot {
        identity: Some(identity),
        lifecycle: CoreLifecycle::Stopped,
        generation,
        session_token: None,
        revision: generation,
        failure: None,
    }
}
