//! CSP directive types and fallback chain logic.

/// The 14 CSP directives supported by PardusBrowser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CspDirectiveKind {
    DefaultSrc,
    ScriptSrc,
    StyleSrc,
    ImgSrc,
    ConnectSrc,
    FontSrc,
    FrameSrc,
    MediaSrc,
    ObjectSrc,
    BaseUri,
    FormAction,
    NavigateTo,
    Sandbox,
    UpgradeInsecureRequests,
}

impl CspDirectiveKind {
    /// Parse a directive name string into a `CspDirectiveKind`.
    /// Returns `None` for unknown directive names.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "default-src" => Some(Self::DefaultSrc),
            "script-src" => Some(Self::ScriptSrc),
            "style-src" => Some(Self::StyleSrc),
            "img-src" => Some(Self::ImgSrc),
            "connect-src" => Some(Self::ConnectSrc),
            "font-src" => Some(Self::FontSrc),
            "frame-src" => Some(Self::FrameSrc),
            "media-src" => Some(Self::MediaSrc),
            "object-src" => Some(Self::ObjectSrc),
            "base-uri" => Some(Self::BaseUri),
            "form-action" => Some(Self::FormAction),
            "navigate-to" => Some(Self::NavigateTo),
            "sandbox" => Some(Self::Sandbox),
            "upgrade-insecure-requests" => Some(Self::UpgradeInsecureRequests),
            _ => None,
        }
    }

    /// The directive name as it appears in a CSP header.
    pub fn name(&self) -> &'static str {
        match self {
            Self::DefaultSrc => "default-src",
            Self::ScriptSrc => "script-src",
            Self::StyleSrc => "style-src",
            Self::ImgSrc => "img-src",
            Self::ConnectSrc => "connect-src",
            Self::FontSrc => "font-src",
            Self::FrameSrc => "frame-src",
            Self::MediaSrc => "media-src",
            Self::ObjectSrc => "object-src",
            Self::BaseUri => "base-uri",
            Self::FormAction => "form-action",
            Self::NavigateTo => "navigate-to",
            Self::Sandbox => "sandbox",
            Self::UpgradeInsecureRequests => "upgrade-insecure-requests",
        }
    }

    /// Returns the fallback directive per the CSP spec.
    /// Most fetch directives fall back to `default-src`.
    /// `base-uri`, `form-action`, `navigate-to`, `sandbox`, and
    /// `upgrade-insecure-requests` have no fallback.
    pub fn fallback(&self) -> Option<CspDirectiveKind> {
        match self {
            Self::DefaultSrc
            | Self::BaseUri
            | Self::FormAction
            | Self::NavigateTo
            | Self::Sandbox
            | Self::UpgradeInsecureRequests => None,
            _ => Some(Self::DefaultSrc),
        }
    }

    /// Returns true if this directive uses a source list (vs being a flag).
    pub fn has_source_list(&self) -> bool {
        !matches!(self, Self::Sandbox | Self::UpgradeInsecureRequests)
    }
}

/// Hash algorithms used in CSP hash source expressions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Sha256,
    Sha384,
    Sha512,
}

impl HashAlgorithm {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "sha256" => Some(Self::Sha256),
            "sha384" => Some(Self::Sha384),
            "sha512" => Some(Self::Sha512),
            _ => None,
        }
    }
}

/// A single CSP directive with its source list.
#[derive(Debug, Clone)]
pub struct CspDirective {
    pub kind: CspDirectiveKind,
    pub sources: Vec<crate::csp::source::CspSource>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_name_all_directives() {
        let directives = [
            "default-src", "script-src", "style-src", "img-src",
            "connect-src", "font-src", "frame-src", "media-src",
            "object-src", "base-uri", "form-action", "navigate-to",
            "sandbox", "upgrade-insecure-requests",
        ];
        for name in &directives {
            assert!(CspDirectiveKind::from_name(name).is_some(), "Unknown: {}", name);
        }
    }

    #[test]
    fn test_from_name_unknown() {
        assert!(CspDirectiveKind::from_name("unknown-directive").is_none());
        assert!(CspDirectiveKind::from_name("").is_none());
    }

    #[test]
    fn test_name_roundtrip() {
        use CspDirectiveKind::*;
        let all = [DefaultSrc, ScriptSrc, StyleSrc, ImgSrc, ConnectSrc,
            FontSrc, FrameSrc, MediaSrc, ObjectSrc, BaseUri, FormAction,
            NavigateTo, Sandbox, UpgradeInsecureRequests];
        for kind in &all {
            assert_eq!(CspDirectiveKind::from_name(kind.name()), Some(*kind));
        }
    }

    #[test]
    fn test_fallback_chain() {
        use CspDirectiveKind::*;
        // Fetch directives fall back to default-src
        assert_eq!(ScriptSrc.fallback(), Some(DefaultSrc));
        assert_eq!(StyleSrc.fallback(), Some(DefaultSrc));
        assert_eq!(ImgSrc.fallback(), Some(DefaultSrc));
        assert_eq!(ConnectSrc.fallback(), Some(DefaultSrc));
        assert_eq!(FontSrc.fallback(), Some(DefaultSrc));
        assert_eq!(FrameSrc.fallback(), Some(DefaultSrc));
        assert_eq!(MediaSrc.fallback(), Some(DefaultSrc));
        assert_eq!(ObjectSrc.fallback(), Some(DefaultSrc));

        // These have no fallback
        assert_eq!(DefaultSrc.fallback(), None);
        assert_eq!(BaseUri.fallback(), None);
        assert_eq!(FormAction.fallback(), None);
        assert_eq!(NavigateTo.fallback(), None);
        assert_eq!(Sandbox.fallback(), None);
        assert_eq!(UpgradeInsecureRequests.fallback(), None);
    }

    #[test]
    fn test_has_source_list() {
        use CspDirectiveKind::*;
        assert!(DefaultSrc.has_source_list());
        assert!(ScriptSrc.has_source_list());
        assert!(!Sandbox.has_source_list());
        assert!(!UpgradeInsecureRequests.has_source_list());
    }
}
