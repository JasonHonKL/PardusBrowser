//! deno_core extension registration.
//!
//! Combines all ops into a single extension for the JS runtime.

use super::fetch::op_fetch;
use super::ops::*;
use super::sse::*;

deno_core::extension!(
    open_dom,
    ops = [
        // Document methods
        op_create_element,
        op_create_text_node,
        op_create_document_fragment,
        op_get_element_by_id,
        op_query_selector,
        op_query_selector_all,
        op_get_document_element,
        op_get_head,
        op_get_body,
        // Node/Element methods
        op_append_child,
        op_remove_child,
        op_insert_before,
        op_replace_child,
        op_clone_node,
        // Attribute methods
        op_set_attribute,
        op_get_attribute,
        op_remove_attribute,
        // Property getters
        op_get_tag_name,
        op_get_node_id_attr,
        op_set_node_id_attr,
        op_get_class_name,
        op_set_class_name,
        op_get_inner_html,
        op_set_inner_html,
        op_get_text_content,
        op_set_text_content,
        op_get_parent,
        op_get_children,
        op_get_previous_sibling,
        // Style
        op_set_style,
        // Utility methods
        op_contains,
        op_has_child_nodes,
        op_has_attributes,
        op_get_node_type,
        op_get_node_name,
        op_get_attribute_names,
        // Fetch
        op_fetch,
        // Timers
        op_set_timeout,
        op_set_interval,
        op_clear_timer,
        // MutationObserver
        op_register_observer,
        op_disconnect_observer,
        op_take_mutation_records,
        op_has_observers,
        op_drain_pending_mutations,
        // Node manipulation
        op_set_node_value,
        op_set_node_name,
        op_copy_to,
        op_move_to,
        // Undo/Redo
        op_mark_undoable_state,
        op_undo,
        op_redo,
        // SSE / EventSource
        op_sse_open,
        op_sse_close,
        op_sse_ready_state,
        op_sse_url,
        // Cookies
        op_get_document_cookie,
        op_set_document_cookie,
        // localStorage
        op_local_storage_get,
        op_local_storage_set,
        op_local_storage_remove,
        op_local_storage_clear,
        op_local_storage_keys,
        op_local_storage_length,
        // sessionStorage
        op_session_storage_get,
        op_session_storage_set,
        op_session_storage_remove,
        op_session_storage_clear,
        op_session_storage_keys,
        op_session_storage_length,
    ],
);
