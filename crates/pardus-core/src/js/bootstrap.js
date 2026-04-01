// bootstrap.js — Sets up document, window, Element, fetch for pardus-browser JS runtime.

// ==================== Event System ====================

// Event listener storage (global)
const _eventListeners = new Map();

class Event {
  constructor(type, eventInitDict = {}) {
    this.type = type;
    this.bubbles = eventInitDict.bubbles || false;
    this.cancelable = eventInitDict.cancelable || false;
    this.composed = eventInitDict.composed || false;
    this.detail = eventInitDict.detail || null;
    this.timeStamp = Date.now();
    this.defaultPrevented = false;
    this.propagationStopped = false;
    this.immediatePropagationStopped = false;
    this.target = null;
    this.currentTarget = null;
    this.eventPhase = 0; // NONE: 0, CAPTURING: 1, AT_TARGET: 2, BUBBLING: 3
  }

  preventDefault() {
    if (this.cancelable) {
      this.defaultPrevented = true;
    }
  }

  stopPropagation() {
    this.propagationStopped = true;
  }

  stopImmediatePropagation() {
    this.immediatePropagationStopped = true;
    this.propagationStopped = true;
  }

  initEvent(type, bubbles, cancelable) {
    this.type = type;
    this.bubbles = bubbles;
    this.cancelable = cancelable;
  }
}

class CustomEvent extends Event {
  constructor(type, eventInitDict = {}) {
    super(type, eventInitDict);
    this.detail = eventInitDict.detail || null;
  }
}

// Event phases
Event.NONE = 0;
Event.CAPTURING_PHASE = 1;
Event.AT_TARGET = 2;
Event.BUBBLING_PHASE = 3;

// Helper: Get or create listener array for a node
function _getListeners(nodeId, eventType) {
  if (!_eventListeners.has(nodeId)) {
    _eventListeners.set(nodeId, new Map());
  }
  const nodeListeners = _eventListeners.get(nodeId);
  if (!nodeListeners.has(eventType)) {
    nodeListeners.set(eventType, []);
  }
  return nodeListeners.get(eventType);
}

// Helper: Dispatch event through the DOM tree
function _dispatchEventThroughTree(nodeId, event, phase) {
  const listeners = _getListeners(nodeId, event.type);
  const element = new Element(nodeId);

  event.currentTarget = element;
  event.eventPhase = phase;

  for (const listener of listeners) {
    if (event.immediatePropagationStopped) break;

    try {
      if (typeof listener.callback === 'function') {
        listener.callback.call(element, event);
      } else if (listener.callback && typeof listener.callback.handleEvent === 'function') {
        listener.callback.handleEvent(event);
      }
    } catch (e) {
      // Ignore errors in event handlers
    }
  }
}

// ==================== Element wrapper ====================

class Element {
  constructor(nodeId) {
    this.__nodeId = nodeId;
  }

  // ---- Properties ----
  get tagName() { return Deno.core.ops.op_get_tag_name(this.__nodeId); }
  get id() { return Deno.core.ops.op_get_node_id_attr(this.__nodeId); }
  set id(v) { Deno.core.ops.op_set_node_id_attr(this.__nodeId, v); }
  get className() { return Deno.core.ops.op_get_class_name(this.__nodeId); }
  set className(v) { Deno.core.ops.op_set_class_name(this.__nodeId, v); }
  get innerHTML() { return Deno.core.ops.op_get_inner_html(this.__nodeId); }
  set innerHTML(v) { Deno.core.ops.op_set_inner_html(this.__nodeId, v); }
  get textContent() { return Deno.core.ops.op_get_text_content(this.__nodeId); }
  set textContent(v) { Deno.core.ops.op_set_text_content(this.__nodeId, v); }
  get outerHTML() { return Deno.core.ops.op_get_inner_html(this.__nodeId); }

  get children() {
    return Deno.core.ops.op_get_children(this.__nodeId).map(id => new Element(id));
  }

  get childElementCount() { return Deno.core.ops.op_get_children(this.__nodeId).length; }

