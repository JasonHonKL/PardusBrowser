use crate::navigation::graph::NavigationGraph;
use crate::semantic::tree::{SemanticNode, SemanticRole, SemanticTree};
use serde::{Deserialize, Serialize};

/// A suggested action for an AI agent based on page state analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedAction {
    pub action_type: ActionType,
    pub element_id: Option<usize>,
    pub selector: Option<String>,
    pub label: Option<String>,
    pub reason: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    Click,
    Navigate,
    Fill,
    Toggle,
    Select,
    ScrollDown,
    Wait,
    Submit,
}

/// An action plan: a prioritized list of suggested next actions for the current page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    pub url: String,
    pub suggestions: Vec<SuggestedAction>,
    pub page_type: PageType,
    pub has_forms: bool,
    pub has_pagination: bool,
    pub interactive_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PageType {
    LoginPage,
    SearchPage,
    NavigationPage,
    ContentPage,
    FormPage,
    ListingPage,
    Unknown,
}

impl ActionPlan {
    /// Analyze a page's semantic tree and navigation graph to produce
    /// a prioritized list of suggested actions.
    pub fn analyze(url: &str, tree: &SemanticTree, nav: Option<&NavigationGraph>) -> Self {
        let mut suggestions = Vec::new();
        let page_type = classify_page(tree, nav);

        collect_form_suggestions(tree, &mut suggestions, &page_type);
        collect_link_suggestions(tree, nav, &mut suggestions, &page_type);
        collect_input_suggestions(tree, &mut suggestions);

        if has_pagination_signals(nav) {
            suggestions.push(SuggestedAction {
                action_type: ActionType::ScrollDown,
                element_id: None,
                selector: None,
                label: None,
                reason: "Page may have more content below (pagination links detected)".to_string(),
                confidence: 0.6,
            });
        }

        suggestions.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let interactive_count = tree.stats.actions;

        Self {
            url: url.to_string(),
            suggestions,
            page_type,
            has_forms: tree.stats.forms > 0,
            has_pagination: has_pagination_signals(nav),
            interactive_count,
        }
    }
}

fn classify_page(tree: &SemanticTree, nav: Option<&NavigationGraph>) -> PageType {
    let has_password = tree_has_input_type(&tree.root, "password");
    let has_search = tree_has_input_type(&tree.root, "search")
        || has_attr_containing(&tree.root, "role", "search")
        || tree
            .root
            .children
            .iter()
            .any(|c| matches!(c.role, SemanticRole::Search))
        || has_search_button(&tree.root);

    if has_password {
        return PageType::LoginPage;
    }
    if has_search {
        return PageType::SearchPage;
    }
    if tree.stats.forms > 0 && tree.stats.links < 3 {
        return PageType::FormPage;
    }
    if let Some(nav_graph) = nav {
        if nav_graph.internal_links.len() > 5 && tree.stats.forms == 0 {
            return PageType::NavigationPage;
        }
        if nav_graph.internal_links.len() >= 3 && has_pagination_signals(Some(nav_graph)) {
            return PageType::ListingPage;
        }
    }
    if tree.stats.links > 0 || tree.stats.headings > 0 {
        return PageType::ContentPage;
    }
    PageType::Unknown
}

fn tree_has_input_type(node: &SemanticNode, target_type: &str) -> bool {
    if node.tag == "input" {
        if let Some(ref input_type) = node.input_type {
            if input_type == target_type {
                return true;
            }
        }
    }
    for child in &node.children {
        if tree_has_input_type(child, target_type) {
            return true;
        }
    }
    false
}

fn has_attr_containing(node: &SemanticNode, _attr: &str, _val: &str) -> bool {
    if matches!(node.role, SemanticRole::Search) {
        return true;
    }
    for child in &node.children {
        if has_attr_containing(child, _attr, _val) {
            return true;
        }
    }
    false
}

/// Check if the page has a submit button whose label contains "search".
fn has_search_button(node: &SemanticNode) -> bool {
    if matches!(node.role, SemanticRole::Button) {
        if let Some(name) = &node.name {
            if name.to_lowercase().contains("search") {
                return true;
            }
        }
    }
    for child in &node.children {
        if has_search_button(child) {
            return true;
        }
    }
    false
}

