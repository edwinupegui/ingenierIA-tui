//! Ejecucion async de hooks como procesos shell.
//!
//! - Fire-and-forget: `HookRunner::fire(trigger, ctx, tx)` nunca bloquea
//!   al caller. Spawnea una tarea que ejecuta todos los hooks matching en
//!   paralelo y envia un `Action::HookExecuted` por cada `HookOutcome`.
//! - Timeout por hook via `tokio::time::timeout`. Si se excede, exit_code=-2.
//! - No mata procesos en timeout (best-effort): deja que el OS limpie.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::mpsc::Sender;

use crate::actions::Action;

use super::config::{HookDef, HookFailurePolicy};
use super::types::{HookContext, HookOutcome, HookTrigger};

/// Registro inmutable de hooks cargados al inicio. Se comparte via `Arc`.
#[derive(Debug, Clone, Default)]
pub struct HookRunner {
    defs: Arc<Vec<HookDef>>,
}

impl HookRunner {
    pub fn new(defs: Vec<HookDef>) -> Self {
        Self { defs: Arc::new(defs) }
    }

    /// Cantidad de hooks registrados (util para status/diagnostics).
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// Dispara todos los hooks matching `trigger` + `ctx.matches_tool`.
    /// No bloquea: spawnea tarea y retorna inmediatamente.
    pub fn fire(&self, trigger: HookTrigger, ctx: HookContext, tx: Sender<Action>) {
        if self.defs.is_empty() {
            return;
        }
        let matching: Vec<HookDef> = self
            .defs
            .iter()
            .filter(|d| d.trigger == trigger && ctx.matches_tool(d.match_tool.as_deref()))
            .cloned()
            .collect();

        if matching.is_empty() {
            return;
        }

        tokio::spawn(async move {
            for def in matching {
                let outcome = execute_one(&def, &ctx, trigger).await;
                if should_report(&def, &outcome) {
                    let _ = tx.send(Action::HookExecuted(outcome)).await; // fire-and-forget: receptor puede estar cerrado
                }
            }
        });
    }
}

/// Devuelve false solo cuando on_failure=Ignore y exit!=0, para evitar spam.
/// Exitos siempre se reportan (el reducer decide si notifica o no).
fn should_report(def: &HookDef, outcome: &HookOutcome) -> bool {
    if outcome.is_success() {
        return true;
    }
    !matches!(def.on_failure, HookFailurePolicy::Ignore)
}

async fn execute_one(def: &HookDef, ctx: &HookContext, trigger: HookTrigger) -> HookOutcome {
    let start = Instant::now();
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(&def.command);
    cmd.env_clear();
    // Pasar PATH minimo para que `sh -c` resuelva binarios comunes.
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }
    if let Ok(home) = std::env::var("HOME") {
        cmd.env("HOME", home);
    }
    for (k, v) in ctx.env_vars(trigger) {
        cmd.env(k, v);
    }

    let fut = cmd.output();
    let result = tokio::time::timeout(Duration::from_secs(def.timeout_secs.into()), fut).await;

    let elapsed_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;

    match result {
        Ok(Ok(output)) => HookOutcome {
            name: def.name.clone(),
            trigger,
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: elapsed_ms,
            stderr_tail: last_lines(&output.stderr, 5),
        },
        Ok(Err(e)) => HookOutcome {
            name: def.name.clone(),
            trigger,
            exit_code: -1,
            duration_ms: elapsed_ms,
            stderr_tail: format!("io error: {e}"),
        },
        Err(_) => HookOutcome {
            name: def.name.clone(),
            trigger,
            exit_code: -2,
            duration_ms: elapsed_ms,
            stderr_tail: format!("timeout tras {}s", def.timeout_secs),
        },
    }
}

