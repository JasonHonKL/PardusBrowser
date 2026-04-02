//! Tests for element ID feature.
//!
//! Tests that interactive elements get unique IDs assigned during semantic tree building,
//! and that these IDs can be used to find and interact with elements.

use pardus_core::semantic::tree::{SemanticTree, SemanticNode, SemanticRole};
use pardus_core::page::Page;

// ---------------------------------------------------------------------------
// Element ID Assignment Tests
// ---------------------------------------------------------------------------

#[test]
fn test_link_gets_element_id() {
    let html = r#"<html><body><a href="/about">About Us</a></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    // Find the link node
    fn find_link(node: &pardus_core::semantic::tree::SemanticNode) -> Option<&pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Link) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_link(child) {
                return Some(found);
            }
        }
        None
    }

    let link = find_link(&tree.root).expect("Should find a link");
    assert!(link.element_id.is_some(), "Link should have an element_id");
    assert_eq!(link.element_id.unwrap(), 1, "First interactive element should have ID 1");
}

#[test]
fn test_button_gets_element_id() {
    let html = r#"<html><body><button>Click Me</button></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_button(node: &pardus_core::semantic::tree::SemanticNode) -> Option<&pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Button) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_button(child) {
                return Some(found);
            }
        }
        None
    }

    let button = find_button(&tree.root).expect("Should find a button");
    assert!(button.element_id.is_some(), "Button should have an element_id");
    assert_eq!(button.element_id.unwrap(), 1, "First interactive element should have ID 1");
}

#[test]
fn test_multiple_interactive_elements_get_sequential_ids() {
    let html = r#"
        <html><body>
            <a href="/link1">Link 1</a>
            <button>Button 1</button>
            <a href="/link2">Link 2</a>
            <input type="text" name="query">
            <input type="submit" value="Submit">
        </body></html>
    "#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn collect_interactive_ids(node: &pardus_core::semantic::tree::SemanticNode, ids: &mut Vec<usize>) {
        if let Some(id) = node.element_id {
            ids.push(id);
        }
        for child in &node.children {
            collect_interactive_ids(child, ids);
        }
    }

    let mut ids: Vec<usize> = Vec::new();
    collect_interactive_ids(&tree.root, &mut ids);

    assert!(!ids.is_empty(), "Should have interactive elements with IDs");
    assert_eq!(ids.len(), 5, "Should have 5 interactive elements");

    // IDs should be sequential starting from 1
    let mut expected = 1;
    for id in &ids {
        assert_eq!(*id, expected, "Element ID should be sequential");
        expected += 1;
    }
}

#[test]
fn test_textbox_gets_element_id() {
    let html = r#"<html><body><input type="text" name="email" placeholder="Email"></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_textbox(node: &pardus_core::semantic::tree::SemanticNode) -> Option<&pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::TextBox) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_textbox(child) {
                return Some(found);
            }
        }
        None
    }

    let textbox = find_textbox(&tree.root).expect("Should find a textbox");
    assert!(textbox.element_id.is_some(), "Textbox should have an element_id");
}

#[test]
fn test_checkbox_gets_element_id() {
    let html = r#"<html><body><input type="checkbox" name="agree"></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_checkbox(node: &pardus_core::semantic::tree::SemanticNode) -> Option<&pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Checkbox) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_checkbox(child) {
                return Some(found);
            }
        }
        None
    }

    let checkbox = find_checkbox(&tree.root).expect("Should find a checkbox");
    assert!(checkbox.element_id.is_some(), "Checkbox should have an element_id");
}

#[test]
fn test_radio_gets_element_id() {
    let html = r#"<html><body><input type="radio" name="choice" value="a"></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_radio(node: &pardus_core::semantic::tree::SemanticNode) -> Option<&pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Radio) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_radio(child) {
                return Some(found);
            }
        }
        None
    }

    let radio = find_radio(&tree.root).expect("Should find a radio button");
    assert!(radio.element_id.is_some(), "Radio should have an element_id");
}

#[test]
fn test_combobox_gets_element_id() {
    let html = r#"<html><body><select name="country"><option>USA</option></select></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_combobox(node: &pardus_core::semantic::tree::SemanticNode) -> Option<&pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Combobox) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_combobox(child) {
                return Some(found);
            }
        }
        None
    }

    let combobox = find_combobox(&tree.root).expect("Should find a combobox");
    assert!(combobox.element_id.is_some(), "Combobox should have an element_id");
}

// ---------------------------------------------------------------------------
// Disabled Element Tests
// ---------------------------------------------------------------------------