fn has_pagination_signals(nav: Option<&NavigationGraph>) -> bool {
    let nav_graph = match nav {
        Some(g) => g,
        None => return false,
    };

    for link in &nav_graph.internal_links {
        let url = &link.url;
        if url.contains("page=")
            || url.contains("/page/")
            || url.contains("p=")
            || url.contains("offset=")
            || url.contains("start=")
        {
            return true;
        }
        if let Some(label) = &link.label {
            let lower = label.to_lowercase();
            if lower.contains("next")
                || lower.contains("more")
                || lower.contains("load more")
                || lower.contains("older")
            {
                return true;
            }
        }
    }
    false
}

fn collect_form_suggestions(
    tree: &SemanticTree,
    suggestions: &mut Vec<SuggestedAction>,
    page_type: &PageType,
) {
    if tree.stats.forms == 0 {
        return;
    }

    collect_interactive_nodes(&tree.root, &mut |node| {
        if node.element_id.is_none() || node.is_disabled {
            return;
        }

        match node.role {
            SemanticRole::TextBox => {
                let name = node.name.as_deref().unwrap_or("field");
                let confidence = if page_type == &PageType::SearchPage {
                    0.9
                } else {
                    0.7
                };
                suggestions.push(SuggestedAction {
                    action_type: ActionType::Fill,
                    element_id: node.element_id,
                    selector: node.selector.clone(),
                    label: node.name.clone(),
                    reason: format!("Fill {} field", name),
                    confidence,
                });
            }
            SemanticRole::Combobox => {
                let name = node.name.as_deref().unwrap_or("dropdown");
                suggestions.push(SuggestedAction {
                    action_type: ActionType::Select,
                    element_id: node.element_id,
                    selector: node.selector.clone(),
                    label: node.name.clone(),
                    reason: format!("Select an option from {}", name),
                    confidence: 0.7,
                });
            }
            SemanticRole::Checkbox | SemanticRole::Radio => {
                suggestions.push(SuggestedAction {
                    action_type: ActionType::Toggle,
                    element_id: node.element_id,
                    selector: node.selector.clone(),
                    label: node.name.clone(),
                    reason: format!("Toggle {}", node.role.role_str()),
                    confidence: 0.5,
                });
            }
            SemanticRole::Button => {
                let name = node.name.as_deref().unwrap_or("");
                let is_submit = name.to_lowercase().contains("submit")
                    || name.to_lowercase().contains("search")
                    || name.to_lowercase().contains("login")
                    || name.to_lowercase().contains("sign in")
                    || name.to_lowercase().contains("go");

                suggestions.push(SuggestedAction {
                    action_type: if is_submit {
                        ActionType::Submit
                    } else {
                        ActionType::Click
                    },
                    element_id: node.element_id,
                    selector: node.selector.clone(),
                    label: node.name.clone(),
                    reason: if is_submit {
                        format!("Submit form via \"{}\"", name)
                    } else {
                        format!("Click button \"{}\"", name)
                    },
                    confidence: if is_submit { 0.8 } else { 0.5 },
                });
            }
            SemanticRole::Link => {
                if let Some(name) = &node.name {
                    let lower = name.to_lowercase();
                    if lower.contains("submit") || lower.contains("login") || lower.contains("sign")
                    {
                        suggestions.push(SuggestedAction {
                            action_type: ActionType::Click,
                            element_id: node.element_id,
                            selector: node.selector.clone(),
                            label: node.name.clone(),
                            reason: format!("Navigate via \"{}\"", name),
                            confidence: 0.7,
                        });
                    }
                }
            }
            _ => {}
        }
    });
}

