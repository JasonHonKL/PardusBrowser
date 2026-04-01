use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url;

// ---------------------------------------------------------------------------
// Semantic Tree
// ---------------------------------------------------------------------------

/// The semantic tree extracted from an HTML page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticTree {
    pub root: SemanticNode,
    pub stats: TreeStats,
}

/// A node in the semantic tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticNode {
    pub role: SemanticRole,
    pub name: Option<String>,
    pub tag: String,
    #[serde(rename = "interactive")]
    pub is_interactive: bool,
    #[serde(skip_serializing_if = "is_false", default)]
    pub is_disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub children: Vec<SemanticNode>,
}

fn is_false(v: &bool) -> bool {
    !v
}

/// Statistics about the semantic tree.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TreeStats {
    pub landmarks: usize,
    pub links: usize,
    pub headings: usize,
    pub actions: usize,
    pub forms: usize,
    pub images: usize,
    pub total_nodes: usize,
}

// ---------------------------------------------------------------------------
// Semantic Role
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticRole {
    Document,
    Banner,
    Navigation,
    Main,
    ContentInfo,
    Complementary,
    Region,
    Form,
    Search,
    Article,
    Heading { level: u8 },
    Link,
    Button,
    TextBox,
    Checkbox,
    Radio,
    Combobox,
    List,
    ListItem,
    Table,
    Row,
    Cell,
    ColumnHeader,
    RowHeader,
    Image,
    Dialog,
    Generic,
    StaticText,
    Other(String),
}

impl Serialize for SemanticRole {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SemanticRole {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(parse_role_str(&s))
    }
}

impl fmt::Display for SemanticRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Heading { level } => write!(f, "heading (h{level})"),
            Self::Other(s) => write!(f, "{s}"),
            _ => write!(f, "{}", self.role_str()),
        }
    }
}

impl SemanticRole {
    pub fn role_str(&self) -> &str {
        match self {
            Self::Document => "document",
            Self::Banner => "banner",
            Self::Navigation => "navigation",
            Self::Main => "main",
            Self::ContentInfo => "contentinfo",
            Self::Complementary => "complementary",
            Self::Region => "region",
            Self::Form => "form",
            Self::Search => "search",
            Self::Article => "article",
            Self::Heading { .. } => "heading",
            Self::Link => "link",
            Self::Button => "button",
            Self::TextBox => "textbox",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio",
            Self::Combobox => "combobox",
            Self::List => "list",
            Self::ListItem => "listitem",
            Self::Table => "table",
            Self::Row => "row",
            Self::Cell => "cell",
            Self::ColumnHeader => "columnheader",
            Self::RowHeader => "rowheader",
            Self::Image => "img",
            Self::Dialog => "dialog",
            Self::Generic => "generic",
            Self::StaticText => "text",
            Self::Other(s) => s.as_str(),
        }
    }

    pub fn is_landmark(&self) -> bool {
        matches!(
            self,
            Self::Banner
                | Self::Navigation
                | Self::Main
                | Self::ContentInfo
                | Self::Complementary
                | Self::Region
                | Self::Form
                | Self::Search
        )
    }

    pub fn is_heading(&self) -> bool {
        matches!(self, Self::Heading { .. })
    }
}

impl SemanticTree {
    /// Build a semantic tree from parsed HTML.
    pub fn build(html: &Html, base_url: &str) -> Self {
        let mut stats = TreeStats::default();
        let mut builder = TreeBuilder {
            base_url,
            stats: &mut stats,
        };

        let root = builder.build_from_html(html);
        stats.total_nodes = count_nodes(&root);
        Self { root, stats }
    }
}

fn count_nodes(node: &SemanticNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

// ---------------------------------------------------------------------------
// Tree Builder
// ---------------------------------------------------------------------------

struct TreeBuilder<'a> {
    base_url: &'a str,
    stats: &'a mut TreeStats,
}

impl<'a> TreeBuilder<'a> {
    fn build_from_html(&mut self, html: &Html) -> SemanticNode {
        let body_selector = Selector::parse("body").unwrap();
        let root = SemanticNode {
            role: SemanticRole::Document,
            name: None,
            tag: "document".to_string(),
            is_interactive: false,
            is_disabled: false,
            href: None,
            action: None,
            children: Vec::new(),
        };

        let mut children = Vec::new();
        if let Some(body_el) = html.select(&body_selector).next() {
            for child_node in body_el.children() {
                if let Some(child_el) = ElementRef::wrap(child_node) {
                    if let Some(node) = self.walk_element(&child_el) {
                        children.push(node);
                    }
                }
            }
        }

        SemanticNode { children, ..root }
    }

