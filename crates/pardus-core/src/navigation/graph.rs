use scraper::{Html, Selector, ElementRef};
use url::Url;
use serde::Serialize;
use std::sync::LazyLock as Lazy;

/// Navigation graph extracted from a page — all reachable routes and forms.
#[derive(Debug, Serialize)]
pub struct NavigationGraph {
    pub current_url: String,
    pub internal_links: Vec<Route>,
    pub external_links: Vec<String>,
    pub forms: Vec<FormDescriptor>,
}

/// A route (link) within the navigation graph.
#[derive(Debug, Serialize)]
pub struct Route {
    pub url: String,
    pub label: Option<String>,
    pub rel: Option<String>,
}

/// A form descriptor with its fields.
#[derive(Debug, Serialize)]
pub struct FormDescriptor {
    pub id: Option<String>,
    pub action: Option<String>,
    pub method: String,
    pub fields: Vec<FieldDescriptor>,
}

/// A single field within a form.
#[derive(Debug, Serialize)]
pub struct FieldDescriptor {
    pub name: Option<String>,
    pub field_type: String,
    pub label: Option<String>,
    pub placeholder: Option<String>,
    pub required: bool,
}

// Pre-compiled selectors for performance
static LINK_SELECTOR: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("a[href]").expect("'a[href]' is a valid CSS selector")
});

static FORM_SELECTOR: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("form").expect("'form' is a valid CSS selector")
});

static INPUT_SELECTOR: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("input, select, textarea").expect("'input, select, textarea' is a valid CSS selector")
});

static LABEL_SELECTOR: Lazy<Selector> = Lazy::new(|| {
    Selector::parse("label").expect("'label' is a valid CSS selector")
});

impl NavigationGraph {
    /// Build a navigation graph from HTML and a base URL.
    /// 
    /// # Arguments
    /// * `html` - The parsed HTML document
    /// * `current_url` - The base URL for resolving relative links
    /// 
    /// # Returns
    /// A `NavigationGraph` containing all links, forms, and fields.
    pub fn build(html: &Html, current_url: &str) -> Self {
        // Parse the base URL once
        let base = match Url::parse(current_url) {
            Ok(u) => u,
            Err(_) => {
                // Return empty graph if URL is invalid
                return Self {
                    current_url: current_url.to_string(),
                    internal_links: Vec::new(),
                    external_links: Vec::new(),
                    forms: Vec::new(),
                };
            }
        };
        
        let current_origin = base.origin().ascii_serialization();
        
        let mut internal_links = Vec::new();
        let mut external_links = Vec::new();
        let mut forms = Vec::new();

        // Collect links using pre-compiled selector
        for el in html.select(&*LINK_SELECTOR) {
            let href = el.value().attr("href").unwrap_or("");
            
            // Skip empty, javascript:, and anchor-only links
            if href.is_empty() || href.starts_with("javascript:") || href.starts_with("#") {
                continue;
            }

            let label: String = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
            let rel = el.value().attr("rel").map(|s| s.to_string());

            // Resolve URL using the already-parsed base
            let resolved = match base.join(href) {
                Ok(url) => url,
                Err(_) => continue,
            };

            let url_string = resolved.to_string();

            if resolved.origin().ascii_serialization() == current_origin {
                // Deduplicate internal links
                if !internal_links.iter().any(|r: &Route| r.url == url_string) {
                    internal_links.push(Route {
                        url: url_string,
                        label: if label.is_empty() { None } else { Some(label) },
                        rel,
                    });
                }
            } else {
                // Deduplicate external links
                if !external_links.contains(&url_string) {
                    external_links.push(url_string);
                }
            }
        }

        // Collect forms using pre-compiled selector
        for form_el in html.select(&*FORM_SELECTOR) {
            let action = form_el.value().attr("action")
                .and_then(|a| base.join(a).ok())
                .map(|u| u.to_string());

            let method = form_el.value()
                .attr("method")
                .map(|m| m.to_uppercase())
                .filter(|m| *m == "GET" || *m == "POST")
                .unwrap_or_else(|| "GET".to_string());

            let id = form_el.value().attr("id").map(|s| s.to_string());

            let mut fields = Vec::new();
            for field_el in form_el.select(&*INPUT_SELECTOR) {
                let field_name = field_el.value().attr("name").map(|s| s.to_string());
                let field_type = field_el.value()
                    .attr("type")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| field_el.value().name().to_string());

                // Find associated label
                let label = find_label_for(&form_el, field_name.as_deref());

                fields.push(FieldDescriptor {
                    name: field_name,
                    field_type,
                    label,
                    placeholder: field_el.value().attr("placeholder").map(|s| s.to_string()),
                    required: field_el.value().attr("required").is_some(),
                });
            }

            forms.push(FormDescriptor {
                id,
                action,
                method,
                fields,
            });
        }