  get firstChild() {
    const ids = Deno.core.ops.op_get_children(this.__nodeId);
    return ids.length > 0 ? new Element(ids[0]) : null;
  }

  get lastChild() {
    const ids = Deno.core.ops.op_get_children(this.__nodeId);
    return ids.length > 0 ? new Element(ids[ids.length - 1]) : null;
  }

  get parentElement() {
    const pid = Deno.core.ops.op_get_parent(this.__nodeId);
    return pid ? new Element(pid) : null;
  }

  get nextSibling() {
    const pid = Deno.core.ops.op_get_parent(this.__nodeId);
    if (!pid) return null;
    const siblings = Deno.core.ops.op_get_children(pid);
    const idx = siblings.indexOf(this.__nodeId);
    return idx >= 0 && idx < siblings.length - 1 ? new Element(siblings[idx + 1]) : null;
  }

  get previousSibling() {
    const sid = Deno.core.ops.op_get_previous_sibling(this.__nodeId);
    return sid ? new Element(sid) : null;
  }

  get nodeType() { return Deno.core.ops.op_get_node_type(this.__nodeId); }
  get nodeName() { return Deno.core.ops.op_get_node_name(this.__nodeId); }

  // ---- DOM Manipulation ----
  appendChild(child) {
    Deno.core.ops.op_append_child(this.__nodeId, child.__nodeId);
    return child;
  }

  removeChild(child) {
    Deno.core.ops.op_remove_child(this.__nodeId, child.__nodeId);
    return child;
  }

  insertBefore(newNode, refNode) {
    const refId = refNode ? refNode.__nodeId : 0;
    Deno.core.ops.op_insert_before(this.__nodeId, newNode.__nodeId, refId);
    return newNode;
  }

  replaceChild(newChild, oldChild) {
    Deno.core.ops.op_replace_child(this.__nodeId, newChild.__nodeId, oldChild.__nodeId);
    return oldChild;
  }

  cloneNode(deep = false) {
    const newId = Deno.core.ops.op_clone_node(this.__nodeId, deep);
    return new Element(newId);
  }

  // ---- Attributes ----
  setAttribute(name, value) {
    Deno.core.ops.op_set_attribute(this.__nodeId, name, String(value));
  }

  getAttribute(name) {
    return Deno.core.ops.op_get_attribute(this.__nodeId, name);
  }

  removeAttribute(name) {
    Deno.core.ops.op_remove_attribute(this.__nodeId, name);
  }

  hasAttribute(name) {
    return Deno.core.ops.op_get_attribute(this.__nodeId, name) !== null;
  }

  hasAttributes() {
    return Deno.core.ops.op_has_attributes(this.__nodeId);
  }

  getAttributeNames() {
    return Deno.core.ops.op_get_attribute_names(this.__nodeId);
  }

  // ---- Query Selectors ----
  querySelector(selector) {
    const id = Deno.core.ops.op_query_selector(this.__nodeId, selector);
    return id ? new Element(id) : null;
  }

  querySelectorAll(selector) {
    return Deno.core.ops.op_query_selector_all(this.__nodeId, selector).map(id => new Element(id));
  }

  // ---- Event Handling ----
  addEventListener(type, callback, options = {}) {
    const capture = typeof options === 'boolean' ? options : (options.capture || false);
    const listeners = _getListeners(this.__nodeId, type);

    // Check for duplicate
    const exists = listeners.some(l => l.callback === callback && l.capture === capture);
    if (!exists) {
      listeners.push({ callback, capture, once: options.once || false });
    }
  }

  removeEventListener(type, callback, options = {}) {
    const capture = typeof options === 'boolean' ? options : (options.capture || false);
    const listeners = _getListeners(this.__nodeId, type);
    const idx = listeners.findIndex(l => l.callback === callback && l.capture === capture);
    if (idx >= 0) {
      listeners.splice(idx, 1);
    }
  }

