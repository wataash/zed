use anyhow::{Context as _, Result};
use futures::{StreamExt as _, channel::oneshot};
use gpui::{App, AppContext as _, Global, actions};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::mpsc,
};
use util::ResultExt;
use workspace::with_active_or_new_workspace;

actions!(
    myhack0,
    [
        /// Opens command dispatch counts and recent history.
        ShowCommandHistory,
        /// Exports command dispatch history as JSONL and opens it.
        ExportCommandHistory,
        /// Opens command history configuration. Restart Zed after changing it.
        ConfigureCommandHistory,
    ]
);

#[derive(Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    enabled: bool,
    /// Action names, or namespaces with a trailing `*` such as `vim::*`.
    excluded_actions: Vec<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            excluded_actions: Vec::new(),
        }
    }
}

impl Config {
    fn records(&self, action: &str) -> bool {
        self.enabled
            && !self
                .excluded_actions
                .iter()
                .any(|pattern| excluded(pattern, action))
    }
}

#[derive(Deserialize, Serialize, Clone)]
struct Event {
    timestamp: String,
    action: String,
}

enum Message {
    Event(Event),
    Report {
        export: bool,
        reply: oneshot::Sender<Result<PathBuf>>,
    },
    Flush(oneshot::Sender<Result<()>>),
}
struct Recorder(mpsc::Sender<Message>);
impl Global for Recorder {}

fn base_directory(variable: &str, fallback: &str) -> Result<PathBuf> {
    let root = std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(fallback)))
        .context("No home directory for command history")?;
    Ok(root.join("wataash/zed/command-history"))
}

fn private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .with_context(|| format!("creating {}", path.display()))
}