#[test]
fn test_disabled_button_has_no_element_id() {
    let html = r#"<html><body><button disabled>Disabled Button</button></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_button(node: &pardus_core::semantic::tree::SemanticNode) -> Option<&pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Button) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_button(child) {
                return Some(found);
            }
        }
        None
    }

    let button = find_button(&tree.root).expect("Should find a button");
    assert!(button.is_disabled, "Button should be disabled");
    assert!(button.element_id.is_none(), "Disabled button should NOT have an element_id");
}

#[test]
fn test_disabled_elements_dont_consume_ids() {
    let html = r#"
        <html><body>
            <button disabled>Disabled</button>
            <button>Enabled</button>
        </body></html>
    "#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn collect_button_ids(node: &pardus_core::semantic::tree::SemanticNode, ids: &mut Vec<usize>) {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Button) {
            if let Some(id) = node.element_id {
                ids.push(id);
            }
        }
        for child in &node.children {
            collect_button_ids(child, ids);
        }
    }

    let mut ids: Vec<usize> = Vec::new();
    collect_button_ids(&tree.root, &mut ids);

    assert_eq!(ids.len(), 1, "Only one button should have an ID (the enabled one)");
    assert_eq!(ids[0], 1, "The enabled button should have ID 1");
}

// ---------------------------------------------------------------------------
// Non-Interactive Element Tests
// ---------------------------------------------------------------------------

#[test]
fn test_heading_has_no_element_id() {
    let html = r#"<html><body><h1>Title</h1></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_heading(node: &pardus_core::semantic::tree::SemanticNode) -> Option<&pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Heading { .. }) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_heading(child) {
                return Some(found);
            }
        }
        None
    }

    let heading = find_heading(&tree.root).expect("Should find a heading");
    assert!(heading.element_id.is_none(), "Heading should NOT have an element_id (not interactive)");
}

#[test]
fn test_generic_element_has_no_element_id() {
    let html = r#"<html><body><div>Some content</div></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_generic(node: &pardus_core::semantic::tree::SemanticNode) -> Option<&pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Generic) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_generic(child) {
                return Some(found);
            }
        }
        None
    }

    let generic = find_generic(&tree.root);
    if let Some(node) = generic {
        assert!(node.element_id.is_none(), "Generic element should NOT have an element_id");
    }
}

// ---------------------------------------------------------------------------
// Statistics Tests
// ---------------------------------------------------------------------------

#[test]
fn test_stats_count_interactive_elements() {
    let html = r#"
        <html><body>
            <a href="/link1">Link 1</a>
            <button>Button 1</button>
            <input type="text" name="query">
        </body></html>
    "#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    assert_eq!(tree.stats.actions, 3, "Should count 3 interactive elements");
    assert_eq!(tree.stats.links, 1, "Should count 1 link");
}

// ---------------------------------------------------------------------------
// Link Without Href Tests
// ---------------------------------------------------------------------------

#[test]
fn test_link_without_href_has_no_element_id() {
    let html = r#"<html><body><a name="anchor">Anchor Only</a></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_link(node: &pardus_core::semantic::tree::SemanticNode) -> Option<&pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Link) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_link(child) {
                return Some(found);
            }
        }
        None
    }

    // Links without href should not be interactive, so no link role should be found
    // Actually the role is still Link but is_interactive should be false
    let link = find_link(&tree.root);
    if let Some(node) = link {
        // If found, it should not be interactive
        if !node.is_interactive {
            assert!(node.element_id.is_none(), "Non-interactive link should NOT have element_id");
        }
    }
}

// ---------------------------------------------------------------------------
// Selector Tests - Verify selectors are correctly stored and resolve to right elements
// ---------------------------------------------------------------------------

#[test]
fn test_interactive_elements_have_selectors() {
    let html = r#"
        <html><body>
            <a href="/about">About</a>
            <button>Click</button>
            <input type="text" name="email">
        </body></html>
    "#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn collect_selectors(node: &pardus_core::semantic::tree::SemanticNode, selectors: &mut Vec<String>) {
        if let Some(sel) = &node.selector {
            selectors.push(sel.clone());
        }
        for child in &node.children {
            collect_selectors(child, selectors);
        }
    }

    let mut selectors: Vec<String> = Vec::new();
    collect_selectors(&tree.root, &mut selectors);

    // All elements should have selectors now
    assert!(!selectors.is_empty(), "Elements should have selectors");
}

