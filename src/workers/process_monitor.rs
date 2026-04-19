//! Process Monitor worker (E26).
//!
//! Spawnea `sh -c <command>` via tokio::process, captura stdout/stderr linea
//! por linea y emite Actions hacia el reducer. El kill cooperativo se
//! chequea con un poll ligero (150ms) dentro de un `tokio::select!`.
//!
//! No intenta parsear el output — el reducer decide que hacer con cada linea
//! (mostrar, filtrar warnings, etc).

use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::Sender;

use crate::actions::Action;

/// Intervalo de poll del kill flag. Balance entre responsividad y overhead.
const KILL_POLL_MS: u64 = 150;

/// Spawnea el worker del monitor. Retorna inmediatamente.
///
/// El task vive hasta que el child termina (por su cuenta o via kill) o
/// hasta que el canal `tx` se cierra (app terminando).
pub fn spawn_monitor_task(id: String, command: String, kill: Arc<AtomicBool>, tx: Sender<Action>) {
    tokio::spawn(async move {
        run(id, command, kill, tx).await;
    });
}

async fn run(id: String, command: String, kill: Arc<AtomicBool>, tx: Sender<Action>) {
    let mut child = match Command::new("sh")
        .arg("-c")
        .arg(&command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            send_finished(&tx, id, None, Some(format!("spawn fallo: {e}")), false).await;
            return;
        }
    };

    // take() es infallible porque pedimos pipes en el builder.
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");
    let mut stdout_reader = BufReader::new(stdout).lines();
    let mut stderr_reader = BufReader::new(stderr).lines();
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut was_killed = false;

    while stdout_open || stderr_open {
        tokio::select! {
            line = stdout_reader.next_line(), if stdout_open => {
                match line {
                    Ok(Some(s)) => send_output(&tx, &id, s, false).await,
                    _ => { stdout_open = false; }
                }
            }
            line = stderr_reader.next_line(), if stderr_open => {
                match line {
                    Ok(Some(s)) => send_output(&tx, &id, s, true).await,
                    _ => { stderr_open = false; }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(KILL_POLL_MS)) => {
                if kill.load(Ordering::Relaxed) {
                    let _ = child.start_kill();
                    was_killed = true;
                    // Dejamos que los streams se cierren solos; exit loop.
                    stdout_open = false;
                    stderr_open = false;
                }
            }
        }
    }

    let status = child.wait().await;
    let exit_code = status.as_ref().ok().and_then(|s| s.code());
    let error = status.as_ref().err().map(|e| e.to_string());
    send_finished(&tx, id, exit_code, error, was_killed).await;
}

async fn send_output(tx: &Sender<Action>, id: &str, line: String, is_stderr: bool) {
    let _ = tx.send(Action::MonitorOutput { id: id.to_string(), line, is_stderr }).await;
}

async fn send_finished(
    tx: &Sender<Action>,
    id: String,
    exit_code: Option<i32>,
    error: Option<String>,
    killed: bool,
) {
    let _ = tx.send(Action::MonitorFinished { id, exit_code, error, killed }).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    async fn drain_actions(
        rx: &mut tokio::sync::mpsc::Receiver<Action>,
        deadline: Duration,
    ) -> Vec<Action> {
        let mut out = Vec::new();
        let deadline = tokio::time::Instant::now() + deadline;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Some(a)) => {
                    let is_finish = matches!(a, Action::MonitorFinished { .. });
                    out.push(a);
                    if is_finish {
                        break;
                    }
                }
                _ => break,
            }
        }
        out
    }

    #[tokio::test]
    async fn echo_produces_output_and_done_with_zero_exit() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(32);
        let kill = Arc::new(AtomicBool::new(false));
        spawn_monitor_task("m1".into(), "echo hello && echo world".into(), kill, tx);
        let actions = drain_actions(&mut rx, Duration::from_secs(5)).await;

        let outputs: Vec<&String> = actions
            .iter()
            .filter_map(|a| match a {
                Action::MonitorOutput { line, .. } => Some(line),
                _ => None,
            })
            .collect();
        assert!(outputs.iter().any(|l| l.contains("hello")));
        assert!(outputs.iter().any(|l| l.contains("world")));

        let finished = actions.iter().rev().find(|a| matches!(a, Action::MonitorFinished { .. }));
        assert!(matches!(
            finished,
            Some(Action::MonitorFinished { exit_code: Some(0), killed: false, .. })
        ));
    }

    #[tokio::test]
    async fn nonzero_exit_reported() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(8);
        let kill = Arc::new(AtomicBool::new(false));
        spawn_monitor_task("m1".into(), "exit 7".into(), kill, tx);
        let actions = drain_actions(&mut rx, Duration::from_secs(5)).await;
        let finished = actions.iter().rev().find(|a| matches!(a, Action::MonitorFinished { .. }));
        assert!(matches!(finished, Some(Action::MonitorFinished { exit_code: Some(7), .. })));
    }

    #[tokio::test]
    async fn stderr_lines_flagged() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(8);
        let kill = Arc::new(AtomicBool::new(false));
        spawn_monitor_task("m1".into(), "echo err 1>&2".into(), kill, tx);
        let actions = drain_actions(&mut rx, Duration::from_secs(5)).await;
        let err_line = actions.iter().find_map(|a| match a {
            Action::MonitorOutput { line, is_stderr: true, .. } => Some(line),
            _ => None,
        });
        assert!(err_line.is_some_and(|l| l.contains("err")));
    }

    #[tokio::test]
    async fn kill_flag_terminates_long_process() {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(8);
        let kill = Arc::new(AtomicBool::new(false));
        let kill_clone = kill.clone();
        spawn_monitor_task("m1".into(), "sleep 30".into(), kill, tx);

        // Dar tiempo a que el child arranque.
        tokio::time::sleep(Duration::from_millis(300)).await;
        kill_clone.store(true, Ordering::Relaxed);

        let actions = drain_actions(&mut rx, Duration::from_secs(5)).await;
        let finished = actions.iter().rev().find(|a| matches!(a, Action::MonitorFinished { .. }));
        match finished {
            Some(Action::MonitorFinished { killed, .. }) => assert!(*killed),
            other => panic!("expected MonitorFinished, got {other:?}"),
        }
    }
}