fn load_config(path: &Path) -> Result<Config> {
    let parse = |bytes: Vec<u8>| -> Result<Config> {
        serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
    };
    match fs::read(path) {
        Ok(bytes) => parse(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let config = Config::default();
            match private_file(path) {
                Ok(mut file) => {
                    serde_json::to_writer_pretty(&mut file, &config)?;
                    file.write_all(b"\n")?;
                    Ok(config)
                }
                // Another instance wrote the default file first.
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists) =>
                {
                    parse(fs::read(path)?)
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn init(cx: &mut App) {
    if let Err(error) = start_from_environment(cx) {
        notify_error(format!("Command history: {error:#}"), cx);
    }
}

fn start_from_environment(cx: &mut App) -> Result<()> {
    let config_directory = base_directory("XDG_CONFIG_HOME", ".config")?;
    let data_directory = base_directory("XDG_DATA_HOME", ".local/share")?;
    start(&config_directory, &data_directory, cx)
}

fn start(config_directory: &Path, data_directory: &Path, cx: &mut App) -> Result<()> {
    fs::create_dir_all(config_directory)
        .with_context(|| format!("creating {}", config_directory.display()))?;
    let config_path = config_directory.join("config.json");
    // Registered before loading so that a broken config file can still be opened from Zed.
    cx.on_action({
        let config_path = config_path.clone();
        move |_: &ConfigureCommandHistory, cx| open(config_path.clone(), cx)
    });
    let config = load_config(&config_path)?;
    fs::create_dir_all(data_directory)
        .with_context(|| format!("creating {}", data_directory.display()))?;
    let session_path = data_directory.join(format!("{}.jsonl", uuid::Uuid::new_v4()));
    let mut file = if config.enabled {
        Some(private_file(&session_path)?)
    } else {
        None
    };

    // The worker cannot touch the UI, so failures travel back to the foreground for notification.
    let (failure_sender, mut failure_receiver) = futures::channel::mpsc::unbounded::<String>();
    let (sender, receiver) = mpsc::channel();
    let data_directory = data_directory.to_owned();
    std::thread::Builder::new()
        .name("command-history".into())
        .spawn(move || {
            let mut reported_write_failure = false;
            while let Ok(message) = receiver.recv() {
                match message {
                    Message::Event(event) => {
                        let Some(file) = file.as_mut() else {
                            continue;
                        };
                        if let Err(error) = write_event(file, &event) {
                            // A full disk fails on every action; notify once instead of per keystroke.
                            if !reported_write_failure {
                                reported_write_failure = true;
                                failure_sender
                                    .unbounded_send(format!(
                                        "Command history: writing {}: {error:#}",
                                        session_path.display()
                                    ))
                                    .log_err();
                            } else {
                                log::debug!("Writing command history: {error:#}");
                            }
                        }
                    }
                    Message::Report { export, reply } => {
                        // Reporting is serialized with recording so this process's queue is drained first.
                        let result = report(&data_directory, export);
                        if reply.send(result).is_err() {
                            log::debug!("Command history report was cancelled");
                        }
                    }
                    Message::Flush(reply) => {
                        let result = file
                            .as_mut()
                            .map(|file| file.sync_data().map_err(anyhow::Error::from))
                            .unwrap_or(Ok(()));
                        if reply.send(result).is_err() {
                            log::debug!("Command history flush was cancelled");
                        }
                    }
                }
            }
        })
        .context("spawning the command history thread")?;
    cx.spawn(async move |cx| {
        while let Some(message) = failure_receiver.next().await {
            cx.update(|cx| notify_error(message, cx));
        }
    })
    .detach();

    cx.set_global(Recorder(sender.clone()));
    if config.enabled {
        let record_sender = sender.clone();
        cx.observe_action_dispatch(move |action, _| {
            let name = action.name();
            if !config.records(name) {
                return;
            }
            let event = Event {
                timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                action: name.to_owned(),
            };
            record_sender.send(Message::Event(event)).log_err();
        })
        .detach();
    }
    cx.on_action(|_: &ShowCommandHistory, cx| request_report(false, cx));
    cx.on_action(|_: &ExportCommandHistory, cx| request_report(true, cx));
    cx.on_app_quit(move |_| {
        let (reply, response) = oneshot::channel();
        sender.send(Message::Flush(reply)).log_err();
        async move {
            match response.await {
                Ok(result) => {
                    result.log_err();
                }
                Err(error) => log::error!("Flushing command history: {error}"),
            }
        }
    })
    .detach();
    Ok(())
}

fn excluded(pattern: &str, action: &str) -> bool {
    // A trailing wildcard covers a namespace without collecting arguments or document contents.
    pattern
        .strip_suffix('*')
        .map_or(pattern == action, |prefix| action.starts_with(prefix))
}

fn write_event(file: &mut File, event: &Event) -> Result<()> {
    let mut bytes = serde_json::to_vec(event)?;
    bytes.push(b'\n');
    file.write_all(&bytes)?;
    Ok(())
}

fn read_events(directory: &Path) -> Result<Vec<Event>> {
    let mut events = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry
            .path()
            .extension()
            .is_none_or(|extension| extension != "jsonl")
            || !entry.file_type()?.is_file()
        {
            continue;
        }
        for line in BufReader::new(File::open(entry.path())?).lines() {
            let line = line?;
            match serde_json::from_str::<Event>(&line) {
                Ok(event) => events.push(event),
                // Another instance may currently be appending its final record.
                Err(error) => log::warn!("Skipping an incomplete command history record: {error}"),
            }
        }
    }
    events.sort_by(|left, right| left.timestamp.cmp(&right.timestamp));
    Ok(events)
}

fn report(directory: &Path, export: bool) -> Result<PathBuf> {
    let events = read_events(directory)?;
    let reports = directory.join("reports");
    fs::create_dir_all(&reports)?;
    let extension = if export { "jsonl" } else { "md" };
    let path = reports.join(format!("history-{}.{}", uuid::Uuid::new_v4(), extension));
    let mut file = private_file(&path)?;
    if export {
        for event in events {
            write_event(&mut file, &event)?;
        }
    } else {
        let mut counts = BTreeMap::<&str, usize>::new();
        for event in &events {
            *counts.entry(&event.action).or_default() += 1;
        }
        let mut counts: Vec<_> = counts.into_iter().collect();
        counts.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
        writeln!(
            file,
            "# Command history\n\n{} dispatches. Arguments and document contents are not recorded.\n\n| Count | Action |\n|---:|---|",
            events.len()
        )?;
        for (action, count) in counts {
            writeln!(file, "| {count} | `{action}` |")?;
        }
        writeln!(file, "\n## Latest 200 dispatches\n")?;
        for event in events.iter().rev().take(200) {
            writeln!(file, "- {} `{}`", event.timestamp, event.action)?;
        }
    }
    Ok(path)
}

fn notify_error(message: String, cx: &mut App) {
    use workspace::notifications::{
        NotificationId, show_app_notification, simple_message_notification::MessageNotification,
    };
    log::error!("{message}");
    show_app_notification(NotificationId::unique::<Recorder>(), cx, move |cx| {
        cx.new(|cx| MessageNotification::new(message.clone(), cx))
    });
}

fn request_report(export: bool, cx: &mut App) {
    let Some(recorder) = cx.try_global::<Recorder>() else {
        notify_error("Command history: recorder is not running".into(), cx);
        return;
    };
    let (reply, response) = oneshot::channel();
    if recorder.0.send(Message::Report { export, reply }).is_err() {
        notify_error("Command history: recorder thread has stopped".into(), cx);
        return;
    }
    cx.spawn(async move |cx| {
        let result = response
            .await
            .map_err(anyhow::Error::from)
            .and_then(|result| result);
        cx.update(|cx| match result {
            Ok(path) => open(path, cx),
            Err(error) => notify_error(format!("Command history: {error:#}"), cx),
        });
    })
    .detach();
}

fn open(path: PathBuf, cx: &mut App) {
    with_active_or_new_workspace(cx, move |workspace, window, cx| {
        let task = workspace.open_abs_path(path.clone(), Default::default(), window, cx);
        cx.spawn(async move |_, cx| {
            if let Err(error) = task.await {
                cx.update(|cx| {
                    notify_error(
                        format!("Command history: opening {}: {error:#}", path.display()),
                        cx,
                    )
                });
            }
        })
        .detach();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    actions!(command_history_test, [Recorded, Excluded]);

    struct TempDirectory(PathBuf);
    impl TempDirectory {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("zed-history-test-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).expect("temp directory");
            Self(path)
        }
    }
    impl Drop for TempDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).log_err();
        }
    }

    fn session_files(directory: &Path) -> Vec<PathBuf> {
        fs::read_dir(directory)
            .expect("data directory")
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "jsonl")
            })
            .collect()
    }

    fn flush(cx: &mut TestAppContext) {
        let (reply, response) = oneshot::channel();
        cx.update(|cx| cx.global::<Recorder>().0.send(Message::Flush(reply)))
            .expect("recorder thread");
        // The worker is a plain thread, so its reply does not go through the test scheduler.
        smol::block_on(response)
            .expect("flush reply")
            .expect("flush");
    }

    #[test]
    fn history_roundtrip_and_export() -> Result<()> {
        let directory = TempDirectory::new();
        let mut file = private_file(&directory.0.join("session.jsonl"))?;
        for action in ["myhack0::InsertDate", "myhack0::InsertDate", "editor::Undo"] {
            write_event(
                &mut file,
                &Event {
                    timestamp: "2026-09-13T00:00:00.000Z".into(),
                    action: action.into(),
                },
            )?;
        }
        file.write_all(b"{incomplete")?;
        assert_eq!(read_events(&directory.0)?.len(), 3);
        let summary = fs::read_to_string(report(&directory.0, false)?)?;
        assert!(summary.contains("| 2 | `myhack0::InsertDate` |"));
        let exported = fs::read_to_string(report(&directory.0, true)?)?;
        assert_eq!(exported.lines().count(), 3);
        // Generated reports must not feed back into later reports.
        assert_eq!(read_events(&directory.0)?.len(), 3);
        Ok(())
    }

    #[test]
    fn config_defaults_and_exclusions() -> Result<()> {
        let directory = TempDirectory::new();
        let path = directory.0.join("config.json");
        let config = load_config(&path)?;
        assert!(config.enabled);
        assert!(config.excluded_actions.is_empty());
        assert!(path.is_file());

        fs::write(
            &path,
            r#"{"enabled": true, "excluded_actions": ["vim::*", "editor::HandleInput"]}"#,
        )?;
        let config = load_config(&path)?;
        assert!(config.records("editor::Undo"));
        assert!(!config.records("vim::Yank"));
        assert!(!config.records("editor::HandleInput"));
        assert!(config.records("editor::HandleInputX"));

        fs::write(&path, r#"{"enabled": false}"#)?;
        assert!(!load_config(&path)?.records("editor::Undo"));

        fs::write(&path, r#"{"enabled": true, "typo": 1}"#)?;
        assert!(load_config(&path).is_err());
        fs::write(&path, "")?;
        assert!(load_config(&path).is_err());
        Ok(())
    }

    /// Starts recording into a temporary tree. The worker is a real thread, so the test
    /// scheduler must accept wakeups from it.
    fn start_with_config(config: &str, cx: &mut TestAppContext) -> (TempDirectory, PathBuf) {
        cx.background_executor.allow_parking();
        let directory = TempDirectory::new();
        let config_directory = directory.0.join("config");
        let data_directory = directory.0.join("data");
        fs::create_dir_all(&config_directory).expect("config directory");
        fs::write(config_directory.join("config.json"), config).expect("config file");
        cx.update(|cx| start(&config_directory, &data_directory, cx))
            .expect("start");
        (directory, data_directory)
    }

    #[gpui::test]
    fn records_dispatched_actions_unless_excluded(cx: &mut TestAppContext) {
        let (_directory, data_directory) = start_with_config(
            r#"{"excluded_actions": ["command_history_test::Excluded"]}"#,
            cx,
        );
        assert_eq!(session_files(&data_directory).len(), 1);

        // Without a window this is the global dispatch path; keyboard dispatch is covered in gpui.
        cx.update(|cx| {
            cx.dispatch_action(&Recorded);
            cx.dispatch_action(&Excluded);
            cx.dispatch_action(&Recorded);
        });
        flush(cx);
        let events = read_events(&data_directory).expect("events");
        assert_eq!(
            events
                .iter()
                .map(|event| event.action.as_str())
                .collect::<Vec<_>>(),
            ["command_history_test::Recorded"; 2]
        );
        assert!(events.iter().all(|event| event.timestamp.ends_with('Z')));
    }

    #[gpui::test]
    fn disabled_config_records_nothing(cx: &mut TestAppContext) {
        let (_directory, data_directory) = start_with_config(r#"{"enabled": false}"#, cx);
        cx.update(|cx| cx.dispatch_action(&Recorded));
        flush(cx);
        assert!(session_files(&data_directory).is_empty());
        assert!(read_events(&data_directory).expect("events").is_empty());
    }

    #[gpui::test]
    fn start_reports_unwritable_data_directory(cx: &mut TestAppContext) {
        let directory = TempDirectory::new();
        let blocker = directory.0.join("file");
        fs::write(&blocker, "").expect("blocker file");
        let result = cx.update(|cx| start(&directory.0.join("config"), &blocker.join("data"), cx));
        assert!(result.is_err());
        assert!(cx.update(|cx| cx.try_global::<Recorder>().is_none()));
    }
}