    fn walk_element(&mut self, el: &ElementRef) -> Option<SemanticNode> {
        let tag = el.value().name().to_lowercase();
        let tag_str = tag.as_str();

        // Skip metadata elements
        if matches!(tag_str, "script" | "style" | "link" | "meta" | "noscript" | "head") {
            return None;
        }

        // Skip hidden elements
        if el.value().attr("hidden").is_some()
            || el.value().attr("aria-hidden") == Some("true")
        {
            return None;
        }

        // Compute role
        let name = self.compute_name(el);
        let has_name = name.is_some();
        let role = self.compute_role(tag_str, el, has_name);

        // Check interactivity
        let is_interactive = self.check_interactive(tag_str, el);
        let action = self.compute_action(tag_str, el, is_interactive);
        let is_disabled = el.value().attr("disabled").is_some();

        // Get href for links
        let href = if tag_str == "a" {
            el.value().attr("href").map(|h| self.resolve_url(h))
        } else {
            None
        };

        // Walk children
        let mut child_nodes = Vec::new();
        for child_node in el.children() {
            if let Some(child_el) = ElementRef::wrap(child_node) {
                if let Some(child) = self.walk_element(&child_el) {
                    child_nodes.push(child);
                }
            }
        }

        // Prune structural nodes without names
        let is_structural = matches!(role, SemanticRole::Generic);
        if is_structural && !has_name && href.is_none() && !is_interactive {
            if child_nodes.is_empty() {
                return None;
            }
            if child_nodes.len() == 1 {
                return Some(child_nodes.remove(0));
            }
        }

        // Update stats
        if role.is_landmark() {
            self.stats.landmarks += 1;
        }
        if matches!(role, SemanticRole::Link) {
            self.stats.links += 1;
        }
        if role.is_heading() {
            self.stats.headings += 1;
        }
        if matches!(role, SemanticRole::Form) {
            self.stats.forms += 1;
        }
        if matches!(role, SemanticRole::Image) {
            self.stats.images += 1;
        }
        if is_interactive {
            self.stats.actions += 1;
        }

        Some(SemanticNode {
            role,
            name,
            tag: tag_str.to_string(),
            is_interactive,
            is_disabled,
            href,
            action,
            children: child_nodes,
        })
    }

