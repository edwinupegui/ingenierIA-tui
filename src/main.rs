mod actions;
mod app;
mod bootstrap;
mod config;
mod domain;
mod registries;
mod services;
mod state;
mod ui;
mod utils;
mod workers;

use actions::Action;
use app::App;
use clap::Parser;
use config::{needs_wizard, Config};
use services::IngenieriaClient;
use std::sync::Arc;
use tokio::sync::mpsc;

const ACTION_CHANNEL_CAPACITY: usize = 100;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = bootstrap::Cli::parse();
    bootstrap::init_tracing();
    let config = Config::resolve(cli.server_url);
    let show_wizard = cli.config || needs_wizard();
    tracing::info!(server_url = %config.server_url, wizard = show_wizard, "Config resolved");
    let (tx, mut rx) = mpsc::channel::<Action>(ACTION_CHANNEL_CAPACITY);
    let client = Arc::new(IngenieriaClient::new(&config.server_url));
    let events_url = client.events_url();
    bootstrap::setup_terminal()?;
    let mut terminal = ratatui::init();
    bootstrap::spawn_workers(tx.clone(), client.clone(), &config, events_url);
    let mut app = App::new(client, tx.clone(), config, show_wizard, cli.config, cli.mock);
    bootstrap::init_app(&mut app, tx.clone(), show_wizard);
    let mut throttle = ui::frame_throttle::FrameThrottle::default();
    let render_stats = ui::buffer_diff::is_stats_enabled();
    let result = loop {
        if throttle.should_draw() {
            if let Err(e) = terminal.draw(|f| ui::render(f, &app.state)) {
                break Err(e.into());
            }
            if render_stats {
                bootstrap::log_render_stats(&throttle);
            }
            throttle.mark_drawn();
        } else {
            throttle.mark_skipped();
        }
        match rx.recv().await {
            Some(action) => {
                if app.handle(action) {
                    break Ok(());
                }
                if app.state.config_dirty {
                    app.state.config_dirty = false;
                    app.save_config();
                }
            }
            None => {
                tracing::error!("Action channel closed");
                break Ok(());
            }
        }
    };
    bootstrap::shutdown_app(&mut app)?;
    result
}