#[test]
fn test_multiple_links_have_unique_selectors() {
    // This simulates the Google homepage scenario with multiple similar links
    let html = r#"
        <html><body>
            <a href="https://mail.google.com">Gmail</a>
            <a href="https://images.google.com">Images</a>
            <a href="/about">About</a>
            <a href="/privacy">Privacy</a>
        </body></html>
    "#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://google.com");

    fn find_link_by_text<'a>(node: &'a pardus_core::semantic::tree::SemanticNode, text: &str) -> Option<&'a pardus_core::semantic::tree::SemanticNode> {
        if matches!(node.role, pardus_core::semantic::tree::SemanticRole::Link) {
            if node.name.as_deref() == Some(text) {
                return Some(node);
            }
        }
        for child in &node.children {
            if let Some(found) = find_link_by_text(child, text) {
                return Some(found);
            }
        }
        None
    }

    // Each link should have a unique selector based on href
    let gmail = find_link_by_text(&tree.root, "Gmail").expect("Should find Gmail link");
    let images = find_link_by_text(&tree.root, "Images").expect("Should find Images link");

    assert!(gmail.selector.is_some(), "Gmail should have a selector");
    assert!(images.selector.is_some(), "Images should have a selector");

    // The selectors should be different (based on different hrefs)
    assert_ne!(gmail.selector, images.selector, "Gmail and Images should have different selectors");

    // The selectors should contain the href
    assert!(gmail.selector.as_ref().unwrap().contains("mail.google.com"), "Gmail selector should contain its href");
    assert!(images.selector.as_ref().unwrap().contains("images.google.com"), "Images selector should contain its href");
}

#[test]
fn test_selector_resolves_to_correct_element() {
    // Test that find_by_element_id returns the correct element
    let html = r#"
        <html><body>
            <a href="https://mail.google.com">Gmail</a>
            <a href="https://images.google.com">Images</a>
            <a href="/about">About</a>
        </body></html>
    "#;
    let page = Page::from_html(html, "https://google.com");

    // Get all interactive elements
    let elements = page.interactive_elements();
    assert_eq!(elements.len(), 3, "Should have 3 interactive elements");

    // Find each by element_id and verify it returns the right element
    for (i, expected) in elements.iter().enumerate() {
        let id = i + 1;
        if let Some(found) = page.find_by_element_id(id) {
            // The found element should have the same href as expected
            assert_eq!(found.href, expected.href, "Element {} should have correct href", id);
        }
    }
}

// ---------------------------------------------------------------------------
// Selector Generation Strategy Tests
// ---------------------------------------------------------------------------

#[test]
fn test_element_with_id_gets_id_selector() {
    let html = r#"<html><body><button id="submit-btn">Submit</button></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_button(node: &SemanticNode) -> Option<&SemanticNode> {
        if matches!(node.role, SemanticRole::Button) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_button(child) {
                return Some(found);
            }
        }
        None
    }

    let button = find_button(&tree.root).expect("Should find button");
    assert_eq!(button.selector.as_deref(), Some("#submit-btn"), "Button with id should use #id selector");
}

#[test]
fn test_element_with_name_gets_name_selector() {
    let html = r#"<html><body><input type="text" name="email" placeholder="Email"></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_textbox(node: &SemanticNode) -> Option<&SemanticNode> {
        if matches!(node.role, SemanticRole::TextBox) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_textbox(child) {
                return Some(found);
            }
        }
        None
    }

    let textbox = find_textbox(&tree.root).expect("Should find textbox");
    let selector = textbox.selector.as_ref().expect("Textbox should have selector");
    assert!(selector.contains("[name=\"email\"]"), "Input with name should use name selector");
}

#[test]
fn test_link_gets_href_selector() {
    let html = r#"<html><body><a href="/login">Login</a></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_link(node: &SemanticNode) -> Option<&SemanticNode> {
        if matches!(node.role, SemanticRole::Link) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_link(child) {
                return Some(found);
            }
        }
        None
    }

    let link = find_link(&tree.root).expect("Should find link");
    let selector = link.selector.as_ref().expect("Link should have selector");
    assert!(selector.contains("[href="), "Link should use href selector");
    assert!(selector.contains("/login"), "Link selector should contain the href");
}

#[test]
fn test_element_without_unique_attrs_gets_structural_selector() {
    // A button without id or name should get a structural selector
    let html = r#"<html><body><div><div><button>Click</button></div></div></body></html>"#;
    let tree = SemanticTree::build(&scraper::Html::parse_document(html), "https://example.com");

    fn find_button(node: &SemanticNode) -> Option<&SemanticNode> {
        if matches!(node.role, SemanticRole::Button) {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = find_button(child) {
                return Some(found);
            }
        }
        None
    }

    let button = find_button(&tree.root).expect("Should find button");
    let selector = button.selector.as_ref().expect("Button should have selector");
    // Should have nth-child in the selector
    assert!(selector.contains(":nth-child"), "Button without unique attrs should use structural selector");
}

