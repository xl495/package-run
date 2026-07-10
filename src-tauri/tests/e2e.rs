//! Backend e2e tests: run against a mock Tauri runtime but with real
//! filesystem, real npm processes and real signals.

use package_run_lib::{ports, projects, runner};
use std::{fs, path::PathBuf, thread, time::Duration};
use tauri::Manager;

type MockApp = tauri::App<tauri::test::MockRuntime>;

fn mock_app() -> MockApp {
    // Unique identifier per test so parallel tests don't share projects.json.
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let mut ctx = tauri::test::mock_context(tauri::test::noop_assets());
    ctx.config_mut().identifier = format!("com.packagerun.e2e-{}-{seq}", std::process::id());
    tauri::test::mock_builder()
        .manage(runner::RunnerState::default())
        .manage(projects::UiState::default())
        .build(ctx)
        .expect("failed to build mock app")
}

/// Creates a throwaway npm project with the given scripts.
fn fixture_project(name: &str, scripts: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("package-run-e2e-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("package.json"),
        format!(r#"{{ "name": "{name}", "version": "1.0.0", "scripts": {{ {scripts} }} }}"#),
    )
    .unwrap();
    dir
}

fn cleanup(app: &MockApp, dir: &PathBuf) {
    let _ = fs::remove_dir_all(dir);
    if let Ok(data_dir) = app.path().app_data_dir() {
        let _ = fs::remove_dir_all(data_dir);
    }
}

fn pid_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[test]
fn add_list_remove_project() {
    let app = mock_app();
    let handle = app.handle();
    let dir = fixture_project("proj-a", r#""dev": "sleep 5", "build": "echo built""#);
    let path = dir.to_string_lossy().to_string();

    // Add: scripts and package manager must be parsed from disk.
    let project = projects::add_project_path(handle, &path).expect("add should succeed");
    assert_eq!(project.name, "proj-a");
    assert_eq!(project.package_manager, "pnpm"); // no lockfile -> default
    assert_eq!(project.scripts.len(), 2);
    assert_eq!(project.scripts["build"], "echo built");

    // Adding the same path twice must not duplicate it.
    projects::add_project_path(handle, &path).unwrap();
    let listed = projects::list_projects(handle.clone());
    assert_eq!(listed.len(), 1);

    // A folder without package.json must be rejected.
    let bogus = std::env::temp_dir().join("package-run-e2e-bogus");
    fs::create_dir_all(&bogus).unwrap();
    assert!(projects::add_project_path(handle, &bogus.to_string_lossy()).is_err());
    let _ = fs::remove_dir_all(&bogus);

    // Remove.
    projects::remove_project(handle.clone(), listed[0].id.clone()).unwrap();
    assert!(projects::list_projects(handle.clone()).is_empty());

    cleanup(&app, &dir);
}

#[test]
fn lockfile_detection() {
    let app = mock_app();
    let handle = app.handle();
    let dir = fixture_project("proj-lock", r#""dev": "sleep 1""#);
    fs::write(dir.join("yarn.lock"), "").unwrap();

    let project = projects::add_project_path(handle, &dir.to_string_lossy()).unwrap();
    assert_eq!(project.package_manager, "yarn");

    cleanup(&app, &dir);
}

#[test]
fn start_logs_and_stop_kills_process_tree() {
    let app = mock_app();
    let handle = app.handle().clone();
    let dir = fixture_project(
        "proj-run",
        r#""serve": "echo server-started && sleep 60""#,
    );
    let project = projects::add_project_path(&handle, &dir.to_string_lossy()).unwrap();
    let state = app.state::<runner::RunnerState>();

    let info = runner::start_script(
        handle.clone(),
        state.clone(),
        project.id.clone(),
        project.path.clone(),
        "npm".into(),
        "serve".into(),
    )
    .expect("start should succeed");
    assert!(pid_alive(info.pid), "npm process should be running");

    // Duplicate start must be rejected while running.
    let dup = runner::start_script(
        handle.clone(),
        state.clone(),
        project.id.clone(),
        project.path.clone(),
        "npm".into(),
        "serve".into(),
    );
    assert!(dup.is_err(), "duplicate start must fail");

    // Output must land in the log file (give npm a moment to boot).
    thread::sleep(Duration::from_secs(3));
    let log = fs::read_to_string(&info.log_file).expect("log file must exist");
    assert!(
        log.contains("server-started"),
        "log should contain script output, got:\n{log}"
    );

    assert_eq!(runner::running_tasks(state.clone()).len(), 1);

    // Stop must kill npm AND its children (the sleep) via the process group.
    runner::stop_script(handle.clone(), state.clone(), project.id.clone(), "serve".into())
        .expect("stop should succeed");
    thread::sleep(Duration::from_millis(300));
    assert!(!pid_alive(info.pid), "npm process must be dead after stop");
    assert!(runner::running_tasks(state.clone()).is_empty());

    let log = fs::read_to_string(&info.log_file).unwrap();
    assert!(log.contains("stopped by user"), "log should record the manual stop");

    cleanup(&app, &dir);
}

#[test]
fn short_script_exits_and_is_reaped() {
    let app = mock_app();
    let handle = app.handle().clone();
    let dir = fixture_project("proj-quick", r#""once": "echo done-quickly""#);
    let project = projects::add_project_path(&handle, &dir.to_string_lossy()).unwrap();
    let state = app.state::<runner::RunnerState>();

    let info = runner::start_script(
        handle.clone(),
        state.clone(),
        project.id.clone(),
        project.path.clone(),
        "npm".into(),
        "once".into(),
    )
    .unwrap();

    // The watcher polls every 500ms; npm itself needs a moment.
    let mut reaped = false;
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(500));
        if runner::running_tasks(state.clone()).is_empty() {
            reaped = true;
            break;
        }
    }
    assert!(reaped, "finished script should be removed from running tasks");

    let log = fs::read_to_string(&info.log_file).unwrap();
    assert!(log.contains("done-quickly"));
    assert!(log.contains("process exited"), "log should record natural exit");

    cleanup(&app, &dir);
}

#[test]
fn pinned_projects_sort_first() {
    let app = mock_app();
    let handle = app.handle();
    let dir_a = fixture_project("aaa-first", r#""dev": "sleep 1""#);
    let dir_b = fixture_project("zzz-last", r#""dev": "sleep 1""#);
    projects::add_project_path(handle, &dir_a.to_string_lossy()).unwrap();
    let b = projects::add_project_path(handle, &dir_b.to_string_lossy()).unwrap();

    // Store order by default.
    let list = projects::list_projects(handle.clone());
    assert_eq!(list[0].name, "aaa-first");

    // Pinned wins over alphabetical order.
    projects::toggle_pin(handle.clone(), b.id.clone()).unwrap();
    let list = projects::list_projects(handle.clone());
    assert_eq!(list[0].name, "zzz-last");
    assert!(list[0].pinned);

    let _ = fs::remove_dir_all(&dir_b);
    cleanup(&app, &dir_a);
}


#[test]
fn manual_project_order_is_persisted() {
    let app = mock_app();
    let handle = app.handle();
    let dir_a = fixture_project("order-a", r#""dev": "sleep 1""#);
    let dir_b = fixture_project("order-b", r#""dev": "sleep 1""#);
    let a = projects::add_project_path(handle, &dir_a.to_string_lossy()).unwrap();
    let b = projects::add_project_path(handle, &dir_b.to_string_lossy()).unwrap();

    let list = projects::list_projects(handle.clone());
    assert_eq!(list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["order-a", "order-b"]);

    projects::set_project_order(handle.clone(), vec![b.id.clone(), a.id.clone()]).unwrap();
    let list = projects::list_projects(handle.clone());
    assert_eq!(list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["order-b", "order-a"]);

    let _ = fs::remove_dir_all(&dir_b);
    cleanup(&app, &dir_a);
}


#[test]
fn corepack_field_beats_lockfile() {
    let app = mock_app();
    let handle = app.handle();
    let dir = fixture_project("proj-corepack", r#""dev": "sleep 1""#);
    // Lockfile says npm, packageManager field says yarn -> yarn wins.
    fs::write(dir.join("package-lock.json"), "{}").unwrap();
    let pkg = fs::read_to_string(dir.join("package.json")).unwrap();
    let pkg = pkg.replace(
        r#""version": "1.0.0","#,
        r#""version": "1.0.0", "packageManager": "yarn@4.1.0","#,
    );
    fs::write(dir.join("package.json"), pkg).unwrap();

    let project = projects::add_project_path(handle, &dir.to_string_lossy()).unwrap();
    assert_eq!(project.package_manager, "yarn");

    // Manual override beats everything.
    projects::set_package_manager(handle.clone(), project.id.clone(), Some("npm".into())).unwrap();
    let list = projects::list_projects(handle.clone());
    assert_eq!(list[0].package_manager, "npm");
    assert!(list[0].pm_overridden);

    cleanup(&app, &dir);
}

#[test]
fn missing_pm_is_rejected_with_code() {
    let app = mock_app();
    let handle = app.handle().clone();
    let dir = fixture_project("proj-nopm", r#""dev": "sleep 1""#);
    let project = projects::add_project_path(&handle, &dir.to_string_lossy()).unwrap();
    let state = app.state::<runner::RunnerState>();

    let err = runner::start_script(
        handle.clone(),
        state.clone(),
        project.id,
        project.path,
        "notarealpm".into(),
        "dev".into(),
    )
    .unwrap_err();
    assert_eq!(err, "ERR_PM_MISSING|notarealpm");

    cleanup(&app, &dir);
}

#[test]
fn installed_pms_include_npm() {
    assert!(
        projects::installed_pms().iter().any(|p| p == "npm"),
        "npm ships with node and must be detected"
    );
}

#[test]
fn launch_config_injects_env_port_and_args() {
    let app = mock_app();
    let handle = app.handle().clone();
    let dir = fixture_project(
        "proj-cfg",
        r#""show": "node -e \"console.log('E='+process.env.FOO+' P='+process.env.PORT+' A='+process.argv.slice(1).join(','))\" --""#,
    );
    // env_file value should be overridden by the explicit env map.
    fs::write(dir.join(".env.test"), "FROM_FILE=yes\nFOO=file-value\n").unwrap();
    let project = projects::add_project_path(&handle, &dir.to_string_lossy()).unwrap();
    let state = app.state::<runner::RunnerState>();

    let mut env = std::collections::BTreeMap::new();
    env.insert("FOO".to_string(), "bar".to_string());
    projects::set_launch_config(
        handle.clone(),
        project.id.clone(),
        "show".into(),
        Some(projects::LaunchConfig {
            port: Some(4567),
            env,
            env_file: Some(".env.test".into()),
            extra_args: Some("--flag x".into()),
        }),
    )
    .unwrap();

    let info = runner::start_script(
        handle.clone(),
        state.clone(),
        project.id.clone(),
        project.path.clone(),
        "npm".into(),
        "show".into(),
    )
    .unwrap();

    for _ in 0..20 {
        thread::sleep(Duration::from_millis(500));
        if runner::running_tasks(state.clone()).is_empty() {
            break;
        }
    }
    let log = fs::read_to_string(&info.log_file).unwrap();
    assert!(log.contains("E=bar"), "explicit env should win, got:\n{log}");
    assert!(log.contains("P=4567"), "PORT env should be set, got:\n{log}");
    assert!(
        log.contains("--port,4567") && log.contains("--flag,x"),
        "args should be forwarded through npm --, got:\n{log}"
    );

    cleanup(&app, &dir);
}

#[test]
fn port_info_detects_listener() {
    // Bind a real port in this test process.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let info = ports::port_info(port)
        .expect("lsof should run")
        .expect("port should be reported as occupied");
    assert_eq!(info.pid, std::process::id(), "should point at this test process");

    drop(listener);
    // A short beat for the OS to release the socket.
    thread::sleep(Duration::from_millis(200));
    let free = ports::port_info(port).unwrap();
    assert!(free.is_none(), "released port should be free");
}

#[test]
fn url_is_extracted_from_output() {
    let app = mock_app();
    let handle = app.handle().clone();
    let dir = fixture_project(
        "proj-url",
        r#""fake-dev": "echo Local: http://localhost:3999/ && sleep 30""#,
    );
    let project = projects::add_project_path(&handle, &dir.to_string_lossy()).unwrap();
    let state = app.state::<runner::RunnerState>();

    runner::start_script(
        handle.clone(),
        state.clone(),
        project.id.clone(),
        project.path.clone(),
        "npm".into(),
        "fake-dev".into(),
    )
    .unwrap();

    let mut url = None;
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(500));
        if let Some(t) = runner::running_tasks(state.clone()).first() {
            if t.url.is_some() {
                url = t.url.clone();
                break;
            }
        }
    }
    assert_eq!(url.as_deref(), Some("http://localhost:3999"));

    runner::stop_script(handle.clone(), state.clone(), project.id, "fake-dev".into()).unwrap();
    cleanup(&app, &dir);
}
