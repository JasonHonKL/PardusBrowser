use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use std::fmt;

/// A stable reference to an element within a Page.
///
/// Stores a CSS selector that uniquely identifies the element,
/// plus cached attribute data extracted at creation time.
/// `Clone + Send + Sync` with no lifetime parameters.
#[derive(Debug, Clone, Serialize)]
pub struct ElementHandle {
    /// CSS selector that re-locates this element in the Page's HTML.
    pub selector: String,
    /// The tag name (lowercase).
    pub tag: String,
    /// Element id attribute, if present.
    pub id: Option<String>,
    /// Element name attribute, if present.
    pub name: Option<String>,
    /// The semantic action: "navigate", "click", "fill", "toggle", "select".
    pub action: Option<String>,
    /// Whether the element is disabled.
    pub is_disabled: bool,
    /// The href for links.
    pub href: Option<String>,
    /// The name/label text.
    pub label: Option<String>,
    /// The input type, if applicable.
    pub input_type: Option<String>,
    /// The current value attribute, if present.
    pub value: Option<String>,
}

/// Build a unique CSS selector for an element.
///
/// - If the element has an `id`, uses `#id`.
/// - Otherwise, prefers attribute-based selectors like `input[name="foo"]`
///   if they are unique in the document.
/// - Falls back to a structural path: `body > div:nth-child(2) > form > input`
pub fn build_unique_selector(el: &ElementRef, html: &Html) -> String {
    // Prefer id
    if let Some(id) = el.value().attr("id") {
        return format!("#{}", css_escape_id(id));
    }

    // Try name-based selector
    if let Some(name) = el.value().attr("name") {
        let tag = el.value().name();
        let candidate = format!("{}[name=\"{}\"]", tag, name);
        let is_unique = match Selector::parse(&candidate) {
            Ok(sel) => html.select(&sel).count() == 1,
            Err(_) => false,
        };
        if is_unique {
            return candidate;
        }
    }

    // Try type + name combination
    if let Some(itype) = el.value().attr("type") {
        let tag = el.value().name();
        let candidate = format!("{}[type=\"{}\"]", tag, itype);
        let is_unique = match Selector::parse(&candidate) {
            Ok(sel) => html.select(&sel).count() == 1,
            Err(_) => false,
        };
        if is_unique {
            return candidate;
        }
    }

    // Build structural path
    build_structural_selector(el)
}

fn css_escape_id(id: &str) -> String {
    if id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        id.to_string()
    } else {
        id.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c.to_string()
                } else {
                    format!("\\{:X}", c as u32)
                }
            })
            .collect()
    }
}

fn build_structural_selector(el: &ElementRef) -> String {
    let mut segments = Vec::new();
    let mut current = Some(*el);

    while let Some(node) = current {
        let tag = node.value().name().to_lowercase();

        if tag == "body" || tag == "html" {
            break;
        }

        // Count position among all sibling elements
        let nth = count_element_position(&node);
        segments.push(format!("{}:nth-child({})", tag, nth));

        current = node.parent().and_then(ElementRef::wrap);
    }

    segments.reverse();
    if segments.is_empty() {
        el.value().name().to_string()
    } else {
        segments.join(" > ")
    }
}

/// Count the 1-based position of this element among its parent's children.
fn count_element_position(el: &ElementRef) -> usize {
    if let Some(parent) = el.parent().and_then(ElementRef::wrap) {
        let target_id = el.value().attr("id");
        let target_name = el.value().name();
        let mut count = 0;

        for child in parent.children() {
            if let Some(child_el) = ElementRef::wrap(child) {
                count += 1;
                if child_el.value().name() == target_name
                    && child_el.value().attr("id") == target_id
                {
                    // Approximate match — good enough for structural selectors
                    return count;
                }
            }
        }
    }
    1
}

/// Create an ElementHandle from a scraper ElementRef.
pub fn element_to_handle(el: &ElementRef, html: &Html) -> ElementHandle {
    let selector = build_unique_selector(el, html);
    let tag = el.value().name().to_lowercase();

    let name_attr = el.value().attr("name").map(|s| s.to_string());
    let href = el.value().attr("href").map(|s| s.to_string());
    let input_type = el.value().attr("type").map(|s| s.to_string());
    let value = el.value().attr("value").map(|s| s.to_string());
    let id = el.value().attr("id").map(|s| s.to_string());
    let is_disabled = el.value().attr("disabled").is_some();

    let action = compute_action(&tag, input_type.as_deref());
    let label = compute_label(&tag, el);

    ElementHandle {
        selector,
        tag,
        id,
        name: name_attr,
        action,
        is_disabled,
        href,
        label,
        input_type,
        value,
    }
}

fn compute_action(tag: &str, input_type: Option<&str>) -> Option<String> {
    match tag {
        "a" => Some("navigate".to_string()),
        "button" => Some("click".to_string()),
        "input" => {
            let itype = input_type.unwrap_or("text");
            Some(
                match itype {
                    "submit" | "reset" | "button" | "image" => "click",
                    "checkbox" | "radio" => "toggle",
                    _ => "fill",
                }
                .to_string(),
            )
        }
        "select" => Some("select".to_string()),
        "textarea" => Some("fill".to_string()),
        _ => None,
    }
}

fn compute_label(tag: &str, el: &ElementRef) -> Option<String> {
    if let Some(label) = el.value().attr("aria-label") {
        let trimmed = label.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    if let Some(title) = el.value().attr("title") {
        let trimmed = title.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    if matches!(tag, "input" | "textarea") {
        if let Some(p) = el.value().attr("placeholder") {
            let trimmed = p.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }

    if matches!(tag, "a" | "button") {
        let text = el.text().collect::<String>().trim().to_string();
        if !text.is_empty() {
            return Some(text);
        }
    }

    if tag == "input" {
        let itype = el.value().attr("type").unwrap_or("text");
        if matches!(itype, "submit" | "reset" | "button") {
            if let Some(val) = el.value().attr("value") {
                let trimmed = val.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }
    }

    None
}

impl fmt::Display for ElementHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.tag)?;
        if let Some(id) = &self.id {
            write!(f, "#{}", id)?;
        }
        if let Some(name) = &self.name {
            write!(f, "[name=\"{}\"]", name)?;
        }
        if let Some(label) = &self.label {
            write!(f, " \"{}\"", label)?;
        }
        if let Some(action) = &self.action {
            write!(f, " [{}]", action)?;
        }
        Ok(())
    }
}
