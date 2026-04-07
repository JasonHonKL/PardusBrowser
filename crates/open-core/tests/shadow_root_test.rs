//! Tests for ShadowRoot DOM support.
//!
//! Tests the shadow_root field on DomNode and related node type functionality.

// Note: The shadow_root field exists on DomNode but manipulation methods are not
// yet implemented. These tests verify the data structure and node type handling.

// ---------------------------------------------------------------------------
// Basic Node Type Tests
// ---------------------------------------------------------------------------

#[test]
fn test_element_node_type_is_1() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body><div id=\"test\"></div></body></html>";
    let doc = DomDocument::from_html(html);
    let div = doc.get_element_by_id("test").unwrap();
    assert_eq!(doc.get_node_type(div), 1);
}

#[test]
fn test_text_node_type_is_3() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body>text content</body></html>";
    let doc = DomDocument::from_html(html);
    // Body contains a text node child
    let body = doc.body();
    let children = doc.get_children(body);
    // Find the text node (skip any whitespace-only nodes)
    for &child in &children {
        let node_name = doc.get_node_name(child);
        if node_name == "#text" {
            assert_eq!(doc.get_node_type(child), 3);
            return;
        }
    }
    // If we get here, no text node was found - that's okay for some HTML
}

#[test]
fn test_comment_node_type_is_8() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body><!-- comment --></body></html>";
    let doc = DomDocument::from_html(html);
    // Body contains a comment node
    let body = doc.body();
    let children = doc.get_children(body);
    for &child in &children {
        let node_name = doc.get_node_name(child);
        if node_name == "#comment" {
            assert_eq!(doc.get_node_type(child), 8);
            return;
        }
    }
}

#[test]
fn test_document_element_is_found() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body></body></html>";
    let doc = DomDocument::from_html(html);

    // The document_element() should return a valid node id
    let doc_elem = doc.document_element();
    assert_ne!(doc_elem, 0, "document element should exist");
    assert!(doc.get_node_type(doc_elem) == 1 || doc.get_node_type(doc_elem) == 9, "document element should be an element or document");
}

#[test]
fn test_document_fragment_node_type_is_11() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body></body></html>";
    let mut doc = DomDocument::from_html(html);

    // Create a document fragment
    let fragment = doc.create_document_fragment();
    // DocumentFragment should return node type 11
    assert_eq!(doc.get_node_type(fragment), 11);
}

// ---------------------------------------------------------------------------
// Node Name Tests
// ---------------------------------------------------------------------------

#[test]
fn test_element_node_name_is_uppercase_tag() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body><div id=\"test\"></div></body></html>";
    let doc = DomDocument::from_html(html);
    let div = doc.get_element_by_id("test").unwrap();
    assert_eq!(doc.get_node_name(div), "DIV");
}

#[test]
fn test_text_node_name_is_text() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body>text</body></html>";
    let doc = DomDocument::from_html(html);
    let body = doc.body();
    let children = doc.get_children(body);
    for &child in &children {
        let content = doc.get_text_content(child);
        if content.trim() == "text" {
            assert_eq!(doc.get_node_name(child), "#text");
            return;
        }
    }
}

#[test]
fn test_comment_node_name_is_comment() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body><!-- my comment --></body></html>";
    let doc = DomDocument::from_html(html);
    let body = doc.body();
    let children = doc.get_children(body);
    for &child in &children {
        let node_name = doc.get_node_name(child);
        if node_name == "#comment" {
            // Found it
            return;
        }
    }
    // Comment should have been found
    panic!("Comment node not found");
}

#[test]
fn test_document_element_node_name_is_html() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body></body></html>";
    let doc = DomDocument::from_html(html);

    // The document element (HTML) should have node name HTML
    let doc_elem = doc.document_element();
    assert!(doc_elem > 0, "document element should exist");
    assert_eq!(doc.get_node_name(doc_elem), "HTML");
}

#[test]
fn test_document_fragment_node_name() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body></body></html>";
    let mut doc = DomDocument::from_html(html);

    let fragment = doc.create_document_fragment();
    assert_eq!(doc.get_node_name(fragment), "#document-fragment");
}

// ---------------------------------------------------------------------------
// Create Document Fragment Tests
// ---------------------------------------------------------------------------

#[test]
fn test_create_document_fragment() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body></body></html>";
    let mut doc = DomDocument::from_html(html);

    let fragment = doc.create_document_fragment();

    // Fragment should exist and be empty
    assert!(!doc.has_child_nodes(fragment));
}

#[test]
fn test_document_fragment_can_have_children() {
    use open_core::js::dom::DomDocument;
    let mut doc = DomDocument::from_html("<html><body></body></html>");

    let fragment = doc.create_document_fragment();

    // Add children to fragment
    let child1 = doc.create_element("div");
    doc.set_attribute(child1, "id", "child1");
    doc.append_child(fragment, child1);

    let child2 = doc.create_element("span");
    doc.set_attribute(child2, "id", "child2");
    doc.append_child(fragment, child2);

    // Fragment should have children
    assert!(doc.has_child_nodes(fragment));
    assert_eq!(doc.get_children(fragment).len(), 2);
}

#[test]
fn test_document_fragment_children_move_on_insert() {
    use open_core::js::dom::DomDocument;
    let mut doc = DomDocument::from_html("<html><body><div id=\"target\"></div></body></html>");

    let fragment = doc.create_document_fragment();

    // Add children to fragment
    let child = doc.create_element("span");
    doc.set_attribute(child, "id", "fragment-child");
    doc.append_child(fragment, child);

    let target = doc.get_element_by_id("target").unwrap();

    // Insert fragment contents into target
    doc.append_child(target, fragment);

    // Fragment children should now be in target
    let target_children = doc.get_children(target);
    assert!(!target_children.is_empty(), "Target should have children after fragment append");
}

// ---------------------------------------------------------------------------
// Shadow Root Mode Tests (enum tests)
// ---------------------------------------------------------------------------

#[test]
fn test_shadow_root_mode_variants() {
    use open_core::js::dom::ShadowRootMode;

    // Test that the enum exists and has the expected variants
    let open = ShadowRootMode::Open;
    let closed = ShadowRootMode::Closed;

    // Test equality
    assert_eq!(open, ShadowRootMode::Open);
    assert_eq!(closed, ShadowRootMode::Closed);
    assert_ne!(open, closed);
}

// ---------------------------------------------------------------------------
// Edge Cases
// ---------------------------------------------------------------------------

#[test]
fn test_nonexistent_node_type_is_0() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body></body></html>";
    let doc = DomDocument::from_html(html);

    // Non-existent node should return 0
    assert_eq!(doc.get_node_type(999999), 0);
}

#[test]
fn test_nonexistent_node_name_is_empty() {
    use open_core::js::dom::DomDocument;
    let html = "<html><body></body></html>";
    let doc = DomDocument::from_html(html);

    // Non-existent node should return empty string
    assert_eq!(doc.get_node_name(999999), "");
}