  dispatchEvent(event) {
    event.target = this;

    // Build propagation path (simplified - just parent chain)
    const path = [];
    let current = this.__nodeId;
    while (current) {
      path.unshift(current);
      current = Deno.core.ops.op_get_parent(current);
      if (!current) break;
    }

    // Capturing phase
    event.eventPhase = Event.CAPTURING_PHASE;
    for (const nodeId of path.slice(0, -1)) {
      if (event.propagationStopped) break;
      _dispatchEventThroughTree(nodeId, event, Event.CAPTURING_PHASE);
    }

    // At target
    if (!event.propagationStopped) {
      event.eventPhase = Event.AT_TARGET;
      _dispatchEventThroughTree(this.__nodeId, event, Event.AT_TARGET);
    }

    // Bubbling phase
    if (event.bubbles && !event.propagationStopped) {
      event.eventPhase = Event.BUBBLING_PHASE;
      for (const nodeId of path.slice(0, -1).reverse()) {
        if (event.propagationStopped) break;
        _dispatchEventThroughTree(nodeId, event, Event.BUBBLING_PHASE);
      }
    }

    event.eventPhase = Event.NONE;
    return !event.defaultPrevented;
  }

  // ---- Utility Methods ----
  contains(other) {
    if (!other) return false;
    return Deno.core.ops.op_contains(this.__nodeId, other.__nodeId);
  }

  hasChildNodes() {
    return Deno.core.ops.op_has_child_nodes(this.__nodeId);
  }

  // ---- Style ----
  get style() {
    const nodeId = this.__nodeId;
    return new Proxy({}, {
      set(_, prop, value) {
        Deno.core.ops.op_set_style(nodeId, prop, String(value));
        return true;
      },
      get(_, prop) {
        // Return empty string for style property reads
        return '';
      }
    });
  }

  // ---- Class List (simplified) ----
  get classList() {
    const nodeId = this.__nodeId;
    const self = this;
    return {
      add(...tokens) {
        const current = self.className.split(/\s+/).filter(s => s);
        for (const token of tokens) {
          if (!current.includes(token)) {
            current.push(token);
          }
        }
        self.className = current.join(' ');
      },
      remove(...tokens) {
        const current = self.className.split(/\s+/).filter(s => s);
        for (const token of tokens) {
          const idx = current.indexOf(token);
          if (idx >= 0) current.splice(idx, 1);
        }
        self.className = current.join(' ');
      },
      toggle(token, force) {
        const current = self.className.split(/\s+/).filter(s => s);
        const has = current.includes(token);
        if (force === true || (!has && force !== false)) {
          if (!has) current.push(token);
          self.className = current.join(' ');
          return true;
        } else if (force === false || has) {
          const idx = current.indexOf(token);
          if (idx >= 0) current.splice(idx, 1);
          self.className = current.join(' ');
          return false;
        }
        return has;
      },
      contains(token) {
        return self.className.split(/\s+/).includes(token);
      },
      get length() {
        return self.className.split(/\s+/).filter(s => s).length;
      }
    };
  }

  // ---- Dataset ----
  get dataset() {
    const nodeId = this.__nodeId;
    return new Proxy({}, {
      set(_, prop, value) {
        const attrName = 'data-' + prop.replace(/([A-Z])/g, '-$1').toLowerCase();
        Deno.core.ops.op_set_attribute(nodeId, attrName, String(value));
        return true;
      },
      get(_, prop) {
        const attrName = 'data-' + prop.replace(/([A-Z])/g, '-$1').toLowerCase();
        return Deno.core.ops.op_get_attribute(nodeId, attrName) || undefined;
      }
    });
  }

  // ---- Convenience Methods ----
  focus() { /* no-op for headless */ }
  blur() { /* no-op for headless */ }
  click() {
    const event = new Event('click', { bubbles: true, cancelable: true });
    this.dispatchEvent(event);
  }
}

// ==================== MutationObserver ====================

class MutationObserver {
  constructor(callback) {
    this.__callback = callback;
    this.__id = Deno.core.ops.op_register_observer(0, true, true, false);
  }

