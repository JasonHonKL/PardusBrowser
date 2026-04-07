// bootstrap_readonly.js — Read-only variant: DOM mutations are silently ignored.
// All reads work normally but writes/setters are no-ops.

// ==================== Core Types ====================

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
    this.eventPhase = 0;
  }
  preventDefault() { if (this.cancelable) { this.defaultPrevented = true; } }
  stopPropagation() { this.propagationStopped = true; }
  stopImmediatePropagation() { this.immediatePropagationStopped = true; this.propagationStopped = true; }
  initEvent(type, bubbles, cancelable) { this.type = type; this.bubbles = bubbles; this.cancelable = cancelable; }
}

class CustomEvent extends Event {
  constructor(type, eventInitDict = {}) { super(type, eventInitDict); this.detail = eventInitDict.detail || null; }
}

class MessageEvent {
  constructor(type, eventInitDict) {
    eventInitDict = eventInitDict || {};
    this.type = type;
    this.data = eventInitDict.data !== undefined ? eventInitDict.data : null;
    this.origin = eventInitDict.origin || '';
    this.lastEventId = eventInitDict.lastEventId || '';
    this.bubbles = false;
    this.cancelable = false;
    this.composed = false;
  }
}

// ==================== Element (Read-Only) ====================

class Element {
  constructor(nodeId) { this.__nodeId = nodeId; }

  get tagName() { return Deno.core.ops.op_get_tag_name(this.__nodeId); }
  get id() { return Deno.core.ops.op_get_attribute(this.__nodeId, "id") || ""; }
  set id(_v) { /* read-only: no-op */ }
  get className() { return Deno.core.ops.op_get_attribute(this.__nodeId, "class") || ""; }
  set className(_v) { /* read-only: no-op */ }

  getAttribute(name) { return Deno.core.ops.op_get_attribute(this.__nodeId, name); }
  setAttribute(_name, _value) { /* read-only: no-op */ }
  removeAttribute(_name) { /* read-only: no-op */ }
  hasAttribute(name) { return Deno.core.ops.op_get_attribute(this.__nodeId, name) !== null; }
  getAttributeNames() { return Deno.core.ops.op_get_attribute_names(this.__nodeId); }

  querySelector(selector) {
    const nid = Deno.core.ops.op_query_selector(this.__nodeId, selector);
    return nid ? new Element(nid) : null;
  }
  querySelectorAll(selector) {
    return Deno.core.ops.op_query_selector_all(this.__nodeId, selector).map(id => new Element(id));
  }
  getElementsByTagName(tag) { return Deno.core.ops.op_get_elements_by_tag_name(this.__nodeId, tag).map(id => new Element(id)); }
  getElementsByClassName(cn) { return Deno.core.ops.op_get_elements_by_class_name(this.__nodeId, cn).map(id => new Element(id)); }
  getElementById(id) { const nid = Deno.core.ops.op_get_element_by_id(this.__nodeId, id); return nid ? new Element(nid) : null; }

  get textContent() { return Deno.core.ops.op_get_text_content(this.__nodeId); }
  set textContent(_v) { /* read-only: no-op */ }
  get innerHTML() { return Deno.core.ops.op_get_inner_html(this.__nodeId); }
  set innerHTML(_v) { /* read-only: no-op */ }
  get outerHTML() { return Deno.core.ops.op_get_outer_html(this.__nodeId); }
  set outerHTML(_v) { /* read-only: no-op */ }

