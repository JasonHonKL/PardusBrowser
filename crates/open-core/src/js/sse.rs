//! EventSource deno_core ops for Server-Sent Events.
//!
//! Provides sync ops to open/close SSE connections and query state.
//! Event dispatch is handled by the event loop drain phase in `runtime.rs`.

use deno_core::*;

use crate::sandbox::SandboxPolicy;
use crate::sse::manager::SseManager;
use crate::url_policy::UrlPolicy;

fn ensure_sse_manager(state: &mut OpState) {
    if !state.has::<SseManager>() {
        let url_policy = UrlPolicy::default();
        state.put(SseManager::new(url_policy));
    }
}

#[op2(fast)]
#[bigint]
pub fn op_sse_open(state: &mut OpState, #[string] url: String) -> u64 {
    // Sandbox: block SSE if policy says so
    if let Some(sandbox) = state.try_borrow::<SandboxPolicy>() {
        if sandbox.block_js_sse {
            return 0; // no-op
        }
    }
    ensure_sse_manager(state);
    let manager = state.borrow::<SseManager>();
    manager.open(url)
}

#[op2(fast)]
pub fn op_sse_close(state: &mut OpState, #[smi] id: u32) {
    if let Some(manager) = state.try_borrow::<SseManager>() {
        manager.close(id as u64);
    }
}

#[op2(fast)]
pub fn op_sse_ready_state(state: &OpState, #[smi] id: u32) -> u8 {
    if let Some(manager) = state.try_borrow::<SseManager>() {
        manager.ready_state(id as u64)
    } else {
        2
    }
}

#[op2]
#[string]
pub fn op_sse_url(state: &OpState, #[smi] id: u32) -> String {
    state
        .try_borrow::<SseManager>()
        .and_then(|m| m.url(id as u64))
        .unwrap_or_default()
}