fn collect_link_suggestions(
    tree: &SemanticTree,
    nav: Option<&NavigationGraph>,
    suggestions: &mut Vec<SuggestedAction>,
    page_type: &PageType,
) {
    let nav_graph = match nav {
        Some(g) => g,
        None => return,
    };

    let mut next_link: Option<(usize, String, String)> = None;
    let mut prev_link: Option<(usize, String, String)> = None;

    for link in &nav_graph.internal_links {
        let label = link.label.as_deref().unwrap_or("");
        let lower = label.to_lowercase();

        if lower.contains("next") || lower.contains("more") || lower.contains(">") {
            let eid = find_element_id_for_url(tree, &link.url);
            next_link = Some((eid, label.to_string(), link.url.clone()));
        }
        if lower.contains("prev")
            || lower.contains("previous")
            || lower.contains("<")
            || lower.contains("back")
        {
            let eid = find_element_id_for_url(tree, &link.url);
            prev_link = Some((eid, label.to_string(), link.url.clone()));
        }
    }

    if page_type == &PageType::ListingPage || page_type == &PageType::SearchPage {
        if let Some((eid, label, _url)) = next_link {
            suggestions.push(SuggestedAction {
                action_type: ActionType::Navigate,
                element_id: if eid > 0 { Some(eid) } else { None },
                selector: None,
                label: Some(label.clone()),
                reason: format!("Go to next page: \"{}\"", label),
                confidence: 0.75,
            });
        }
    }

    for route in &nav_graph.internal_links {
        if let Some(label) = &route.label {
            let lower = label.to_lowercase();
            if lower.contains("about") || lower.contains("contact") || lower.contains("help") {
                let eid = find_element_id_for_url(tree, &route.url);
                suggestions.push(SuggestedAction {
                    action_type: ActionType::Navigate,
                    element_id: if eid > 0 { Some(eid) } else { None },
                    selector: None,
                    label: Some(label.clone()),
                    reason: format!("Navigate to \"{}\"", label),
                    confidence: 0.4,
                });
            }
        }
    }

    let _ = prev_link;
}

fn collect_input_suggestions(tree: &SemanticTree, suggestions: &mut Vec<SuggestedAction>) {
    collect_interactive_nodes(&tree.root, &mut |node| {
        if node.element_id.is_none() || node.is_disabled {
            return;
        }
        if matches!(node.role, SemanticRole::TextBox) {
            if let Some(name) = &node.name {
                let lower = name.to_lowercase();
                if lower.contains("email") || lower.contains("user") || lower.contains("name") {
                    suggestions.push(SuggestedAction {
                        action_type: ActionType::Fill,
                        element_id: node.element_id,
                        selector: node.selector.clone(),
                        label: node.name.clone(),
                        reason: format!("Fill {} field (required for login/signup)", name),
                        confidence: 0.85,
                    });
                }
            }
        }
    });
}

fn collect_interactive_nodes(node: &SemanticNode, callback: &mut dyn FnMut(&SemanticNode)) {
    if node.is_interactive {
        callback(node);
    }
    for child in &node.children {
        collect_interactive_nodes(child, callback);
    }
}

fn find_element_id_for_url(tree: &SemanticTree, url: &str) -> usize {
    let mut found = 0;
    find_link_id(&tree.root, url, &mut found);
    found
}

