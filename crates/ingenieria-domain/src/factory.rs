//! Factory identity type — determines which ingenierIA context is active.

/// UI factory selector. Determines theming, document filtering, and API context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiFactory {
    Net,
    Ang,
    Nest,
    All,
}

impl UiFactory {
    pub fn label(&self) -> &'static str {
        match self {
            UiFactory::Net => "ingenierIA-Net",
            UiFactory::Ang => "ingenierIA-Ang",
            UiFactory::Nest => "ingenierIA-Nest",
            UiFactory::All => "Full Stack",
        }
    }

    pub fn next(&self) -> UiFactory {
        match self {
            UiFactory::Net => UiFactory::Ang,
            UiFactory::Ang => UiFactory::Nest,
            UiFactory::Nest => UiFactory::All,
            UiFactory::All => UiFactory::Net,
        }
    }

    /// Canonical string key for this factory. Always returns `Some`.
    pub fn api_key(&self) -> Option<&'static str> {
        match self {
            UiFactory::Net => Some("net"),
            UiFactory::Ang => Some("ang"),
            UiFactory::Nest => Some("nest"),
            UiFactory::All => Some("all"),
        }
    }

    /// Key for document/query filtering. Returns `None` for `All` (= no filter).
    pub fn filter_key(&self) -> Option<&'static str> {
        match self {
            UiFactory::All => None,
            other => other.api_key(),
        }
    }

    pub fn from_key(key: Option<&str>) -> UiFactory {
        match key {
            Some("net") => UiFactory::Net,
            Some("ang") => UiFactory::Ang,
            Some("nest") => UiFactory::Nest,
            _ => UiFactory::All,
        }
    }

    /// Representative RGB color for each factory.
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            UiFactory::Net => (104, 33, 122), // .NET purple
            UiFactory::Ang => (200, 35, 51),  // Angular red
            UiFactory::Nest => (224, 35, 78), // NestJS red
            UiFactory::All => (72, 187, 120), // green
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_cycles_through_all() {
        let start = UiFactory::Net;
        let after4 = start.next().next().next().next();
        assert_eq!(after4, UiFactory::Net);
    }

    #[test]
    fn from_key_defaults_to_all() {
        assert_eq!(UiFactory::from_key(None), UiFactory::All);
        assert_eq!(UiFactory::from_key(Some("unknown")), UiFactory::All);
    }

    #[test]
    fn filter_key_none_for_all() {
        assert!(UiFactory::All.filter_key().is_none());
        assert_eq!(UiFactory::Net.filter_key(), Some("net"));
    }
}
