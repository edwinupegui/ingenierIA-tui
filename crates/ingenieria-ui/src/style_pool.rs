//! Style pool — reusable Style cache to reduce allocation in render functions.
//!
//! Instead of creating `Style::default().fg(X).bg(Y).add_modifier(M)` inline
//! every frame, styles are built once and retrieved by key.

use std::collections::HashMap;

use ratatui::style::{Color, Modifier, Style};

/// Key for cached styles: foreground, background, and modifier flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StyleKey {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub modifiers: u16, // Modifier bits
}

impl StyleKey {
    pub fn fg(color: Color) -> Self {
        Self { fg: Some(color), bg: None, modifiers: 0 }
    }

    pub fn fg_bg(fg: Color, bg: Color) -> Self {
        Self { fg: Some(fg), bg: Some(bg), modifiers: 0 }
    }

    pub fn with_modifier(mut self, m: Modifier) -> Self {
        self.modifiers |= m.bits();
        self
    }
}

/// Pool of pre-built ratatui Styles, keyed by (fg, bg, modifiers).
pub struct StylePool {
    cache: HashMap<StyleKey, Style>,
}

impl StylePool {
    pub fn new() -> Self {
        Self { cache: HashMap::with_capacity(32) }
    }

    /// Get or create a Style for the given key.
    pub fn get(&mut self, key: StyleKey) -> Style {
        *self.cache.entry(key).or_insert_with(|| {
            let mut style = Style::default();
            if let Some(fg) = key.fg {
                style = style.fg(fg);
            }
            if let Some(bg) = key.bg {
                style = style.bg(bg);
            }
            if key.modifiers != 0 {
                style = style.add_modifier(Modifier::from_bits_truncate(key.modifiers));
            }
            style
        })
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}

impl Default for StylePool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_key_returns_same_style() {
        let mut pool = StylePool::new();
        let key = StyleKey::fg(Color::Red);
        let s1 = pool.get(key);
        let s2 = pool.get(key);
        assert_eq!(s1, s2);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn different_keys_different_entries() {
        let mut pool = StylePool::new();
        pool.get(StyleKey::fg(Color::Red));
        pool.get(StyleKey::fg(Color::Blue));
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn modifier_key_works() {
        let mut pool = StylePool::new();
        let key = StyleKey::fg(Color::Green).with_modifier(Modifier::BOLD);
        let style = pool.get(key);
        assert_eq!(style, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));
    }

    #[test]
    fn fg_bg_key() {
        let mut pool = StylePool::new();
        let key = StyleKey::fg_bg(Color::White, Color::Black);
        let style = pool.get(key);
        assert_eq!(style, Style::default().fg(Color::White).bg(Color::Black));
    }
}
