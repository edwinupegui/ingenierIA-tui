use crate::domain::event::IngenieriaEvent;

// ── Eventos con timestamp ────────────────────────────────────────────────────

pub struct TimedEvent {
    pub time: String, // "HH:MM:SS" extraído o del reloj
    pub event: IngenieriaEvent,
}

impl TimedEvent {
    pub fn new(event: IngenieriaEvent) -> Self {
        let time = time_from_event(&event).unwrap_or_else(system_time_str);
        Self { time, event }
    }
}

fn time_from_event(event: &IngenieriaEvent) -> Option<String> {
    let ts = match event {
        IngenieriaEvent::Connected { timestamp, .. } => timestamp.as_str(),
        IngenieriaEvent::Sync { timestamp, .. } => timestamp.as_str(),
        IngenieriaEvent::Reload { timestamp, .. } => timestamp.as_str(),
        IngenieriaEvent::Heartbeat { timestamp, .. } => timestamp.as_str(),
        _ => return None,
    };
    // Extraer HH:MM:SS de ISO "2025-03-20T14:32:01.000Z"
    let timestamp = ts.find('T').map(|i| &ts[i + 1..])?;
    Some(timestamp.chars().take(8).collect())
}

pub fn system_time_str() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{:02}:{:02}:{:02}", (secs / 3600) % 24, (secs / 60) % 60, secs % 60)
}

// ── Sessions activas ─────────────────────────────────────────────────────────

pub struct ActiveSession {
    pub developer: String,
    pub time: String,
}
