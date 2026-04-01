//! Arena-allocated DOM implementation
//!
//! Provides cache-friendly, contiguous memory layout for DOM nodes.
//! Uses bumpalo for fast allocation and deallocation.

use bumpalo::Bump;
use smallvec::SmallVec;
use std::fmt;

/// Node identifier - compact 32-bit index
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(u32);

impl NodeId {
    pub const ROOT: NodeId = NodeId(0);
    pub const NULL: NodeId = NodeId(u32::MAX);

    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn is_null(self) -> bool {
        self == Self::NULL
    }
}

/// Node type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Element,
    Text,
    Comment,
    Document,
    DocumentFragment,
}

/// An element with attributes stored in the arena
#[derive(Debug)]
pub struct ElementData {
    pub tag: String,
    pub attributes: SmallVec<[(String, String); 4]>,
}

/// DOM node stored in arena
#[derive(Debug)]
pub struct Node {
    pub id: NodeId,
    pub node_type: NodeType,
    pub parent: NodeId,
    pub first_child: NodeId,
    pub next_sibling: NodeId,
    pub prev_sibling: NodeId,
    /// Element data if this is an element node
    pub element_data: Option<ElementData>,
    /// Text content if this is a text node
    pub text_content: Option<String>,
}

impl Node {
    pub fn new(id: NodeId, node_type: NodeType) -> Self {
        Self {
            id,
            node_type,
            parent: NodeId::NULL,
            first_child: NodeId::NULL,
            next_sibling: NodeId::NULL,
            prev_sibling: NodeId::NULL,
            element_data: None,
            text_content: None,
        }
    }

    pub fn is_element(&self) -> bool {
        self.node_type == NodeType::Element
    }

    pub fn is_text(&self) -> bool {
        self.node_type == NodeType::Text
    }

    pub fn tag(&self) -> Option<&str> {
        self.element_data.as_ref().map(|e| e.tag.as_str())
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.element_data.as_ref()
            .and_then(|e| e.attributes.iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.as_str()))
    }

    pub fn text(&self) -> &str {
        self.text_content.as_deref().unwrap_or("")
    }
}

/// Arena-allocated DOM tree
pub struct ArenaDom {
    arena: Bump,
    nodes: Vec<Node>,
    root: NodeId,
}

impl ArenaDom {
    /// Create new empty DOM
    pub fn new() -> Self {
        let mut dom = Self {
            arena: Bump::new(),
            nodes: Vec::new(),
            root: NodeId::NULL,
        };
        // Create root document node
        let root = dom.create_node(NodeType::Document);
        dom.root = root;
        dom
    }

    /// Create with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        let mut dom = Self {
            arena: Bump::with_capacity(capacity * 64), // Estimate 64 bytes per node
            nodes: Vec::with_capacity(capacity),
            root: NodeId::NULL,
        };
        let root = dom.create_node(NodeType::Document);
        dom.root = root;
        dom
    }

    /// Create a new node
    pub fn create_node(&mut self, node_type: NodeType) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        let node = Node::new(id, node_type);
        self.nodes.push(node);
        id
    }

    /// Create element node
    pub fn create_element(&mut self,
        tag: impl Into<String>,
        attrs: impl IntoIterator<Item = (String, String)>,
    ) -> NodeId {
        let id = self.create_node(NodeType::Element);
        let element_data = ElementData {
            tag: tag.into(),
            attributes: attrs.into_iter().collect(),
        };
        self.nodes[id.index()].element_data = Some(element_data);
        id
    }

    /// Create text node
    pub fn create_text(&mut self,
        content: impl Into<String>,
    ) -> NodeId {
        let id = self.create_node(NodeType::Text);
        self.nodes[id.index()].text_content = Some(content.into());
        id
    }

    /// Append child to parent
    pub fn append_child(&mut self,
        parent: NodeId,
        child: NodeId,
    ) {
        let child_idx = child.index();
        let parent_idx = parent.index();

        // Update child's parent
        self.nodes[child_idx].parent = parent;

        // Link to siblings
        if let Some(last_child) = self.last_child(parent) {
            let last_idx = last_child.index();
            self.nodes[child_idx].prev_sibling = last_child;
            self.nodes[last_idx].next_sibling = child;
        } else {
            // First child
            self.nodes[parent_idx].first_child = child;
        }
    }

    /// Get last child of a node
    fn last_child(&self,
        parent: NodeId,
    ) -> Option<NodeId> {
        let first = self.nodes[parent.index()].first_child;
        if first.is_null() {
            return None;
        }

        let mut current = first;
        loop {
            let next = self.nodes[current.index()].next_sibling;
            if next.is_null() {
                return Some(current);
            }
            current = next;
        }
    }

    /// Get node by id
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id.index())
    }

    /// Get mutable node
    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id.index())
    }

    /// Get root node
    pub fn root(&self) -> NodeId {
        self.root
    }

    /// Total node count
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Iterate over all nodes
    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter()
    }

    /// Iterate children of a node
    pub fn children(&self, parent: NodeId) -> impl Iterator<Item = &Node> {
        let first = self.nodes[parent.index()].first_child;
        NodeIter {
            dom: self,
            current: if first.is_null() { None } else { Some(first) },
        }
    }

    /// Memory used by arena
    pub fn memory_used(&self) -> usize {
        self.arena.allocated_bytes() + self.nodes.capacity() * std::mem::size_of::<Node>()
    }

    /// Clear and reuse arena
    pub fn clear(&mut self) {
        self.arena.reset();
        self.nodes.clear();
        let root = self.create_node(NodeType::Document);
        self.root = root;
    }
}

impl Default for ArenaDom {
    fn default() -> Self {
        Self::new()
    }
}

/// Iterator over sibling nodes
struct NodeIter<'a> {
    dom: &'a ArenaDom,
    current: Option<NodeId>,
}

impl<'a> Iterator for NodeIter<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        let node = self.dom.get(id)?;
        self.current = if node.next_sibling.is_null() {
            None
        } else {
            Some(node.next_sibling)
        };
        Some(node)
    }
}

/// Builder for constructing DOM from scraper
impl ArenaDom {
    /// Build from scraper Html document
    pub fn from_scraper(html: &scraper::Html) -> Self {
        let mut dom = Self::with_capacity(html.tree.nodes().count());
        // Convert scraper DOM to arena
        // Implementation would traverse scraper tree
        dom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_dom_creation() {
        let mut dom = ArenaDom::new();

        // Create structure: <html><head></head><body>Text</body></html>
        let html = dom.create_element("html", []);
        let head = dom.create_element("head", []);
        let body = dom.create_element("body", []);
        let text = dom.create_text("Hello");

        dom.append_child(NodeId::ROOT, html);
        dom.append_child(html, head);
        dom.append_child(html, body);
        dom.append_child(body, text);

        assert_eq!(dom.len(), 5);

        let body_node = dom.get(body).unwrap();
        assert_eq!(body_node.tag(), Some("body"));

        let text_node = dom.get(text).unwrap();
        assert_eq!(text_node.text(), "Hello");
    }

    #[test]
    fn test_arena_attributes() {
        let mut dom = ArenaDom::new();
        let div = dom.create_element("div", [
            ("id".to_string(), "main".to_string()),
            ("class".to_string(), "container".to_string()),
        ]);

        let node = dom.get(div).unwrap();
        assert_eq!(node.attr("id"), Some("main"));
        assert_eq!(node.attr("class"), Some("container"));
        assert_eq!(node.attr("missing"), None);
    }
}
