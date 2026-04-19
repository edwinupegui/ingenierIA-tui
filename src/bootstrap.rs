//! Startup y shutdown del proceso — ingenieria-tui.
//!
//! Extrae la lógica de inicialización de main.rs para mantenerlo ≤80 líneas.

use std::sync::Arc;

use clap::Parser;
use tokio::sync::mpsc::Sender;

use crate::actions::Action;
use crate::app::App;
use crate::config::Config;
use crate::services::IngenieriaClient;
use crate::ui::frame_throttle::FrameThrottle;
use crate::{services, state, workers};

/// Argumentos de línea de comandos.
#[derive(Parser)]
#[command(name = "ingenieria", version, about = "Terminal UI for ingenierIA MCP Server")]
pub struct Cli {
    /// URL del servidor ingenierIA (override)
    #[arg(long = "server-url")]
    pub server_url: Option<String>,
    /// Abrir wizard de configuración
    #[arg(long)]
    pub config: bool,
    /// Demos offline sin API keys; escenario via INGENIERIA_MOCK_SCENARIO.
    #[arg(long)]
    pub mock: bool,
}

pub fn log_render_stats(throttle: &FrameThrottle) {
    tracing::debug!(
        drawn = throttle.frames_drawn(),
        skipped = throttle.frames_skipped(),
        "render frame"
    );
}

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off")),
        )
        .with_writer(std::io::stderr)
        .init();
}

pub fn setup_terminal() -> anyhow::Result<()> {
    crossterm::execute!(
        std::io::stdout(),
        crossterm::event::EnableMouseCapture,
        crossterm::event::EnableBracketedPaste,
        crossterm::event::EnableFocusChange,
    )?;
    // Enhanced keyboard protocol (Kitty) — silently ignored if unsupported.
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::PushKeyboardEnhancementFlags(
            crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | crossterm::event::KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | crossterm::event::KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        ),
    );
    Ok(())
}

pub fn spawn_workers(
    tx: Sender<Action>,
    client: Arc<IngenieriaClient>,
    config: &Config,
    events_url: String,
) {
    tokio::spawn(workers::tick::run(tx.clone()));
    tokio::spawn(workers::keyboard::run(tx.clone()));
    tokio::spawn(workers::health::run(client.clone(), tx.clone()));
    tokio::spawn(workers::sse::run(
        events_url,
        config.developer.clone(),
        config.model.clone(),
        tx.clone(),
    ));
    tokio::spawn(workers::tool_events::run(config.server_url.clone(), tx.clone()));
    tokio::spawn(workers::hook_events::run(config.server_url.clone(), tx.clone()));
    tokio::spawn(workers::file_watcher::run(tx.clone()));
}

pub fn init_app(app: &mut App, tx: Sender<Action>, show_wizard: bool) {
    match services::cron::load_jobs() {
        Ok(jobs) => {
            if !jobs.is_empty() {
                app.state.toasts.push(
                    format!("{} cron job(s) cargados", jobs.len()),
                    state::ToastLevel::Info,
                    0,
                );
            }
            app.state.crons = services::cron::CronRegistry::with_jobs(jobs);
        }
        Err(e) => {
            tracing::warn!(error = %e, "fallo cargar crons.json — empezando vacio");
            app.state.toasts.push(
                format!("⚠ crons.json corrupto: {e}"),
                state::ToastLevel::Warning,
                0,
            );
        }
    }
    workers::cron_worker::spawn(app.state.crons.clone(), tx);

    app.state.onboarding = services::onboarding::OnboardingState::load();
    app.state.onboarding.bump_session_and_save();
    if !show_wizard {
        app.state.onboarding.checklist.mark(services::onboarding::ChecklistStep::ConfigureServer);
    }
    if let Some(summary) = app.state.platform_hints.summary() {
        tracing::info!(platform = %summary, "terminal detected");
    }
    let picked = app.state.onboarding.tips.pick(
        app.state.onboarding.session_count,
        services::onboarding::TipScope::Any,
        services::onboarding::TIP_CATALOG,
    );
    if let Some(tip) = picked {
        app.state.current_tip = Some(*tip);
        app.state.onboarding.tips.mark_shown(app.state.onboarding.session_count);
        if let Err(err) = app.state.onboarding.save() {
            tracing::warn!(%err, "onboarding save after tip pick failed");
        }
    }

    app.dispatch_plugin_init_effects();
    app.spawn_project_detect();
    app.spawn_load_documents();
}

pub fn shutdown_app(app: &mut App) -> anyhow::Result<()> {
    app.state.worktree_manager.cleanup_all();
    app.state.monitors.kill_all();
    app.state.lsp.shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
    app.state.plugins.on_shutdown();

    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::PopKeyboardEnhancementFlags);
    crossterm::execute!(
        std::io::stdout(),
        crossterm::event::DisableFocusChange,
        crossterm::event::DisableBracketedPaste,
        crossterm::event::DisableMouseCapture,
    )?;
    ratatui::restore();
    Ok(())
}