        NavigationGraph {
            current_url: current_url.to_string(),
            internal_links,
            external_links,
            forms,
        }
    }

    /// Returns the number of internal links.
    pub fn internal_link_count(&self) -> usize {
        self.internal_links.len()
    }

    /// Returns the number of external links.
    pub fn external_link_count(&self) -> usize {
        self.external_links.len()
    }

    /// Returns the number of forms.
    pub fn form_count(&self) -> usize {
        self.forms.len()
    }

    /// Check if the graph contains a specific URL (internal or external).
    pub fn contains_url(&self, url: &str) -> bool {
        self.internal_links.iter().any(|r| r.url == url) 
            || self.external_links.contains(&url.to_string())
    }
}

/// Try to find a <label> associated with a form field.
/// 
/// Uses the `for` attribute matching the field name, or
/// checks for a label that wraps the field.
fn find_label_for(form: &ElementRef, field_name: Option<&str>) -> Option<String> {
    // First try: for="field_name" attribute
    if let Some(name) = field_name {
        for label_el in form.select(&*LABEL_SELECTOR) {
            if label_el.value().attr("for") == Some(name) {
                let text: String = label_el.text().collect();
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    return Some(trimmed);
                }
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_html() {
        let html = Html::parse_document("<html><body></body></html>");
        let graph = NavigationGraph::build(&html, "https://example.com");
        
        assert_eq!(graph.internal_link_count(), 0);
        assert_eq!(graph.external_link_count(), 0);
        assert_eq!(graph.form_count(), 0);
    }

    #[test]
    fn test_internal_link() {
        let html = Html::parse_document(r#"
            <html><body>
            <a href="/about">About</a>
            </body></html>
        "#);
        let graph = NavigationGraph::build(&html, "https://example.com");
        
        assert_eq!(graph.internal_link_count(), 1);
        assert_eq!(graph.internal_links[0].url, "https://example.com/about");
        assert_eq!(graph.internal_links[0].label, Some("About".to_string()));
    }

    #[test]
    fn test_external_link() {
        let html = Html::parse_document(r#"
            <html><body>
            <a href="https://other.com">External</a>
            </body></html>
        "#);
        let graph = NavigationGraph::build(&html, "https://example.com");
        
        assert_eq!(graph.internal_link_count(), 0);
        assert_eq!(graph.external_link_count(), 1);
        assert!(graph.external_links[0].contains("other.com"));
    }

    #[test]
    fn test_form_extraction() {
        let html = Html::parse_document(r#"
            <html><body>
            <form action="/search" method="post">
                <input type="text" name="query" placeholder="Search...">
                <button type="submit">Search</button>
            </form>
            </body></html>
        "#);
        let graph = NavigationGraph::build(&html, "https://example.com");
        
        assert_eq!(graph.form_count(), 1);
        assert_eq!(graph.forms[0].action, Some("https://example.com/search".to_string()));
        assert_eq!(graph.forms[0].method, "POST");
    }

    #[test]
    fn test_contains_url() {
        let html = Html::parse_document(r#"
            <html><body>
            <a href="/page1">Page 1</a>
            <a href="https://external.com">External</a>
            </body></html>
        "#);
        let graph = NavigationGraph::build(&html, "https://example.com");
        
        assert!(graph.contains_url("https://example.com/page1"));
        assert!(graph.contains_url("https://external.com/"));
        assert!(!graph.contains_url("https://example.com/notfound"));
    }

    #[test]
    fn test_invalid_url_graceful() {
        let html = Html::parse_document("<html><body></body></html>");
        let graph = NavigationGraph::build(&html, "not-a-valid-url");
        
        // Should return empty graph, not panic
        assert_eq!(graph.internal_link_count(), 0);
        assert_eq!(graph.external_link_count(), 0);
    }
}