fn find_link_id(node: &SemanticNode, target_url: &str, found: &mut usize) {
    if matches!(node.role, SemanticRole::Link) && node.is_interactive {
        if let Some(href) = &node.href {
            if href == target_url {
                *found = node.element_id.unwrap_or(0);
                return;
            }
        }
    }
    for child in &node.children {
        if *found > 0 {
            return;
        }
        find_link_id(child, target_url, found);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    #[test]
    fn test_plan_login_page() {
        let html = Html::parse_document(
            r#"
            <html><body>
                <form action="/login" method="post">
                    <input type="text" name="username" placeholder="Username">
                    <input type="password" name="password" placeholder="Password">
                    <button type="submit">Login</button>
                </form>
            </body></html>
        "#,
        );
        let tree = SemanticTree::build(&html, "https://example.com/login");
        let nav = NavigationGraph::build(&html, "https://example.com/login");
        let plan = ActionPlan::analyze("https://example.com/login", &tree, Some(&nav));

        assert_eq!(plan.page_type, PageType::LoginPage);
        assert!(plan.has_forms);
        assert!(!plan.suggestions.is_empty());

        let fill_actions: Vec<_> = plan
            .suggestions
            .iter()
            .filter(|s| s.action_type == ActionType::Fill)
            .collect();
        assert!(!fill_actions.is_empty());
    }

    #[test]
    fn test_plan_search_page() {
        let html = Html::parse_document(
            r#"
            <html><body>
                <form action="/search" method="get">
                    <input type="text" name="q" placeholder="Search...">
                    <button type="submit">Search</button>
                </form>
            </body></html>
        "#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let nav = NavigationGraph::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, Some(&nav));

        assert_eq!(plan.page_type, PageType::SearchPage);
    }

    #[test]
    fn test_plan_content_page() {
        let html = Html::parse_document(
            r#"
            <html><body>
                <h1>Article Title</h1>
                <p>Some content here.</p>
                <nav><a href="/about">About</a><a href="/contact">Contact</a></nav>
            </body></html>
        "#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let nav = NavigationGraph::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, Some(&nav));

        assert_eq!(plan.page_type, PageType::ContentPage);
        assert!(!plan.has_forms);
    }

    #[test]
    fn test_plan_pagination_detected() {
        let html = Html::parse_document(
            r#"
            <html><body>
                <h1>Items</h1>
                <div><a href="?page=1">Item 1</a></div>
                <div><a href="?page=2">Item 2</a></div>
                <a href="?page=2">Next</a>
            </body></html>
        "#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let nav = NavigationGraph::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, Some(&nav));

        assert!(plan.has_pagination);
    }

    #[test]
    fn test_suggestions_sorted_by_confidence() {
        let html = Html::parse_document(
            r#"
            <html><body>
                <form>
                    <input type="text" name="email">
                    <button type="submit">Submit</button>
                </form>
            </body></html>
        "#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, None);

        for window in plan.suggestions.windows(2) {
            assert!(window[0].confidence >= window[1].confidence);
        }
    }

    #[test]
    fn test_plan_form_page() {
        let html = Html::parse_document(
            r#"
            <html><body>
                <form>
                    <input type="text" name="name">
                    <input type="text" name="address">
                    <button type="submit">Submit</button>
                </form>
            </body></html>
        "#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, None);

        assert_eq!(plan.page_type, PageType::FormPage);
        assert!(plan.has_forms);
    }

    #[test]
    fn test_plan_navigation_page() {
        let mut links = String::from("<html><body><h1>Site</h1><nav>");
        for i in 1..=7 {
            links.push_str(&format!("<a href=\"/page-{}\">Page {}</a>", i, i));
        }
        links.push_str("</nav></body></html>");

        let html = Html::parse_document(&links);
        let tree = SemanticTree::build(&html, "https://example.com");
        let nav = NavigationGraph::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, Some(&nav));

        assert_eq!(plan.page_type, PageType::NavigationPage);
    }

    #[test]
    fn test_plan_unknown_page() {
        let html = Html::parse_document("<html><body><div>Empty div</div></body></html>");
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, None);

        assert_eq!(plan.page_type, PageType::Unknown);
        assert!(!plan.has_forms);
        assert!(plan.suggestions.is_empty());
    }

    #[test]
    fn test_plan_listing_page() {
        let html = Html::parse_document(
            r#"
            <html><body>
                <h1>Products</h1>
                <div><a href="/products?page=1">P1</a></div>
                <div><a href="/products?page=2">P2</a></div>
                <div><a href="/products?page=3">P3</a></div>
                <a href="/products?page=2">Next</a>
            </body></html>
        "#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let nav = NavigationGraph::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, Some(&nav));

        assert_eq!(plan.page_type, PageType::ListingPage);
        assert!(plan.has_pagination);
    }

    #[test]
    fn test_plan_no_pagination_when_absent() {
        let html = Html::parse_document(
            r#"<html><body>
                <h1>Blog</h1>
                <a href="/post1">Post 1</a>
                <a href="/post2">Post 2</a>
            </body></html>"#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let nav = NavigationGraph::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, Some(&nav));

        assert!(!plan.has_pagination);
    }

    #[test]
    fn test_plan_submit_button_high_confidence() {
        let html = Html::parse_document(
            r#"<html><body>
                <form>
                    <input type="text" name="q">
                    <button type="submit">Search</button>
                </form>
            </body></html>"#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, None);

        let submit: Vec<_> = plan
            .suggestions
            .iter()
            .filter(|s| s.action_type == ActionType::Submit)
            .collect();
        assert_eq!(submit.len(), 1);
        assert_eq!(submit[0].confidence, 0.8);
    }

    #[test]
    fn test_plan_checkbox_toggle_suggestion() {
        let html = Html::parse_document(
            r#"<html><body>
                <form>
                    <label><input type="checkbox" name="agree"> Agree</label>
                    <button type="submit">Go</button>
                </form>
            </body></html>"#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, None);

        let toggles: Vec<_> = plan
            .suggestions
            .iter()
            .filter(|s| s.action_type == ActionType::Toggle)
            .collect();
        assert_eq!(toggles.len(), 1);
        assert_eq!(toggles[0].confidence, 0.5);
    }

    #[test]
    fn test_plan_combobox_select_suggestion() {
        let html = Html::parse_document(
            r#"<html><body>
                <form>
                    <select name="country">
                        <option value="us">US</option>
                        <option value="uk">UK</option>
                    </select>
                    <button type="submit">Go</button>
                </form>
            </body></html>"#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, None);

        let selects: Vec<_> = plan
            .suggestions
            .iter()
            .filter(|s| s.action_type == ActionType::Select)
            .collect();
        assert_eq!(selects.len(), 1);
        assert_eq!(selects[0].confidence, 0.7);
    }

    #[test]
    fn test_plan_interactive_count() {
        let html = Html::parse_document(
            r#"<html><body>
                <a href="/a">A</a>
                <a href="/b">B</a>
                <button>Btn</button>
                <input type="text" name="x">
            </body></html>"#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, None);

        assert_eq!(plan.interactive_count, 4);
    }

    #[test]
    fn test_plan_url_stored() {
        let html = Html::parse_document("<html><body><h1>Test</h1></body></html>");
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com/test", &tree, None);

        assert_eq!(plan.url, "https://example.com/test");
    }

    #[test]
    fn test_plan_scroll_suggestion_with_pagination() {
        let html = Html::parse_document(
            r#"<html><body>
                <div><a href="?page=1">Page 1</a></div>
                <div><a href="?page=2">Page 2</a></div>
                <a href="?page=2">Next</a>
            </body></html>"#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let nav = NavigationGraph::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, Some(&nav));

        let scrolls: Vec<_> = plan
            .suggestions
            .iter()
            .filter(|s| s.action_type == ActionType::ScrollDown)
            .collect();
        assert_eq!(scrolls.len(), 1);
        assert_eq!(scrolls[0].confidence, 0.6);
    }

    #[test]
    fn test_plan_serialization() {
        let html = Html::parse_document(
            r#"<html><body>
                <form>
                    <input type="password" name="pw">
                    <button type="submit">Login</button>
                </form>
            </body></html>"#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, None);

        let json = serde_json::to_string(&plan).unwrap();
        let deserialized: ActionPlan = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.page_type, plan.page_type);
        assert_eq!(deserialized.suggestions.len(), plan.suggestions.len());
    }

    #[test]
    fn test_plan_email_field_boosted() {
        let html = Html::parse_document(
            r#"<html><body>
                <form>
                    <input type="text" name="user_email">
                    <input type="password" name="pass">
                    <button type="submit">Go</button>
                </form>
            </body></html>"#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, None);

        let email_fills: Vec<_> = plan
            .suggestions
            .iter()
            .filter(|s| {
                s.action_type == ActionType::Fill
                    && s.label.as_deref() == Some("user_email")
                    && s.confidence == 0.85
            })
            .collect();
        assert_eq!(email_fills.len(), 1);
    }

    #[test]
    fn test_plan_no_nav_graph() {
        let html = Html::parse_document(
            r#"<html><body>
                <form>
                    <input type="text" name="q">
                    <button type="submit">Search</button>
                </form>
            </body></html>"#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, None);

        assert!(!plan.has_pagination);
        assert_eq!(plan.page_type, PageType::SearchPage);
    }

    #[test]
    fn test_plan_pagination_by_label_more() {
        let html = Html::parse_document(
            r#"<html><body>
                <a href="/items?page=1">Item 1</a>
                <a href="/items?page=2">Load More</a>
            </body></html>"#,
        );
        let tree = SemanticTree::build(&html, "https://example.com");
        let nav = NavigationGraph::build(&html, "https://example.com");
        let plan = ActionPlan::analyze("https://example.com", &tree, Some(&nav));

        assert!(plan.has_pagination);
    }
}