  get children() {
    const ids = Deno.core.ops.op_get_children(this.__nodeId);
    return ids.filter(id => Deno.core.ops.op_get_node_type(id) === 1).map(id => new Element(id));
  }
  get childNodes() {
    return Deno.core.ops.op_get_children(this.__nodeId).map(id => {
      const nt = Deno.core.ops.op_get_node_type(id);
      return nt === 1 ? new Element(id) : { nodeType: nt, textContent: Deno.core.ops.op_get_text_content(id) };
    });
  }
  get firstChild() { const ids = Deno.core.ops.op_get_children(this.__nodeId); return ids.length > 0 ? ids[0] : null; }
  get lastChild() { const ids = Deno.core.ops.op_get_children(this.__nodeId); return ids.length > 0 ? ids[ids.length - 1] : null; }
  get firstElementChild() { return Deno.core.ops.op_first_element_child(this.__nodeId) ? new Element(Deno.core.ops.op_first_element_child(this.__nodeId)) : null; }
  get lastElementChild() { return Deno.core.ops.op_last_element_child(this.__nodeId) ? new Element(Deno.core.ops.op_last_element_child(this.__nodeId)) : null; }
  get nextElementSibling() { const nid = Deno.core.ops.op_next_element_sibling(this.__nodeId); return nid ? new Element(nid) : null; }
  get previousElementSibling() { const nid = Deno.core.ops.op_previous_element_sibling(this.__nodeId); return nid ? new Element(nid) : null; }
  get parentElement() { const nid = Deno.core.ops.op_get_parent(this.__nodeId); return nid ? new Element(nid) : null; }
  get parentNode() { return this.parentElement; }

  appendChild(_child) { return _child; /* read-only: no-op */ }
  removeChild(_child) { return _child; /* read-only: no-op */ }
  insertBefore(_newChild, _refChild) { return _newChild; /* read-only: no-op */ }
  replaceChild(_newChild, _oldChild) { return _oldChild; /* read-only: no-op */ }
  cloneNode(deep) { const nid = Deno.core.ops.op_clone_node(this.__nodeId, deep); return nid ? new Element(nid) : null; }
  contains(other) { return Deno.core.ops.op_contains(this.__nodeId, other.__nodeId); }
  hasChildNodes() { const ids = Deno.core.ops.op_get_children(this.__nodeId); return ids.length > 0; }
  get childElementCount() { return this.children.length; }

  get style() { return new Proxy({}, { get: () => "", set: () => true }); }
  set style(_v) { /* read-only: no-op */ }

  get dataset() {
    const attrs = this.getAttributeNames();
    const ds = {};
    for (const a of attrs) { if (a.startsWith("data-")) ds[a.slice(5)] = this.getAttribute(a); }
    return ds;
  }

  get classList() {
    const self = this;
    return {
      add() {}, remove() {}, toggle() { return false; }, contains(c) { return self.className.split(" ").includes(c); },
      replace() { return false; }, item(i) { return null; }, forEach() {}, toString() { return self.className; },
      values() { return [].values(); }, keys() { return [].keys(); }, entries() { return [].entries(); },
      [Symbol.iterator]() { return [][Symbol.iterator](); },
      get length() { return self.className.split(" ").filter(Boolean).length; },
    };
  }

  get boundingClientRect() { return { x: 0, y: 0, width: 1280, height: 720, top: 0, right: 1280, bottom: 720, left: 0 }; }

  addEventListener(type, callback) { /* read-only: listeners don't fire */ }
  removeEventListener() {}
  dispatchEvent(_event) { return true; }
  click() {}
  focus() {}
  blur() {}
  get value() { return this.getAttribute("value") || ""; }
  set value(_v) { /* read-only */ }
  get checked() { return this.getAttribute("checked") !== null; }
  set checked(_v) { /* read-only */ }
  get selected() { return this.getAttribute("selected") !== null; }
  set selected(_v) { /* read-only */ }
  get disabled() { return this.getAttribute("disabled") !== null; }
  get href() { return this.getAttribute("href") || ""; }
  get src() { return this.getAttribute("src") || ""; }
  get alt() { return this.getAttribute("alt") || ""; }
  get placeholder() { return this.getAttribute("placeholder") || ""; }
  get type() { return this.getAttribute("type") || ""; }
  get name() { return this.getAttribute("name") || ""; }
  get action() { return this.getAttribute("action") || ""; }
  get method() { return this.getAttribute("method") || "get"; }
}

// ==================== Document (Read-Only) ====================

