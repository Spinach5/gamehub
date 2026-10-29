use application::{ComponentEvent, PrefixPlan, install_component};
use launcher_core::{Arch, Runner, RunnerKind, RunnerSource};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::tempdir;
use uuid::Uuid;

fn test_runner(root: &Path) -> Runner {
    Runner {
        id: Uuid::new_v4(),
        kind: RunnerKind::Wine,
        name: "test".into(),
        binary_path: root.join("bin/wine"),
        root_dir: root.to_path_buf(),
        arch: Arch::Win64,
        source: RunnerSource::System,
        verified: true,
        version_string: None,
    }
}

fn write_script(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

fn initialized_plan(dir: &Path) -> PrefixPlan {
    let prefix = dir.join("prefix");
    fs::create_dir_all(&prefix).unwrap();
    fs::write(prefix.join("system.reg"), "#arch=win64\n").unwrap();
    PrefixPlan::new(
        prefix,
        test_runner(dir),
        Arch::Win64,
        dir.join("steam-client"),
    )
}

#[tokio::test]
async fn streams_output_and_writes_the_log() {
    let dir = tempdir().unwrap();
    let plan = initialized_plan(dir.path());
    let env_log = dir.path().join("env.txt");
    let winetricks = dir.path().join("winetricks");
    write_script(
        &winetricks,
        &format!(
            "#!/bin/sh\nprintf '%s|%s|%s\\n' \"$WINEPREFIX\" \"$1\" \"$2\" > '{}'\n\
             echo 'Executing w_do_call vcrun2019'\n\
             echo 'download failed' >&2\nexit 0\n",
            env_log.display()
        ),
    );

    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let log_path = dir.path().join("logs/winetricks.log");
    let outcome = install_component(
        plan.clone(),
        winetricks,
        "vcrun2019",
        log_path.clone(),
        events,
    )
    .await
    .unwrap();

    assert_eq!(outcome.verb, "vcrun2019");
    assert!(outcome.success);
    assert_eq!(outcome.exit_code, Some(0));

    assert_eq!(
        fs::read_to_string(env_log).unwrap().trim(),
        format!("{}|-q|vcrun2019", plan.path.display())
    );

    let mut started = false;
    let mut lines = Vec::new();
    while let Ok(event) = received.try_recv() {
        match event {
            ComponentEvent::Started { process_group } => {
                assert!(process_group > 0);
                started = true;
            }
            ComponentEvent::Line(line) => lines.push(line),
        }
    }
    assert!(started, "the UI needs the process group for cancel");
    assert!(lines.iter().any(|line| line.contains("w_do_call")));
    assert!(lines.iter().any(|line| line.contains("download failed")));

    let logged = fs::read_to_string(log_path).unwrap();
    assert!(logged.contains("Executing w_do_call vcrun2019"));
    assert!(logged.contains("download failed"));
}

#[tokio::test]
async fn failure_outcome_carries_the_exit_code() {
    let dir = tempdir().unwrap();
    let plan = initialized_plan(dir.path());
    let winetricks = dir.path().join("winetricks");
    write_script(&winetricks, "#!/bin/sh\necho boom\nexit 3\n");

    let (events, _received) = tokio::sync::mpsc::unbounded_channel();
    let outcome = install_component(
        plan,
        winetricks,
        "dotnet48",
        dir.path().join("winetricks.log"),
        events,
    )
    .await
    .unwrap();

    assert!(!outcome.success);
    assert_eq!(outcome.exit_code, Some(3));
}

#[tokio::test]
async fn rejects_invalid_verbs_and_uninitialized_prefixes() {
    let dir = tempdir().unwrap();
    let plan = initialized_plan(dir.path());
    let winetricks = dir.path().join("winetricks");
    write_script(&winetricks, "#!/bin/sh\nexit 0\n");
    let log_path = dir.path().join("winetricks.log");
    let (events, _received) = tokio::sync::mpsc::unbounded_channel();

    let invalid = install_component(
        plan.clone(),
        winetricks.clone(),
        "--force",
        log_path.clone(),
        events.clone(),
    )
    .await;
    assert!(
        matches!(invalid, Err(application::AppError::ComponentVerbInvalid(_))),
        "unexpected: {invalid:?}"
    );

    let uninitialized = PrefixPlan::new(
        dir.path().join("missing"),
        test_runner(dir.path()),
        Arch::Win64,
        dir.path().join("steam-client"),
    );
    let result = install_component(uninitialized, winetricks, "vcrun2019", log_path, events).await;
    assert!(
        matches!(result, Err(application::AppError::Prefix(_))),
        "unexpected: {result:?}"
    );
}
