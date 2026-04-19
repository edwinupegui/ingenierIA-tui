use crate::state::{ChatMessage, ChatRole};

use super::App;

impl App {
    /// Handle /apply [n] command — apply code block to file.
    pub(crate) fn handle_apply_command(&mut self, arg: &str) {
        if self.state.chat.code_blocks.is_empty() {
            self.notify("No hay code blocks detectados".to_string());
            return;
        }

        if !arg.is_empty() {
            if let Ok(n) = arg.parse::<usize>() {
                if n == 0 || n > self.state.chat.code_blocks.len() {
                    self.notify(format!(
                        "Índice fuera de rango (1-{})",
                        self.state.chat.code_blocks.len()
                    ));
                    return;
                }
                self.state.chat.code_block_cursor = n - 1;
            }
        }

        self.apply_selected_code_block();
    }

    /// Handle /blocks command — list detected code blocks.
    pub(crate) fn handle_blocks_command(&mut self) {
        if self.state.chat.code_blocks.is_empty() {
            self.notify("No hay code blocks detectados".to_string());
            return;
        }

        let mut msg = String::from("## Code blocks detectados\n\n");
        for (i, block) in self.state.chat.code_blocks.iter().enumerate() {
            let path = block.file_path.as_deref().unwrap_or("(sin ruta)");
            let selected = if i == self.state.chat.code_block_cursor { " ◀" } else { "" };
            msg.push_str(&format!(
                "{}. `{}` — {} ({} líneas){}\n",
                i + 1,
                path,
                block.lang,
                block.content.lines().count(),
                selected,
            ));
        }
        msg.push_str("\nUsa `/apply <n>` para aplicar.");

        self.state.chat.messages.push(ChatMessage::new(ChatRole::Assistant, msg));
        self.state.chat.scroll_offset = u16::MAX;
    }

    /// Scan the last assistant message for code blocks.
    pub(crate) fn detect_code_blocks(&mut self) {
        let blocks = if let Some(last) = self.state.chat.messages.last() {
            if last.role == ChatRole::Assistant {
                crate::services::codeblocks::extract_code_blocks(&last.content)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let has_applicable = blocks.iter().any(|b| b.file_path.is_some());
        self.state.chat.code_blocks = blocks;
        self.state.chat.code_block_cursor = 0;

        if has_applicable {
            let count =
                self.state.chat.code_blocks.iter().filter(|b| b.file_path.is_some()).count();
            self.notify(format!(
                "{count} code block(s) aplicables — Tab para navegar, Enter para aplicar"
            ));
        }
    }

    /// Apply the currently selected code block to its file.
    pub(crate) fn apply_selected_code_block(&mut self) {
        let Some(block) = self.state.chat.code_blocks.get(self.state.chat.code_block_cursor) else {
            self.notify("No hay code block seleccionado".to_string());
            return;
        };

        let Some(path) = &block.file_path else {
            self.notify("Este code block no tiene ruta de archivo".to_string());
            return;
        };

        let path = path.clone();
        let content = block.content.clone();
        let tx = self.tx.clone();

        self.hooks.fire(
            crate::services::hooks::HookTrigger::PreCodeApply,
            crate::services::hooks::HookContext::for_code_apply(&path),
            tx.clone(),
        );

        tokio::spawn(async move {
            let p = std::path::Path::new(&path);
            if let Some(parent) = p.parent() {
                if !parent.exists() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
            }
            match tokio::fs::write(p, &content).await {
                Ok(()) => {
                    let msg = format!("✓ Archivo escrito: {path} ({} bytes)", content.len());
                    let _ = tx.send(crate::actions::Action::ChatStreamDelta(String::new())).await;
                    let abs_path = std::path::Path::new(&path)
                        .canonicalize()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| path.clone());
                    let _ = tx
                        .send(crate::actions::Action::CodeBlockApplied {
                            msg,
                            path: Some(abs_path),
                            content: Some(content),
                        })
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(crate::actions::Action::CodeBlockApplied {
                            msg: format!("✗ Error escribiendo {path}: {e}"),
                            path: None,
                            content: None,
                        })
                        .await;
                }
            }
        });
    }

    /// Navigate to next code block.
    #[expect(dead_code, reason = "available for future keyboard shortcut binding")]
    pub(crate) fn next_code_block(&mut self) {
        if self.state.chat.code_blocks.is_empty() {
            return;
        }
        self.state.chat.code_block_cursor =
            (self.state.chat.code_block_cursor + 1) % self.state.chat.code_blocks.len();
    }
}