  observe(target, options = {}) {
    if (target && target.__nodeId) {
      this.__id = Deno.core.ops.op_register_observer(
        target.__nodeId,
        options.childList || false,
        options.attributes !== false,
        options.subtree || false
      );
    }
  }

  disconnect() {
    Deno.core.ops.op_disconnect_observer(this.__id);
  }

  takeRecords() {
    return Deno.core.ops.op_take_mutation_records().map(r => ({
      type: r.type_,
      target: r.target ? new Element(r.target) : null,
      addedNodes: (r.added_nodes || []).map(id => new Element(id)),
      removedNodes: (r.removed_nodes || []).map(id => new Element(id)),
      attributeName: r.attribute_name || null,
      oldValue: r.old_value || null,
    }));
  }
}

// ==================== TextNode wrapper ====================

class TextNode {
  constructor(nodeId) {
    this.__nodeId = nodeId;
  }
  get textContent() { return Deno.core.ops.op_get_text_content(this.__nodeId); }
  set textContent(v) { Deno.core.ops.op_set_text_content(this.__nodeId, v); }
  get nodeType() { return 3; }
  get nodeName() { return '#text'; }
  get parentElement() {
    const pid = Deno.core.ops.op_get_parent(this.__nodeId);
    return pid ? new Element(pid) : null;
  }
}

// ==================== DocumentFragment wrapper ====================

class DocumentFragment {
  constructor(nodeId) {
    this.__nodeId = nodeId;
  }
  appendChild(child) {
    Deno.core.ops.op_append_child(this.__nodeId, child.__nodeId);
    return child;
  }
  get children() {
    return Deno.core.ops.op_get_children(this.__nodeId).map(id => new Element(id));
  }
  get nodeType() { return 11; }
  get nodeName() { return '#document-fragment'; }
  querySelector(selector) {
    const id = Deno.core.ops.op_query_selector(this.__nodeId, selector);
    return id ? new Element(id) : null;
  }
  querySelectorAll(selector) {
    return Deno.core.ops.op_query_selector_all(this.__nodeId, selector).map(id => new Element(id));
  }
}

// ==================== DOMContentLoaded ====================

let _DOMContentLoadedFired = false;
const _DOMContentLoadedListeners = [];

function _fireDOMContentLoaded() {
    _DOMContentLoadedFired = true;
    for (const listener of _DOMContentLoadedListeners) {
        try { listener.callback(new Event('DOMContentLoaded')); } catch (e) {}
    }
    _DOMContentLoadedListeners.length = 0;
}

// ==================== Document object ====================

const document = {
  createElement(tag) { return new Element(Deno.core.ops.op_create_element(tag)); },
  createTextNode(text) { return new TextNode(Deno.core.ops.op_create_text_node(text)); },
  createDocumentFragment() { return new DocumentFragment(Deno.core.ops.op_create_document_fragment()); },
  getElementById(id) {
    const nid = Deno.core.ops.op_get_element_by_id(id);
    return nid ? new Element(nid) : null;
  },
  querySelector(selector) {
    const nid = Deno.core.ops.op_query_selector(0, selector);
    return nid ? new Element(nid) : null;
  },
  querySelectorAll(selector) {
    return Deno.core.ops.op_query_selector_all(0, selector).map(id => new Element(id));
  },
  get documentElement() { return new Element(Deno.core.ops.op_get_document_element()); },
  get head() { return new Element(Deno.core.ops.op_get_head()); },
  get body() { return new Element(Deno.core.ops.op_get_body()); },

  // Event handling
  addEventListener(type, callback, options) {
    if (type === 'DOMContentLoaded') {
      _DOMContentLoadedListeners.push({ callback, options });
      if (_DOMContentLoadedFired) {
        try { callback(new Event('DOMContentLoaded')); } catch (e) {}
      }
      return;
    }
    const docEl = this.documentElement;
    if (docEl) {
      docEl.addEventListener(type, callback, options);
    }
  },

  removeEventListener(type, callback, options) {
    if (type === 'DOMContentLoaded') {
      const idx = _DOMContentLoadedListeners.findIndex(l => l.callback === callback);
      if (idx >= 0) _DOMContentLoadedListeners.splice(idx, 1);
      return;
    }
    const docEl = this.documentElement;
    if (docEl) {
      docEl.removeEventListener(type, callback, options);
    }
  },

  dispatchEvent(event) {
    const docEl = this.documentElement;
    if (docEl) {
      return docEl.dispatchEvent(event);
    }
    return true;
  },

  createEvent(type) {
    const eventClasses = {
      'Event': Event,
      'CustomEvent': CustomEvent,
      'UIEvent': Event,
      'MouseEvent': Event,
      'KeyboardEvent': Event,
    };
    const EventClass = eventClasses[type] || Event;
    const event = new EventClass('');
    return event;
  }
};

