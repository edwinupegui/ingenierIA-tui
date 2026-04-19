use crate::actions::Action;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use futures_util::StreamExt;
use tokio::sync::mpsc::Sender;

pub async fn run(tx: Sender<Action>) {
    let mut stream = EventStream::new();

    while let Some(Ok(event)) = stream.next().await {
        let action = match event {
            // Only handle key press events (ignore release/repeat from Kitty protocol)
            Event::Key(key) if key.kind == KeyEventKind::Press => match (key.modifiers, key.code) {
                (_, KeyCode::Esc) => Action::KeyEsc,
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::KeyCtrlC,
                (KeyModifiers::CONTROL, KeyCode::Char('b')) => Action::ChatToggleSidebar,
                (KeyModifiers::CONTROL, KeyCode::Char('z')) => Action::InputUndo,
                (KeyModifiers::CONTROL, KeyCode::Char('y')) => Action::InputRedo,
                (m, KeyCode::Up) if m.contains(KeyModifiers::CONTROL) => Action::ChatNavPrev,
                (m, KeyCode::Down) if m.contains(KeyModifiers::CONTROL) => Action::ChatNavNext,
                (m, KeyCode::Up) if m.contains(KeyModifiers::ALT) => Action::PreviewScrollUp,
                (m, KeyCode::Down) if m.contains(KeyModifiers::ALT) => Action::PreviewScrollDown,
                (m, KeyCode::Up) if m.contains(KeyModifiers::SHIFT) => Action::PreviewScrollUp,
                (m, KeyCode::Down) if m.contains(KeyModifiers::SHIFT) => Action::PreviewScrollDown,
                (_, KeyCode::PageUp) => Action::PreviewScrollUp,
                (_, KeyCode::PageDown) => Action::PreviewScrollDown,
                (_, KeyCode::Char(c)) => Action::KeyChar(c),
                (_, KeyCode::Backspace) => Action::KeyBackspace,
                (_, KeyCode::Delete) => Action::KeyDelete,
                (m, KeyCode::Enter) if m.contains(KeyModifiers::ALT) => Action::KeyShiftEnter,
                (m, KeyCode::Enter) if m.contains(KeyModifiers::CONTROL) => Action::KeyShiftEnter,
                (m, KeyCode::Enter) if m.contains(KeyModifiers::SHIFT) => Action::KeyShiftEnter,
                (_, KeyCode::Enter) => Action::KeyEnter,
                (_, KeyCode::Tab) => Action::KeyTab,
                (_, KeyCode::BackTab) => Action::KeyBackTab,
                (_, KeyCode::Up) => Action::KeyUp,
                (_, KeyCode::Down) => Action::KeyDown,
                (_, KeyCode::Left) => Action::KeyLeft,
                (_, KeyCode::Right) => Action::KeyRight,
                (_, KeyCode::Home) => Action::KeyHome,
                (_, KeyCode::End) => Action::KeyEnd,
                _ => continue,
            },
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::ScrollUp => Action::MouseScrollUp(mouse.column),
                MouseEventKind::ScrollDown => Action::MouseScrollDown(mouse.column),
                _ => continue,
            },
            Event::Paste(text) => Action::Paste(text),
            Event::FocusGained => Action::FocusGained,
            Event::FocusLost => Action::FocusLost,
            _ => continue,
        };

        if tx.send(action).await.is_err() {
            break;
        }
    }
}