/// Devuelve las ultimas `n` lineas de un buffer como string utf8-safe.
fn last_lines(bytes: &[u8], n: usize) -> String {
    let s = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = s.lines().collect();
    let start = lines.len().saturating_sub(n);
    let tail = lines[start..].join("\n");
    if tail.len() > 512 {
        let mut out: String = tail.chars().take(511).collect();
        out.push('…');
        out
    } else {
        tail
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn def(name: &str, trigger: HookTrigger, cmd: &str) -> HookDef {
        HookDef {
            name: name.into(),
            trigger,
            match_tool: None,
            command: cmd.into(),
            timeout_secs: 5,
            on_failure: HookFailurePolicy::Warn,
        }
    }

    #[tokio::test]
    async fn runs_matching_hook_and_reports() {
        let runner = HookRunner::new(vec![def("a", HookTrigger::PreToolUse, "exit 0")]);
        let (tx, mut rx) = mpsc::channel(8);
        runner.fire(HookTrigger::PreToolUse, HookContext::for_tool("Bash", ""), tx);
        let action = rx.recv().await.expect("should receive outcome");
        match action {
            Action::HookExecuted(o) => {
                assert_eq!(o.name, "a");
                assert_eq!(o.exit_code, 0);
            }
            _ => panic!("expected HookExecuted"),
        }
    }

    #[tokio::test]
    async fn skips_hooks_of_other_triggers() {
        let runner = HookRunner::new(vec![def("a", HookTrigger::PreCodeApply, "exit 0")]);
        let (tx, mut rx) = mpsc::channel(8);
        runner.fire(HookTrigger::PreToolUse, HookContext::default(), tx);
        // Sin matches → nadie spawnea → rx no recibe nada y se cierra cuando se dropea tx.
        drop(runner);
        let timeout = tokio::time::timeout(Duration::from_millis(50), rx.recv()).await;
        assert!(timeout.is_err() || timeout.unwrap().is_none());
    }

    #[tokio::test]
    async fn tool_pattern_filters_hooks() {
        let runner = HookRunner::new(vec![
            HookDef {
                name: "only_bash".into(),
                trigger: HookTrigger::PreToolUse,
                match_tool: Some("Bash".into()),
                command: "exit 0".into(),
                timeout_secs: 5,
                on_failure: HookFailurePolicy::Warn,
            },
            HookDef {
                name: "any_tool".into(),
                trigger: HookTrigger::PreToolUse,
                match_tool: None,
                command: "exit 0".into(),
                timeout_secs: 5,
                on_failure: HookFailurePolicy::Warn,
            },
        ]);
        let (tx, mut rx) = mpsc::channel(8);
        runner.fire(HookTrigger::PreToolUse, HookContext::for_tool("Read", ""), tx);
        let mut names = Vec::new();
        while let Ok(Some(a)) = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await {
            if let Action::HookExecuted(o) = a {
                names.push(o.name);
            }
        }
        assert_eq!(names, vec!["any_tool"]);
    }

    #[tokio::test]
    async fn nonzero_exit_is_still_reported_with_warn_policy() {
        let runner = HookRunner::new(vec![def("bad", HookTrigger::PreToolUse, "exit 7")]);
        let (tx, mut rx) = mpsc::channel(8);
        runner.fire(HookTrigger::PreToolUse, HookContext::default(), tx);
        let action = rx.recv().await.unwrap();
        match action {
            Action::HookExecuted(o) => {
                assert_eq!(o.exit_code, 7);
                assert!(!o.is_success());
            }
            _ => panic!(),
        }
    }

    #[tokio::test]
    async fn ignore_policy_silences_failures() {
        let mut d = def("silent", HookTrigger::PreToolUse, "exit 1");
        d.on_failure = HookFailurePolicy::Ignore;
        let runner = HookRunner::new(vec![d]);
        let (tx, mut rx) = mpsc::channel(8);
        runner.fire(HookTrigger::PreToolUse, HookContext::default(), tx);
        let t = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await;
        assert!(t.is_err() || matches!(t, Ok(None)));
    }

    #[tokio::test]
    async fn timeout_yields_exit_minus_two() {
        let mut d = def("slow", HookTrigger::PreToolUse, "sleep 5");
        d.timeout_secs = 1;
        let runner = HookRunner::new(vec![d]);
        let (tx, mut rx) = mpsc::channel(8);
        runner.fire(HookTrigger::PreToolUse, HookContext::default(), tx);
        let action =
            tokio::time::timeout(Duration::from_secs(3), rx.recv()).await.unwrap().unwrap();
        match action {
            Action::HookExecuted(o) => {
                assert_eq!(o.exit_code, -2);
                assert!(o.stderr_tail.contains("timeout"));
            }
            _ => panic!(),
        }
    }

    #[tokio::test]
    async fn env_vars_propagate_to_command() {
        // El hook valida su env y retorna 0 si matchea, 7 si no.
        let cmd = r#"[ "$INGENIERIA_TOOL_NAME" = "Bash" ] && [ "$INGENIERIA_HOOK_TRIGGER" = "PreToolUse" ] || exit 7"#;
        let runner = HookRunner::new(vec![def("env", HookTrigger::PreToolUse, cmd)]);
        let (tx, mut rx) = mpsc::channel(8);
        runner.fire(HookTrigger::PreToolUse, HookContext::for_tool("Bash", "ls"), tx);
        let action = rx.recv().await.unwrap();
        if let Action::HookExecuted(o) = action {
            assert_eq!(o.exit_code, 0);
        } else {
            panic!();
        }
    }

    #[test]
    fn last_lines_truncates_long_output() {
        let big: String = (0..100).map(|i| format!("line {i}\n")).collect();
        let out = last_lines(big.as_bytes(), 3);
        let count = out.lines().count();
        assert_eq!(count, 3);
        assert!(out.contains("line 99"));
    }

    #[test]
    fn empty_runner_is_noop() {
        let runner = HookRunner::default();
        let (tx, _rx) = mpsc::channel::<Action>(1);
        runner.fire(HookTrigger::PreToolUse, HookContext::default(), tx);
        // no panic, no hang
        assert!(runner.is_empty());
    }
}