    fn compute_name(&self, el: &ElementRef) -> Option<String> {
        // aria-label
        if let Some(label) = el.value().attr("aria-label") {
            let trimmed = label.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }

        // title
        if let Some(title) = el.value().attr("title") {
            let trimmed = title.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }

        // alt for images
        if el.value().name() == "img" {
            if let Some(alt) = el.value().attr("alt") {
                let trimmed = alt.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }

        // placeholder for inputs
        if matches!(el.value().name(), "input" | "textarea") {
            if let Some(p) = el.value().attr("placeholder") {
                let trimmed = p.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }

        // text content for buttons, links, headings
        let tag = el.value().name();
        if matches!(tag, "a" | "button" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "summary") {
            let text = el.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }

        // value for submit/reset buttons
        if tag == "input" {
            let input_type = el.value().attr("type").unwrap_or("text");
            if matches!(input_type, "submit" | "reset" | "button" | "image") {
                if let Some(value) = el.value().attr("value") {
                    let trimmed = value.trim().to_string();
                    if !trimmed.is_empty() {
                        return Some(trimmed);
                    }
                }
                return Some(match input_type {
                    "submit" => "Submit".to_string(),
                    "reset" => "Reset".to_string(),
                    _ => "Button".to_string(),
                });
            }
        }

        None
    }

    fn compute_role(&self, tag: &str, el: &ElementRef, has_name: bool) -> SemanticRole {
        // Check explicit role attribute first
        if let Some(role_str) = el.value().attr("role") {
            return parse_role_str(role_str);
        }

        // Implicit roles based on tag
        match tag {
            "nav" => SemanticRole::Navigation,
            "main" => SemanticRole::Main,
            "header" => SemanticRole::Banner,
            "footer" => SemanticRole::ContentInfo,
            "aside" => SemanticRole::Complementary,
            "search" => SemanticRole::Search,
            "section" if has_name => SemanticRole::Region,
            "article" => SemanticRole::Article,
            "form" if has_name => SemanticRole::Form,

            "h1" => SemanticRole::Heading { level: 1 },
            "h2" => SemanticRole::Heading { level: 2 },
            "h3" => SemanticRole::Heading { level: 3 },
            "h4" => SemanticRole::Heading { level: 4 },
            "h5" => SemanticRole::Heading { level: 5 },
            "h6" => SemanticRole::Heading { level: 6 },

            "a" => SemanticRole::Link,
            "button" => SemanticRole::Button,
            "input" => {
                match el.value().attr("type").unwrap_or("text") {
                    "checkbox" => SemanticRole::Checkbox,
                    "radio" => SemanticRole::Radio,
                    _ => SemanticRole::TextBox,
                }
            }
            "select" => SemanticRole::Combobox,
            "textarea" => SemanticRole::TextBox,
            "img" => SemanticRole::Image,
            "ul" | "ol" => SemanticRole::List,
            "li" => SemanticRole::ListItem,
            "table" => SemanticRole::Table,
            "dialog" => SemanticRole::Dialog,

            _ => SemanticRole::Generic,
        }
    }

    fn check_interactive(&self, tag: &str, el: &ElementRef) -> bool {
        // Native interactive
        if matches!(tag, "a" | "button" | "input" | "select" | "textarea" | "details") {
            return !(tag == "a" && el.value().attr("href").is_none());
        }

        // ARIA interactive
        if let Some(role) = el.value().attr("role") {
            if matches!(
                role,
                "button" | "link" | "textbox" | "checkbox"
                    | "radio" | "combobox" | "switch" | "tab"
                    | "menuitem" | "option"
            ) {
                return true;
            }
        }

        // Focusable
        if let Some(tabindex) = el.value().attr("tabindex") {
            if let Ok(idx) = tabindex.parse::<i32>() {
                if idx >= 0 {
                    return true;
                }
            }
        }

        false
    }

    fn compute_action(&self, tag: &str, el: &ElementRef, is_interactive: bool) -> Option<String> {
        if !is_interactive {
            return None;
        }

        match tag {
            "a" => Some("navigate".to_string()),
            "button" => Some("click".to_string()),
            "input" => {
                let input_type = el.value().attr("type").unwrap_or("text");
                Some(match input_type {
                    "submit" | "reset" | "button" | "image" => "click".to_string(),
                    "checkbox" | "radio" => "toggle".to_string(),
                    _ => "fill".to_string(),
                })
            }
            "select" => Some("select".to_string()),
            "textarea" => Some("fill".to_string()),
            _ => {
                if let Some(role) = el.value().attr("role") {
                    match role {
                        "button" => Some("click".to_string()),
                        "link" => Some("navigate".to_string()),
                        "textbox" => Some("fill".to_string()),
                        _ => None,
                    }
                } else {
                    None
                }
            }
        }
    }

    fn resolve_url(&self, href: &str) -> String {
        Url::parse(self.base_url)
            .and_then(|base| base.join(href))
            .map(|u| u.to_string())
            .unwrap_or_else(|_| href.to_string())
    }
}

fn parse_role_str(s: &str) -> SemanticRole {
    match s {
        "document" => SemanticRole::Document,
        "banner" => SemanticRole::Banner,
        "navigation" => SemanticRole::Navigation,
        "main" => SemanticRole::Main,
        "contentinfo" => SemanticRole::ContentInfo,
        "complementary" => SemanticRole::Complementary,
        "region" => SemanticRole::Region,
        "form" => SemanticRole::Form,
        "search" => SemanticRole::Search,
        "article" => SemanticRole::Article,
        "link" => SemanticRole::Link,
        "button" => SemanticRole::Button,
        "textbox" => SemanticRole::TextBox,
        "checkbox" => SemanticRole::Checkbox,
        "radio" => SemanticRole::Radio,
        "combobox" => SemanticRole::Combobox,
        "list" => SemanticRole::List,
        "listitem" => SemanticRole::ListItem,
        "table" => SemanticRole::Table,
        "img" => SemanticRole::Image,
        "dialog" => SemanticRole::Dialog,
        _ => SemanticRole::Other(s.to_string()),
    }
}
