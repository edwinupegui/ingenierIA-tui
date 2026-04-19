/// Design system tokens — consistent constants for spacing, borders, and z-index.
use ratatui::widgets::BorderType;

pub mod spacing {
    pub const XS: u16 = 0;
    pub const SM: u16 = 1;
    pub const MD: u16 = 2;
    pub const LG: u16 = 3;
}

pub mod z_index {
    /// Base screens (splash, dashboard, chat).
    pub const BASE: u8 = 0;
    /// Panels that overlay content (tool monitor, agents).
    pub const OVERLAY: u8 = 10;
    /// Modal dialogs (permission, doctor).
    pub const MODAL: u8 = 20;
    /// Toasts (always visible above modals).
    pub const TOAST: u8 = 30;
    /// Autocomplete popups (highest layer).
    pub const POPUP: u8 = 40;
}

pub mod border {
    use super::BorderType;

    pub const DEFAULT: BorderType = BorderType::Rounded;
    pub const EMPHASIS: BorderType = BorderType::Double;
    pub const SUBTLE: BorderType = BorderType::Plain;
}