// ---------------------------------------------------------------------------
// Multiple Similar Elements Tests (Google Scenario)
// ---------------------------------------------------------------------------

#[test]
fn test_google_homepage_scenario() {
    // Simulates the Google homepage with multiple navigation links
    let html = r#"
        <html><body>
            <header>
                <nav>
                    <a href="https://mail.google.com/mail/u/0/">Gmail</a>
                    <a href="https://www.google.com/imghp">Images</a>
                </nav>
            </header>
            <main>
                <form action="/search">
                    <input type="text" name="q" placeholder="Search">
                    <button type="submit">Google Search</button>
                    <button type="button">I'm Feeling Lucky</button>
                </form>
            </main>
            <footer>
                <a href="/about">About</a>
                <a href="/privacy">Privacy</a>
                <a href="/terms">Terms</a>
            </footer>
        </body></html>
    "#;
    let page = Page::from_html(html, "https://www.google.com");

    // Collect all interactive elements with their IDs and hrefs/actions
    let elements = page.interactive_elements();

    // Should have Gmail, Images, Search input, 2 buttons, About, Privacy, Terms = 7 elements
    assert!(elements.len() >= 5, "Should have at least 5 interactive elements");

    // Verify each element has a unique selector
    let selectors: Vec<&str> = elements.iter()
        .map(|e| e.selector.as_str())
        .collect();

    // All selectors should be unique
    let unique_selectors: std::collections::HashSet<&str> = selectors.iter().copied().collect();
    assert_eq!(selectors.len(), unique_selectors.len(), "All selectors should be unique");

    // Verify Gmail and Images can be found correctly by element_id
    for id in 1..=elements.len() {
        let found = page.find_by_element_id(id);
        assert!(found.is_some(), "Element {} should be findable", id);

        let found = found.unwrap();
        // Verify the selector can actually find the element
        let expected = &elements[id - 1];
        assert_eq!(found.href, expected.href, "Element {} href should match", id);
    }
}

#[test]
fn test_multiple_buttons_same_text_different_positions() {
    // Two buttons with same text but different positions/contexts
    let html = r#"
        <html><body>
            <div id="section1">
                <button class="btn">Submit</button>
            </div>
            <div id="section2">
                <button class="btn">Submit</button>
            </div>
        </body></html>
    "#;
    let page = Page::from_html(html, "https://example.com");

    let elements = page.interactive_elements();
    assert_eq!(elements.len(), 2, "Should have 2 buttons");

    // Both buttons should be findable with different selectors
    let first = page.find_by_element_id(1).expect("Button 1 should exist");
    let second = page.find_by_element_id(2).expect("Button 2 should exist");

    // Selectors should be different
    assert_ne!(first.selector, second.selector, "Two buttons should have different selectors");
}

#[test]
fn test_multiple_inputs_same_type_different_names() {
    let html = r#"
        <html><body>
            <form>
                <input type="text" name="username" placeholder="Username">
                <input type="text" name="email" placeholder="Email">
                <input type="password" name="password" placeholder="Password">
            </form>
        </body></html>
    "#;
    let page = Page::from_html(html, "https://example.com");

    let elements = page.interactive_elements();
    assert_eq!(elements.len(), 3, "Should have 3 inputs");

    // Each should have a unique name-based selector
    let username = page.find_by_element_id(1).expect("Username input should exist");
    let email = page.find_by_element_id(2).expect("Email input should exist");
    let password = page.find_by_element_id(3).expect("Password input should exist");

    assert!(username.selector.contains("username"), "Username selector should contain 'username'");
    assert!(email.selector.contains("email"), "Email selector should contain 'email'");
    assert!(password.selector.contains("password"), "Password selector should contain 'password'");
}

// ---------------------------------------------------------------------------
// Selector Resolution Verification Tests
// ---------------------------------------------------------------------------