// ==================== Fetch polyfill ====================

async function fetch(input, init) {
  init = init || {};
  const url = typeof input === "string" ? input : (input.url || String(input));
  const method = init.method || "GET";
  const headers = {};
  if (init.headers) {
    if (init.headers instanceof Map) {
      init.headers.forEach((v, k) => headers[k] = v);
    } else if (typeof init.headers === "object") {
      Object.assign(headers, init.headers);
    }
  }
  const resp = await Deno.core.ops.op_fetch({
    url,
    method,
    headers,
    body: init.body || null
  });

  return {
    ok: resp.ok,
    status: resp.status,
    statusText: resp.status_text,
    url,
    headers: new Map(Object.entries(resp.headers || {})),
    text: async () => resp.body,
    json: async () => JSON.parse(resp.body),
  };
}

// ==================== Window object ====================

const window = {
  document,
  fetch,
  addEventListener: document.addEventListener.bind(document),
  removeEventListener: document.removeEventListener.bind(document),
  location: {
    href: "",
    origin: "",
    protocol: "https:",
    host: "",
    hostname: "",
    pathname: "/",
    search: "",
    hash: ""
  },
  navigator: { userAgent: "PardusBrowser/0.1.0" },
  console: {
    log(...a) {},
    warn(...a) {},
    error(...a) {},
    info(...a) {},
    debug(...a) {},
  },
  setTimeout(fn, ms) {
    // Don't execute - just return a fake timer ID
    // Executing callbacks synchronously can cause infinite loops on complex sites
    return 1;
  },
  setInterval(fn, ms) {
    // Don't execute - just return a fake timer ID
    return 1;
  },
  clearTimeout() {},
  clearInterval() {},
  getComputedStyle() { return new Proxy({}, { get: () => "" }); },
  matchMedia() {
    return { matches: false, addListener() {}, removeListener() {} };
  },
  innerWidth: 1280,
  innerHeight: 720,
  dispatchEvent(event) {
    return document.dispatchEvent(event);
  },
  Event,
  CustomEvent,
};

// ==================== Globals ====================

globalThis.window = window;
globalThis.document = document;
globalThis.fetch = fetch;
globalThis.Element = Element;
globalThis.TextNode = TextNode;
globalThis.DocumentFragment = DocumentFragment;
globalThis.Event = Event;
globalThis.CustomEvent = CustomEvent;
globalThis.MutationObserver = MutationObserver;
globalThis.Node = {
  ELEMENT_NODE: 1,
  TEXT_NODE: 3,
  DOCUMENT_FRAGMENT_NODE: 11,
  DOCUMENT_NODE: 9
};
globalThis.setTimeout = window.setTimeout;
globalThis.setInterval = window.setInterval;
globalThis.clearTimeout = window.clearTimeout;
globalThis.clearInterval = window.clearInterval;
globalThis.console = window.console;
globalThis.navigator = window.navigator;
globalThis.performance = { now: () => Date.now() };
globalThis.self = globalThis;
globalThis.top = globalThis;
globalThis.parent = globalThis;
globalThis.frames = globalThis;
globalThis.__modules = {};
