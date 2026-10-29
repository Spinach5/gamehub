use crate::{AppError, PrefixPlan};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentOutcome {
    pub verb: String,
    pub success: bool,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone)]
pub enum ComponentEvent {
    Started { process_group: u32 },
    Line(String),
}

/// Reads the verbs winetricks recorded in the prefix itself.
pub async fn installed_components(plan: PrefixPlan) -> Vec<String> {
    tokio::task::spawn_blocking(move || ::prefix::installed_verbs(&plan))
        .await
        .unwrap_or_default()
}

/// Runs one winetricks verb against the prefix, streaming its output.
///
/// The process gets its own group so cancelling can terminate winetricks and
/// the installer helpers it spawns in one go.
pub async fn install_component(
    plan: PrefixPlan,
    winetricks: PathBuf,
    verb: &str,
    log_path: PathBuf,
    events: UnboundedSender<ComponentEvent>,
) -> Result<ComponentOutcome, AppError> {
    if ::prefix::status(&plan) != ::prefix::PrefixStatus::Initialized {
        return Err(::prefix::PrefixError::PrefixNotInitialized(plan.path.clone()).into());
    }
    let verb = ::prefix::normalize_verb(verb)
        .map_err(|_| AppError::ComponentVerbInvalid(verb.trim().to_owned()))?;
    let command = ::prefix::command_for(&plan, &winetricks, &verb);

    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .envs(&command.env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(false);

    let mut child = process
        .spawn()
        .map_err(|source| ::prefix::PrefixError::WinetricksSpawn {
            program: command.program.clone(),
            source,
        })?;
    if let Some(process_group) = child.id() {
        let _ = events.send(ComponentEvent::Started { process_group });
    }

    let (lines, receiver) = tokio::sync::mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(write_log(receiver, log_path, events));

    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(spawn_reader(stdout, lines.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(spawn_reader(stderr, lines.clone()));
    }
    drop(lines);

    let status = child.wait().await.map_err(AppError::ComponentIo)?;
    for reader in readers {
        let _ = reader.await;
    }
    let _ = writer.await;

    Ok(ComponentOutcome {
        verb,
        success: status.success(),
        exit_code: status.code(),
    })
}

pub async fn cancel_component(process_group: u32) -> Result<(), AppError> {
    ::launch::terminate_process_group(process_group).await?;
    Ok(())
}

fn spawn_reader<R>(reader: R, lines: tokio::sync::mpsc::UnboundedSender<String>) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines_in = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines_in.next_line().await {
            if lines.send(line.trim_end_matches('\r').to_owned()).is_err() {
                break;
            }
        }
    })
}

async fn write_log(
    mut lines: tokio::sync::mpsc::UnboundedReceiver<String>,
    log_path: PathBuf,
    events: UnboundedSender<ComponentEvent>,
) {
    let mut file = open_log(&log_path).await;
    while let Some(line) = lines.recv().await {
        if let Some(file) = file.as_mut() {
            let _ = file.write_all(line.as_bytes()).await;
            let _ = file.write_all(b"\n").await;
        }
        let _ = events.send(ComponentEvent::Line(line));
    }
    if let Some(mut file) = file {
        let _ = file.flush().await;
    }
}

async fn open_log(path: &Path) -> Option<tokio::fs::File> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        && let Err(error) = tokio::fs::create_dir_all(parent).await
    {
        tracing::warn!(%error, path = %path.display(), "cannot create component log dir");
        return None;
    }
    match tokio::fs::File::create(path).await {
        Ok(file) => Some(file),
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "cannot open component log");
            None
        }
    }
}