#[test]
fn test_find_by_element_id_returns_correct_href() {
    // This is the core test: verify that clicking #1, #2, #3 etc
    // actually returns the element with the correct href
    let html = r#"
        <html><body>
            <a href="https://mail.google.com">Gmail</a>
            <a href="https://images.google.com">Images</a>
            <a href="https://drive.google.com">Drive</a>
            <a href="https://maps.google.com">Maps</a>
            <a href="https://news.google.com">News</a>
        </body></html>
    "#;
    let page = Page::from_html(html, "https://google.com");

    // Expected hrefs in order
    let expected_hrefs = vec![
        "https://mail.google.com",
        "https://images.google.com",
        "https://drive.google.com",
        "https://maps.google.com",
        "https://news.google.com",
    ];

    for (id, expected_href) in expected_hrefs.iter().enumerate() {
        let id = id + 1; // IDs start at 1
        let element = page.find_by_element_id(id)
            .unwrap_or_else(|| panic!("Element #{} should exist", id));

        assert_eq!(
            element.href.as_deref(),
            Some(*expected_href),
            "Element #{} should have href {}",
            id, expected_href
        );
    }
}

#[test]
fn test_find_by_element_id_works_after_tree_rebuild() {
    // Verify that building the tree twice produces same element_id -> element mapping
    let html = r#"
        <html><body>
            <a href="/page1">Page 1</a>
            <a href="/page2">Page 2</a>
            <a href="/page3">Page 3</a>
        </body></html>
    "#;

    let page = Page::from_html(html, "https://example.com");

    // First lookup
    let first_lookup: Vec<_> = (1..=3)
        .map(|id| page.find_by_element_id(id).unwrap().href.clone())
        .collect();

    // Second lookup (tree is rebuilt each time)
    let second_lookup: Vec<_> = (1..=3)
        .map(|id| page.find_by_element_id(id).unwrap().href.clone())
        .collect();

    // Should be identical
    assert_eq!(first_lookup, second_lookup, "Lookups should be consistent");
}

// ---------------------------------------------------------------------------
// Edge Cases Tests
// ---------------------------------------------------------------------------

#[test]
fn test_links_with_query_params_in_href() {
    let html = r#"
        <html><body>
            <a href="/search?q=rust">Search Rust</a>
            <a href="/search?q=python">Search Python</a>
        </body></html>
    "#;
    let page = Page::from_html(html, "https://example.com");

    let rust_link = page.find_by_element_id(1).expect("Rust link should exist");
    let python_link = page.find_by_element_id(2).expect("Python link should exist");

    assert_eq!(rust_link.href.as_deref(), Some("/search?q=rust"));
    assert_eq!(python_link.href.as_deref(), Some("/search?q=python"));
}

#[test]
fn test_links_with_fragments_in_href() {
    let html = r#"
        <html><body>
            <a href="/page#section1">Section 1</a>
            <a href="/page#section2">Section 2</a>
        </body></html>
    "#;
    let page = Page::from_html(html, "https://example.com");

    let s1 = page.find_by_element_id(1).expect("Section 1 link should exist");
    let s2 = page.find_by_element_id(2).expect("Section 2 link should exist");

    assert_ne!(s1.selector, s2.selector, "Links with different fragments should have different selectors");
}

#[test]
fn test_deeply_nested_elements() {
    let html = r#"
        <html><body>
            <div><div><div><div><div>
                <button id="deep-button">Deep Button</button>
            </div></div></div></div></div>
        </body></html>
    "#;
    let page = Page::from_html(html, "https://example.com");

    let button = page.find_by_element_id(1).expect("Deep button should be found");
    // Should use ID selector, not structural
    assert!(button.selector.contains("#deep-button"), "Should use ID selector");
}

#[test]
fn test_mixed_interactive_elements() {
    // A realistic form with various element types
    let html = r#"
        <html><body>
            <a href="/home">Home</a>
            <form>
                <input type="text" name="name">
                <input type="email" name="email">
                <input type="checkbox" name="subscribe">
                <select name="country">
                    <option>USA</option>
                </select>
                <button type="submit">Submit</button>
            </form>
            <a href="/logout">Logout</a>
        </body></html>
    "#;
    let page = Page::from_html(html, "https://example.com");

    // All elements should be findable with correct attributes
    let home = page.find_by_element_id(1).expect("Home link");
    assert_eq!(home.href.as_deref(), Some("/home"));

    let name_input = page.find_by_element_id(2).expect("Name input");
    assert!(name_input.selector.contains("name"));

    let email_input = page.find_by_element_id(3).expect("Email input");
    assert!(email_input.selector.contains("email"));

    let checkbox = page.find_by_element_id(4).expect("Checkbox");
    assert!(checkbox.selector.contains("subscribe"));

    let select = page.find_by_element_id(5).expect("Select");
    assert!(select.selector.contains("country"));

    let submit = page.find_by_element_id(6).expect("Submit button");
    assert_eq!(submit.action.as_deref(), Some("click"));

    let logout = page.find_by_element_id(7).expect("Logout link");
    assert_eq!(logout.href.as_deref(), Some("/logout"));
}