const document = {
  get nodeType() { return 9; },
  get documentElement() { return new Element(Deno.core.ops.op_get_document_element()); },
  get head() { return new Element(Deno.core.ops.op_get_head()); },
  get body() { return new Element(Deno.core.ops.op_get_body()); },
  get title() { return Deno.core.ops.op_get_title(); },
  set title(_v) { /* read-only */ },

  querySelector(selector) { const nid = Deno.core.ops.op_query_selector(0, selector); return nid ? new Element(nid) : null; },
  querySelectorAll(selector) { return Deno.core.ops.op_query_selector_all(0, selector).map(id => new Element(id)); },
  getElementById(id) { const nid = Deno.core.ops.op_get_element_by_id(0, id); return nid ? new Element(nid) : null; },
  getElementsByTagName(tag) { return Deno.core.ops.op_get_elements_by_tag_name(0, tag).map(id => new Element(id)); },
  getElementsByClassName(cn) { return Deno.core.ops.op_get_elements_by_class_name(0, cn).map(id => new Element(id)); },

  createElement(_tag) { return null; /* read-only */ },
  createTextNode(_text) { return null; /* read-only */ },
  createDocumentFragment() { return null; /* read-only */ },
  createComment(_text) { return null; /* read-only */ },

  addEventListener() {},
  removeEventListener() {},
  dispatchEvent(_event) { return true; },

  readyState: "complete",
  cookie: "",
  referrer: "",
  domain: "",
  URL: "",
  characterSet: "UTF-8",
  contentType: "text/html",
  compatMode: "CSS1Compat",
  hidden: false,
  visibilityState: "visible",
  hasFocus() { return true; },
};

// ==================== MutationObserver (Read-Only) ====================

class MutationObserver {
  constructor(_callback) {}
  observe() {}
  disconnect() {}
  takeRecords() { return []; }
}

// ==================== Storage (Read-Only) ====================

var _noopStorage = new Proxy({}, {
  get: function(_, prop) {
    if (prop === 'getItem') return function() { return null; };
    if (prop === 'setItem') return function() {};
    if (prop === 'removeItem') return function() {};
    if (prop === 'clear') return function() {};
    if (prop === 'key') return function() { return null; };
    if (prop === 'length') return 0;
    return null;
  },
  set: function() { return true; }
});

// ==================== Window (Read-Only) ====================

const window = {
  document,
  fetch() { return Promise.reject(new Error("fetch is disabled in read-only mode")); },
  localStorage: _noopStorage,
  sessionStorage: _noopStorage,
  addEventListener: document.addEventListener.bind(document),
  removeEventListener: document.removeEventListener.bind(document),
  location: new Proxy({
    href: "", origin: "", protocol: "https:", host: "", hostname: "", pathname: "/", search: "", hash: ""
  }, {
    get(target, prop) { return target[prop]; },
    set() { return true; /* read-only: silently ignore */ },
  }),
  navigator: { userAgent: "OpenBrowser/0.1.0" },
  console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
  innerWidth: 1280,
  innerHeight: 720,
  dispatchEvent() { return true; },
  Event,
  CustomEvent,
  setTimeout(fn, ms) { return 1; },
  setInterval(fn, ms) { return 1; },
  clearTimeout() {},
  clearInterval() {},
  getComputedStyle() { return new Proxy({}, { get: () => "" }); },
  matchMedia() { return { matches: false, addListener() {}, removeListener() {} }; },
};

// ==================== Globals ====================

globalThis.window = window;
globalThis.document = document;
globalThis.localStorage = _noopStorage;
globalThis.sessionStorage = _noopStorage;
globalThis.Element = Element;
globalThis.TextNode = Element;
globalThis.DocumentFragment = Element;
globalThis.Event = Event;
globalThis.CustomEvent = CustomEvent;
globalThis.MutationObserver = MutationObserver;
globalThis.MessageEvent = MessageEvent;
globalThis.Node = {
  ELEMENT_NODE: 1, TEXT_NODE: 3, COMMENT_NODE: 8, DOCUMENT_NODE: 9, DOCUMENT_FRAGMENT_NODE: 11,
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
globalThis.__openReadOnly = true;
