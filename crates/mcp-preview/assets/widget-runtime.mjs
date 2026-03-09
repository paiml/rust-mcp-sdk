// src/runtime.ts
var WidgetRuntime = class {
  constructor() {
    this._ready = false;
    this._eventHandlers = /* @__PURE__ */ new Map();
    this._state = {};
    this._hostInfo = null;
    this._readyPromise = new Promise((resolve) => {
      this._resolveReady = resolve;
    });
    if (typeof window !== "undefined") {
      if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", () => this._initialize());
      } else {
        this._initialize();
      }
    }
  }
  // ===========================================================================
  // Initialization
  // ===========================================================================
  _initialize() {
    this._hostInfo = this._detectHost();
    this._setupEventListeners();
    if (this._getBridge()) {
      this._markReady();
    } else {
      setTimeout(() => {
        if (this._getBridge()) {
          this._markReady();
        }
      }, 100);
    }
  }
  _detectHost() {
    const capabilities = this._detectCapabilities();
    let type = "unknown";
    let name = "Unknown";
    if (typeof window !== "undefined") {
      if (window.openai) {
        type = "chatgpt";
        name = "ChatGPT";
      } else if (window.mcpBridge) {
        if (window.mcpBridge.sendIntent) {
          type = "mcp-ui";
          name = "MCP-UI Host";
        } else {
          type = "mcp-apps";
          name = "MCP Apps Host";
        }
      }
    }
    return {
      type,
      name,
      ready: this._ready,
      capabilities
    };
  }
  _detectCapabilities() {
    const bridge = this._getBridge();
    return {
      callTool: !!bridge?.callTool,
      readResource: !!bridge?.readResource,
      getPrompt: !!bridge?.getPrompt,
      state: !!bridge?.getState && !!bridge?.setState,
      sendMessage: !!bridge?.sendMessage,
      openExternal: !!bridge?.openExternal,
      notify: !!bridge?.notify,
      sendIntent: !!bridge?.sendIntent,
      uploadFile: !!bridge?.uploadFile,
      downloadFile: !!bridge?.getFileDownloadUrl,
      displayModes: !!bridge?.requestDisplayMode,
      intrinsicHeight: !!bridge?.notifyIntrinsicHeight
    };
  }
  _getBridge() {
    if (typeof window === "undefined") return void 0;
    return window.mcpBridge;
  }
  _markReady() {
    if (this._ready) return;
    this._ready = true;
    if (this._hostInfo) {
      this._hostInfo.ready = true;
      this._hostInfo.capabilities = this._detectCapabilities();
    }
    this._emit("ready", void 0);
    this._resolveReady();
  }
  _setupEventListeners() {
    if (typeof window === "undefined") return;
    window.addEventListener("widgetStateUpdate", ((event) => {
      this._state = event.detail;
      this._emit("stateUpdate", event.detail);
    }));
    window.addEventListener("mcpNotification", ((event) => {
      this._emit("notification", event.detail);
    }));
  }
  // ===========================================================================
  // Public API - Lifecycle
  // ===========================================================================
  /**
   * Wait for the widget runtime to be ready.
   *
   * @returns Promise that resolves when the bridge is available
   *
   * @example
   * ```typescript
   * await runtime.ready();
   * console.log('Bridge is ready!');
   * ```
   */
  ready() {
    return this._readyPromise;
  }
  /**
   * Check if the runtime is ready.
   */
  get isReady() {
    return this._ready;
  }
  /**
   * Get information about the host platform.
   */
  get host() {
    return this._hostInfo ?? {
      type: "unknown",
      name: "Unknown",
      ready: false,
      capabilities: {
        callTool: false,
        readResource: false,
        getPrompt: false,
        state: false,
        sendMessage: false,
        openExternal: false,
        notify: false,
        sendIntent: false,
        uploadFile: false,
        downloadFile: false,
        displayModes: false,
        intrinsicHeight: false
      }
    };
  }
  // ===========================================================================
  // Public API - Tools
  // ===========================================================================
  /**
   * Call an MCP tool.
   *
   * @param name - Tool name
   * @param args - Tool arguments
   * @param options - Call options
   * @returns Tool result
   *
   * @example
   * ```typescript
   * const result = await runtime.callTool('get_weather', { city: 'Seattle' });
   * if (result.success) {
   *   console.log('Weather:', result.data);
   * }
   * ```
   */
  async callTool(name, args, options) {
    const bridge = this._getBridge();
    if (!bridge?.callTool) {
      return { success: false, error: "Tool calls not supported on this host" };
    }
    const timeout = options?.timeout ?? 3e4;
    const controller = new AbortController();
    const signal = options?.signal ?? controller.signal;
    const timeoutId = setTimeout(() => controller.abort(), timeout);
    try {
      const result = await Promise.race([
        bridge.callTool(name, args),
        new Promise((_, reject) => {
          signal.addEventListener("abort", () => reject(new Error("Request aborted")));
        })
      ]);
      clearTimeout(timeoutId);
      if (typeof result === "object" && result !== null) {
        const r = result;
        if (r.error || r.isError) {
          return { success: false, error: String(r.content ?? "Unknown error"), isError: true };
        }
        return { success: true, data: result };
      }
      return { success: true, data: result };
    } catch (error) {
      clearTimeout(timeoutId);
      return { success: false, error: error instanceof Error ? error.message : String(error) };
    }
  }
  // ===========================================================================
  // Public API - Resources
  // ===========================================================================
  /**
   * Read an MCP resource.
   *
   * @param uri - Resource URI
   * @returns Resource contents
   *
   * @example
   * ```typescript
   * const result = await runtime.readResource('file:///config.json');
   * if (result) {
   *   console.log('Config:', result.contents);
   * }
   * ```
   */
  async readResource(uri) {
    const bridge = this._getBridge();
    if (!bridge?.readResource) {
      console.warn("Resource reading not supported on this host");
      return null;
    }
    try {
      const result = await bridge.readResource(uri);
      return result;
    } catch (error) {
      console.error("Failed to read resource:", error);
      return null;
    }
  }
  // ===========================================================================
  // Public API - Prompts
  // ===========================================================================
  /**
   * Get an MCP prompt.
   *
   * @param name - Prompt name
   * @param args - Prompt arguments
   * @returns Prompt result
   *
   * @example
   * ```typescript
   * const prompt = await runtime.getPrompt('analyze_code', { language: 'rust' });
   * if (prompt) {
   *   console.log('Prompt messages:', prompt.messages);
   * }
   * ```
   */
  async getPrompt(name, args) {
    const bridge = this._getBridge();
    if (!bridge?.getPrompt) {
      console.warn("Prompts not supported on this host");
      return null;
    }
    try {
      const result = await bridge.getPrompt(name, args);
      return result;
    } catch (error) {
      console.error("Failed to get prompt:", error);
      return null;
    }
  }
  // ===========================================================================
  // Public API - State (ChatGPT)
  // ===========================================================================
  /**
   * Get current widget state.
   *
   * State is preserved across tool calls in ChatGPT Apps.
   *
   * @returns Current state object
   *
   * @example
   * ```typescript
   * const state = runtime.getState();
   * console.log('Current turn:', state.turn);
   * ```
   */
  getState() {
    const bridge = this._getBridge();
    if (bridge?.getState) {
      return bridge.getState();
    }
    return this._state;
  }
  /**
   * Update widget state.
   *
   * Merges new state with existing state.
   *
   * @param state - State updates to apply
   *
   * @example
   * ```typescript
   * runtime.setState({ turn: 'white', moveCount: 10 });
   * ```
   */
  setState(state) {
    const bridge = this._getBridge();
    const newState = { ...this._state, ...state };
    this._state = newState;
    if (bridge?.setState) {
      bridge.setState(newState);
    }
  }
  // ===========================================================================
  // Public API - Communication
  // ===========================================================================
  /**
   * Send a follow-up message to the AI.
   *
   * Available on ChatGPT Apps.
   *
   * @param message - Message text
   *
   * @example
   * ```typescript
   * runtime.sendMessage('Show me the next move');
   * ```
   */
  sendMessage(message) {
    const bridge = this._getBridge();
    if (bridge?.sendMessage) {
      bridge.sendMessage(message);
    } else {
      console.warn("sendMessage not supported on this host");
    }
  }
  /**
   * Show a notification to the user.
   *
   * Available on MCP-UI hosts.
   *
   * @param level - Notification level
   * @param message - Notification message
   *
   * @example
   * ```typescript
   * runtime.notify('success', 'Move completed!');
   * ```
   */
  notify(level, message) {
    const bridge = this._getBridge();
    if (bridge?.notify) {
      bridge.notify(level, message);
    } else {
      console[level === "error" ? "error" : level === "warning" ? "warn" : "log"](
        `[${level.toUpperCase()}] ${message}`
      );
    }
  }
  /**
   * Open an external URL.
   *
   * @param url - URL to open
   *
   * @example
   * ```typescript
   * runtime.openExternal('https://chess.com/learn');
   * ```
   */
  openExternal(url) {
    const bridge = this._getBridge();
    if (bridge?.openExternal) {
      bridge.openExternal(url);
    } else if (bridge?.openLink) {
      bridge.openLink(url);
    } else {
      window.open(url, "_blank", "noopener,noreferrer");
    }
  }
  /**
   * Send an intent action.
   *
   * Available on MCP-UI hosts.
   *
   * @param action - Intent action name
   * @param data - Intent data
   * @returns Intent result
   *
   * @example
   * ```typescript
   * await runtime.sendIntent('navigate', { route: '/settings' });
   * ```
   */
  async sendIntent(action, data) {
    const bridge = this._getBridge();
    if (!bridge?.sendIntent) {
      console.warn("sendIntent not supported on this host");
      return null;
    }
    try {
      return await bridge.sendIntent(action, data);
    } catch (error) {
      console.error("Failed to send intent:", error);
      return null;
    }
  }
  // ===========================================================================
  // Public API - Tool Context (ChatGPT)
  // ===========================================================================
  /**
   * Get the arguments that were passed when the tool was invoked.
   *
   * Available on ChatGPT Apps.
   *
   * @returns Tool input arguments
   *
   * @example
   * ```typescript
   * const input = runtime.getToolInput();
   * console.log('Tool was called with:', input);
   * ```
   */
  getToolInput() {
    const bridge = this._getBridge();
    return bridge?.toolInput ?? window.openai?.toolInput ?? {};
  }
  /**
   * Get the structured content returned by the tool.
   *
   * Available on ChatGPT Apps.
   *
   * @returns Tool output (structuredContent)
   *
   * @example
   * ```typescript
   * const output = runtime.getToolOutput();
   * console.log('Tool returned:', output);
   * ```
   */
  getToolOutput() {
    const bridge = this._getBridge();
    return bridge?.toolOutput ?? window.openai?.toolOutput;
  }
  /**
   * Get the tool response metadata (_meta payload).
   *
   * This is widget-only data that is never sent to the model.
   *
   * Available on ChatGPT Apps.
   *
   * @returns Tool response metadata
   *
   * @example
   * ```typescript
   * const meta = runtime.getToolResponseMetadata();
   * console.log('Widget session ID:', meta['openai/widgetSessionId']);
   * ```
   */
  getToolResponseMetadata() {
    const bridge = this._getBridge();
    return bridge?.toolResponseMetadata ?? window.openai?.toolResponseMetadata ?? {};
  }
  // ===========================================================================
  // Public API - File Operations (ChatGPT)
  // ===========================================================================
  /**
   * Upload a file and get a file ID.
   *
   * Supported file types: image/png, image/jpeg, image/webp
   *
   * Available on ChatGPT Apps.
   *
   * @param file - File to upload
   * @returns Upload result with file ID, or null if not supported
   *
   * @example
   * ```typescript
   * const input = document.querySelector('input[type="file"]');
   * const file = input.files[0];
   * const result = await runtime.uploadFile(file);
   * if (result) {
   *   console.log('Uploaded file ID:', result.fileId);
   * }
   * ```
   */
  async uploadFile(file) {
    const bridge = this._getBridge();
    if (bridge?.uploadFile) {
      try {
        return await bridge.uploadFile(file);
      } catch (error) {
        console.error("Failed to upload file:", error);
        return null;
      }
    }
    if (window.openai?.uploadFile) {
      try {
        return await window.openai.uploadFile(file);
      } catch (error) {
        console.error("Failed to upload file:", error);
        return null;
      }
    }
    console.warn("File upload not supported on this host");
    return null;
  }
  /**
   * Get a temporary download URL for a file.
   *
   * Available on ChatGPT Apps.
   *
   * @param fileId - The file ID to get download URL for
   * @returns Download URL, or null if not supported
   *
   * @example
   * ```typescript
   * const url = await runtime.getFileDownloadUrl('file-123');
   * if (url) {
   *   const img = document.createElement('img');
   *   img.src = url;
   * }
   * ```
   */
  async getFileDownloadUrl(fileId) {
    const bridge = this._getBridge();
    if (bridge?.getFileDownloadUrl) {
      try {
        const result = await bridge.getFileDownloadUrl(fileId);
        return result.downloadUrl;
      } catch (error) {
        console.error("Failed to get file download URL:", error);
        return null;
      }
    }
    if (window.openai?.getFileDownloadUrl) {
      try {
        const result = await window.openai.getFileDownloadUrl({ fileId });
        return result.downloadUrl;
      } catch (error) {
        console.error("Failed to get file download URL:", error);
        return null;
      }
    }
    console.warn("File download not supported on this host");
    return null;
  }
  // ===========================================================================
  // Public API - Display Modes (ChatGPT)
  // ===========================================================================
  /**
   * Request a display mode change.
   *
   * Available on ChatGPT Apps. On mobile, PiP may be coerced to fullscreen.
   *
   * @param mode - The display mode to request
   * @returns Whether the request was successful
   *
   * @example
   * ```typescript
   * // Go fullscreen for a better experience
   * const success = await runtime.requestDisplayMode('fullscreen');
   * if (success) {
   *   console.log('Now in fullscreen mode');
   * }
   * ```
   */
  async requestDisplayMode(mode) {
    const bridge = this._getBridge();
    if (bridge?.requestDisplayMode) {
      try {
        await bridge.requestDisplayMode(mode);
        return true;
      } catch (error) {
        console.error("Failed to request display mode:", error);
        return false;
      }
    }
    if (window.openai?.requestDisplayMode) {
      try {
        await window.openai.requestDisplayMode({ mode });
        return true;
      } catch (error) {
        console.error("Failed to request display mode:", error);
        return false;
      }
    }
    console.warn("Display mode changes not supported on this host");
    return false;
  }
  /**
   * Close the widget.
   *
   * Available on ChatGPT Apps.
   *
   * @example
   * ```typescript
   * // Close when user clicks "Done"
   * doneButton.onclick = () => runtime.requestClose();
   * ```
   */
  requestClose() {
    const bridge = this._getBridge();
    if (bridge?.requestClose) {
      bridge.requestClose();
    } else if (window.openai?.requestClose) {
      window.openai.requestClose();
    } else {
      console.warn("Widget close not supported on this host");
    }
  }
  /**
   * Report the widget's intrinsic height to avoid scroll clipping.
   *
   * Available on ChatGPT Apps.
   *
   * @param height - Height in pixels
   *
   * @example
   * ```typescript
   * // Report height after content changes
   * const height = document.body.scrollHeight;
   * runtime.notifyIntrinsicHeight(height);
   * ```
   */
  notifyIntrinsicHeight(height) {
    const bridge = this._getBridge();
    if (bridge?.notifyIntrinsicHeight) {
      bridge.notifyIntrinsicHeight(height);
    } else if (window.openai?.notifyIntrinsicHeight) {
      window.openai.notifyIntrinsicHeight(height);
    }
  }
  /**
   * Set the URL for the "Open in App" button in fullscreen mode.
   *
   * Available on ChatGPT Apps.
   *
   * @param href - URL to open
   *
   * @example
   * ```typescript
   * runtime.setOpenInAppUrl('https://myapp.com/game/123');
   * ```
   */
  setOpenInAppUrl(href) {
    const bridge = this._getBridge();
    if (bridge?.setOpenInAppUrl) {
      bridge.setOpenInAppUrl(href);
    } else if (window.openai?.setOpenInAppUrl) {
      window.openai.setOpenInAppUrl({ href });
    }
  }
  // ===========================================================================
  // Public API - Environment Context (ChatGPT)
  // ===========================================================================
  /**
   * Get the current theme setting.
   *
   * @returns 'light' or 'dark'
   *
   * @example
   * ```typescript
   * const theme = runtime.theme;
   * document.body.classList.add(`theme-${theme}`);
   * ```
   */
  get theme() {
    const bridge = this._getBridge();
    return bridge?.theme ?? window.openai?.theme ?? "light";
  }
  /**
   * Get the current locale.
   *
   * @returns Locale string (e.g., 'en-US')
   *
   * @example
   * ```typescript
   * const locale = runtime.locale;
   * const formatter = new Intl.NumberFormat(locale);
   * ```
   */
  get locale() {
    const bridge = this._getBridge();
    return bridge?.locale ?? window.openai?.locale ?? "en-US";
  }
  /**
   * Get the current display mode.
   *
   * @returns 'inline', 'pip', or 'fullscreen'
   */
  get currentDisplayMode() {
    const bridge = this._getBridge();
    return bridge?.displayMode ?? window.openai?.displayMode ?? "inline";
  }
  /**
   * Get the maximum height available for the widget.
   *
   * @returns Height in pixels, or undefined if not available
   */
  get maxHeight() {
    const bridge = this._getBridge();
    return bridge?.maxHeight ?? window.openai?.maxHeight;
  }
  /**
   * Get the safe area insets.
   *
   * @returns Safe area insets, or undefined if not available
   */
  get safeArea() {
    const bridge = this._getBridge();
    return bridge?.safeArea ?? window.openai?.safeArea;
  }
  /**
   * Get the widget view type.
   *
   * @returns 'default' or 'compact'
   */
  get view() {
    const bridge = this._getBridge();
    return bridge?.view ?? window.openai?.view ?? "default";
  }
  /**
   * Get the user agent string of the host.
   *
   * @returns User agent string, or undefined if not available
   */
  get userAgent() {
    const bridge = this._getBridge();
    return bridge?.userAgent ?? window.openai?.userAgent;
  }
  // ===========================================================================
  // Public API - Events
  // ===========================================================================
  /**
   * Subscribe to runtime events.
   *
   * @param event - Event name
   * @param handler - Event handler
   * @returns Unsubscribe function
   *
   * @example
   * ```typescript
   * const unsubscribe = runtime.on('stateUpdate', (state) => {
   *   console.log('State updated:', state);
   * });
   *
   * // Later: unsubscribe();
   * ```
   */
  on(event, handler) {
    if (!this._eventHandlers.has(event)) {
      this._eventHandlers.set(event, /* @__PURE__ */ new Set());
    }
    this._eventHandlers.get(event).add(handler);
    return () => {
      this._eventHandlers.get(event)?.delete(handler);
    };
  }
  /**
   * Subscribe to an event once.
   *
   * @param event - Event name
   * @param handler - Event handler
   */
  once(event, handler) {
    const wrapper = ((data) => {
      this._eventHandlers.get(event)?.delete(wrapper);
      handler(data);
    });
    this.on(event, wrapper);
  }
  _emit(event, data) {
    this._eventHandlers.get(event)?.forEach((handler) => {
      try {
        handler(data);
      } catch (error) {
        console.error(`Error in ${event} handler:`, error);
      }
    });
  }
};

// src/transport.ts
var PostMessageTransport = class {
  constructor(options) {
    this._nextId = 1;
    this._pending = /* @__PURE__ */ new Map();
    this._messageHandler = null;
    this._notificationHandler = null;
    this._requestHandler = null;
    this._targetWindow = options.targetWindow;
    this._targetOrigin = options.targetOrigin;
    this._timeout = options.timeout ?? 3e4;
    this._messageHandler = this._handleMessage.bind(this);
    window.addEventListener("message", this._messageHandler);
  }
  /**
   * Send a JSON-RPC request and wait for the response.
   *
   * @param method - The RPC method name (e.g., 'tools/call')
   * @param params - Optional parameters for the method
   * @returns A promise that resolves with the result or rejects on error/timeout
   */
  send(method, params) {
    const id = this._nextId++;
    const request = {
      jsonrpc: "2.0",
      id,
      method,
      params
    };
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this._pending.delete(id);
        reject(new Error(`JSON-RPC request '${method}' timed out after ${this._timeout}ms`));
      }, this._timeout);
      this._pending.set(id, { resolve, reject, timer });
      this._targetWindow.postMessage(request, this._targetOrigin);
    });
  }
  /**
   * Send a JSON-RPC response to an incoming request.
   *
   * @param id - The request ID to respond to
   * @param result - The result value (mutually exclusive with error)
   * @param error - The error value (mutually exclusive with result)
   */
  respond(id, result, error) {
    const response = {
      jsonrpc: "2.0",
      id
    };
    if (error) {
      response.error = error;
    } else {
      response.result = result;
    }
    this._targetWindow.postMessage(response, this._targetOrigin);
  }
  /**
   * Send a JSON-RPC notification (fire-and-forget, no response expected).
   *
   * @param method - The notification method name
   * @param params - Optional notification parameters
   */
  notify(method, params) {
    const notification = {
      jsonrpc: "2.0",
      method,
      params
    };
    this._targetWindow.postMessage(notification, this._targetOrigin);
  }
  /**
   * Set a handler for incoming JSON-RPC notifications (messages without an id).
   *
   * @param handler - Function called with the notification method and params
   */
  onNotification(handler) {
    this._notificationHandler = handler;
  }
  /**
   * Set a handler for incoming JSON-RPC requests (messages with an id and method).
   * The handler should return a promise that resolves with the result.
   *
   * @param handler - Async function that processes the request and returns a result
   */
  onRequest(handler) {
    this._requestHandler = handler;
  }
  /**
   * Clean up event listeners and reject all pending requests.
   */
  destroy() {
    if (this._messageHandler) {
      window.removeEventListener("message", this._messageHandler);
      this._messageHandler = null;
    }
    for (const [id, pending] of this._pending) {
      clearTimeout(pending.timer);
      pending.reject(new Error("Transport destroyed"));
      this._pending.delete(id);
    }
    this._notificationHandler = null;
    this._requestHandler = null;
  }
  // ===========================================================================
  // Private
  // ===========================================================================
  _handleMessage(event) {
    if (this._targetOrigin !== "*" && event.origin !== this._targetOrigin && event.origin !== "null") {
      return;
    }
    const data = event.data;
    if (!data || data.jsonrpc !== "2.0") {
      return;
    }
    if (typeof data.id === "number" && !data.method) {
      this._handleResponse(data);
      return;
    }
    if (typeof data.id === "number" && typeof data.method === "string") {
      this._handleIncomingRequest(data);
      return;
    }
    if (typeof data.method === "string" && data.id === void 0) {
      if (this._notificationHandler) {
        this._notificationHandler(data.method, data.params);
      }
    }
  }
  _handleResponse(response) {
    const pending = this._pending.get(response.id);
    if (!pending) {
      return;
    }
    clearTimeout(pending.timer);
    this._pending.delete(response.id);
    if (response.error) {
      pending.reject(new Error(response.error.message));
    } else {
      pending.resolve(response.result);
    }
  }
  _handleIncomingRequest(request) {
    if (!this._requestHandler) {
      this.respond(request.id, void 0, {
        code: -32601,
        message: `Method not found: ${request.method}`
      });
      return;
    }
    this._requestHandler(request.method, request.params).then((result) => {
      this.respond(request.id, result);
    }).catch((err) => {
      const message = err instanceof Error ? err.message : String(err);
      this.respond(request.id, void 0, {
        code: -32e3,
        message
      });
    });
  }
};

// src/app.ts
var App = class {
  constructor(options) {
    this._transport = null;
    this._hostContext = void 0;
    this._connected = false;
    // Lifecycle callbacks (setter-based, matching MCP Apps spec)
    this.ontoolinput = null;
    this.ontoolresult = null;
    this.ontoolcancelled = null;
    this.onhostcontextchanged = null;
    this.onteardown = null;
    this._name = options.name;
    this._version = options.version;
  }
  /**
   * Connect to the host by creating a PostMessageTransport to window.parent
   * and sending the `ui/initialize` handshake.
   *
   * Gracefully degrades: if connect() times out (2 seconds), logs a warning
   * and resolves anyway to allow standalone usage without a host.
   */
  async connect() {
    if (this._connected) {
      return;
    }
    if (typeof window === "undefined") {
      console.warn("[App] No window object -- running outside browser. Skipping connect.");
      return;
    }
    const targetOrigin = this._resolveTargetOrigin();
    this._transport = new PostMessageTransport({
      targetWindow: window.parent,
      targetOrigin,
      timeout: 3e4
    });
    this._transport.onNotification((method, params) => {
      this._handleNotification(method, params);
    });
    try {
      const result = await Promise.race([
        this._transport.send("ui/initialize", {
          name: this._name,
          version: this._version
        }),
        new Promise((resolve) => setTimeout(() => resolve(null), 2e3))
      ]);
      if (result && typeof result === "object") {
        this._hostContext = result;
      } else {
        console.warn(
          "[App] Host did not respond to ui/initialize within 2s. Running in standalone mode (no host bridge)."
        );
      }
    } catch (err) {
      console.warn("[App] ui/initialize failed:", err);
    }
    this._connected = true;
  }
  /**
   * Call a server-side MCP tool through the host bridge.
   *
   * @param params - Tool name and optional arguments
   * @returns The tool call result
   */
  async callServerTool(params) {
    if (!this._transport) {
      console.warn("[App] Not connected. Call connect() first.");
      return { content: [], isError: true };
    }
    try {
      const result = await this._transport.send("tools/call", {
        name: params.name,
        arguments: params.arguments
      });
      return result ?? { content: [] };
    } catch (err) {
      console.error("[App] callServerTool failed:", err);
      return { content: [], isError: true };
    }
  }
  /**
   * Send a follow-up message to the AI conversation.
   *
   * @param params - Message parameters
   */
  async sendMessage(params) {
    if (!this._transport) {
      console.warn("[App] Not connected. Call connect() first.");
      return;
    }
    try {
      await this._transport.send("ui/message", params);
    } catch (err) {
      console.warn("[App] sendMessage not supported by host:", err);
    }
  }
  /**
   * Open an external URL via the host.
   *
   * @param params - URL parameters
   */
  async openLink(params) {
    if (!this._transport) {
      console.warn("[App] Not connected. Call connect() first.");
      return;
    }
    try {
      await this._transport.send("ui/open-link", params);
    } catch (err) {
      console.warn("[App] openLink not supported by host:", err);
    }
  }
  /**
   * Send a log entry to the host for debugging.
   *
   * @param params - Log parameters
   */
  sendLog(params) {
    if (!this._transport) {
      return;
    }
    this._transport.notify("ui/log", params);
  }
  /**
   * Get the host context received during initialization.
   *
   * @returns The host context or undefined if not yet initialized
   */
  getHostContext() {
    return this._hostContext;
  }
  /**
   * Whether the App is connected to a host.
   */
  get connected() {
    return this._connected;
  }
  /**
   * The app name.
   */
  get name() {
    return this._name;
  }
  /**
   * The app version.
   */
  get version() {
    return this._version;
  }
  /**
   * Disconnect from the host and clean up resources.
   */
  destroy() {
    if (this.onteardown) {
      this.onteardown();
    }
    if (this._transport) {
      this._transport.destroy();
      this._transport = null;
    }
    this._connected = false;
    this._hostContext = void 0;
  }
  // ===========================================================================
  // Private
  // ===========================================================================
  _resolveTargetOrigin() {
    if (window.origin === "null" || window.location.origin === "null") {
      return "*";
    }
    try {
      if (document.referrer) {
        const url = new URL(document.referrer);
        return url.origin;
      }
    } catch {
    }
    return window.location.origin;
  }
  _handleNotification(method, params) {
    switch (method) {
      case "ui/notifications/tool-input":
      case "ui/toolInput":  // backward compat
        if (this.ontoolinput && params) {
          this.ontoolinput(params);
        }
        break;
      case "ui/notifications/tool-result":
      case "ui/toolResult":  // backward compat
        if (this.ontoolresult && params) {
          this.ontoolresult(params);
        }
        break;
      case "ui/notifications/tool-cancelled":
      case "ui/toolCancelled":  // backward compat
        if (this.ontoolcancelled) {
          this.ontoolcancelled();
        }
        break;
      case "ui/notifications/context-changed":
      case "ui/hostContextChanged":  // backward compat
        if (params) {
          this._hostContext = params;
          if (this.onhostcontextchanged) {
            this.onhostcontextchanged(this._hostContext);
          }
        }
        break;
      case "ui/resource-teardown":
      case "ui/teardown":  // backward compat
        this.destroy();
        break;
    }
  }
};

// src/app-bridge.ts
var AppBridge = class {
  constructor(options) {
    this._transport = null;
    this._initialized = false;
    this._iframe = options.iframe;
    this._toolCallHandler = options.toolCallHandler;
    this._origin = options.origin ?? window.location.origin;
    this._hostContext = options.hostContext ?? {};
  }
  /**
   * Start listening for messages from the widget iframe.
   *
   * Sets up the PostMessageTransport and registers request handlers for
   * the MCP Apps protocol methods (ui/initialize, tools/call, etc.).
   */
  initialize() {
    if (this._initialized) {
      return;
    }
    const contentWindow = this._iframe.contentWindow;
    if (!contentWindow) {
      console.error("[AppBridge] iframe has no contentWindow. Is it attached to the DOM?");
      return;
    }
    this._transport = new PostMessageTransport({
      targetWindow: contentWindow,
      targetOrigin: this._origin
    });
    this._transport.onRequest(async (method, params) => {
      return this._handleRequest(method, params);
    });
    this._initialized = true;
  }
  /**
   * Send a tool input notification to the widget.
   *
   * @param params - The tool input arguments
   */
  sendToolInput(params) {
    if (!this._transport) {
      console.warn("[AppBridge] Not initialized.");
      return;
    }
    this._transport.notify("ui/notifications/tool-input", params);
  }
  /**
   * Send a tool result notification to the widget.
   *
   * @param result - The tool call result
   */
  sendToolResult(result) {
    if (!this._transport) {
      console.warn("[AppBridge] Not initialized.");
      return;
    }
    this._transport.notify("ui/notifications/tool-result", result);
  }
  /**
   * Send a host context changed notification to the widget.
   *
   * @param ctx - The updated host context
   */
  sendHostContextChanged(ctx) {
    this._hostContext = ctx;
    if (!this._transport) {
      console.warn("[AppBridge] Not initialized.");
      return;
    }
    this._transport.notify("ui/notifications/context-changed", ctx);
  }
  /**
   * Send a teardown notification to the widget.
   */
  sendTeardown() {
    if (!this._transport) {
      return;
    }
    this._transport.notify("ui/resource-teardown");
  }
  /**
   * Whether the bridge has been initialized.
   */
  get initialized() {
    return this._initialized;
  }
  /**
   * The current host context.
   */
  get hostContext() {
    return this._hostContext;
  }
  /**
   * Clean up event listeners and transport.
   */
  destroy() {
    this.sendTeardown();
    if (this._transport) {
      this._transport.destroy();
      this._transport = null;
    }
    this._initialized = false;
  }
  // ===========================================================================
  // Private
  // ===========================================================================
  async _handleRequest(method, params) {
    switch (method) {
      case "ui/initialize": {
        const ctx = this._hostContext;
        // Send initialized notification after responding
        setTimeout(() => {
          if (this._transport) {
            this._transport.notify("ui/notifications/initialized", {});
          }
        }, 0);
        return ctx;
      }
      case "tools/call": {
        const name = params?.name;
        const args = params?.arguments;
        if (!name) {
          throw new Error('tools/call requires a "name" parameter');
        }
        return await this._toolCallHandler(name, args);
      }
      case "ui/message":
      case "ui/sendMessage":  // backward compat
        console.log("[AppBridge] Widget sent message:", params);
        return {};
      case "ui/open-link":
      case "ui/openLink": {  // backward compat
        const url = params?.url;
        if (url) {
          window.open(url, "_blank", "noopener,noreferrer");
        }
        return {};
      }
      default:
        throw new Error(`Unknown method: ${method}`);
    }
  }
};

// src/compat.ts
var deprecationWarned = false;
function unwrapToolResult(result) {
  if (result.isError) {
    const errorText = result.content?.[0]?.text ?? "Unknown error";
    return { success: false, error: errorText };
  }
  const textItem = result.content?.find(
    (c) => c.type === "text" && typeof c.text === "string"
  );
  if (textItem) {
    try {
      return JSON.parse(textItem.text);
    } catch {
      return textItem.text;
    }
  }
  return result.content ?? result.structuredContent;
}
function installCompat(app) {
  if (typeof window === "undefined") {
    return;
  }
  const warnDeprecation = () => {
    if (!deprecationWarned) {
      deprecationWarned = true;
      console.warn(
        "[widget-runtime] window.mcpBridge is deprecated. Use `import { App } from 'widget-runtime.js'` instead."
      );
    }
  };
  const getCtx = () => app.getHostContext() ?? {};
  const mcpBridge = {
    callTool: async (name, args) => {
      warnDeprecation();
      const result = await app.callServerTool({ name, arguments: args });
      return unwrapToolResult(result);
    },
    getState: () => {
      warnDeprecation();
      return {};
    },
    setState: (_state) => {
      warnDeprecation();
    },
    sendMessage: (message) => {
      warnDeprecation();
      app.sendMessage({ message });
    },
    openExternal: (url) => {
      warnDeprecation();
      app.openLink({ url });
    },
    openLink: (url) => {
      warnDeprecation();
      app.openLink({ url });
    },
    get theme() {
      return getCtx().theme;
    },
    get locale() {
      return getCtx().locale;
    },
    get displayMode() {
      return getCtx().displayMode;
    }
  };
  window.mcpBridge = mcpBridge;
  window.openai = {
    callTool: mcpBridge.callTool,
    setWidgetState: mcpBridge.setState,
    sendFollowUpMessage: (options) => {
      warnDeprecation();
      app.sendMessage({ message: options.prompt });
    },
    openExternal: (options) => {
      warnDeprecation();
      app.openLink({ url: options.href });
    }
  };
}

// src/types.ts
var SET_GLOBALS_EVENT_TYPE = "openai:set_globals";

// src/utils.ts
function detectHost() {
  if (typeof window === "undefined") {
    return "unknown";
  }
  if (window.openai) {
    return "chatgpt";
  }
  if (window.mcpBridge) {
    if (window.mcpBridge.sendIntent) {
      return "mcp-ui";
    }
    return "mcp-apps";
  }
  return "unknown";
}
function isWidget() {
  return detectHost() !== "unknown";
}
function isChatGPT() {
  return detectHost() === "chatgpt";
}
function isMcpApps() {
  return detectHost() === "mcp-apps";
}
function isMcpUI() {
  return detectHost() === "mcp-ui";
}
function getBridge() {
  if (typeof window === "undefined") {
    return void 0;
  }
  return window.mcpBridge;
}
async function waitForBridge(timeout = 5e3) {
  if (typeof window === "undefined") {
    return null;
  }
  if (window.mcpBridge) {
    return window.mcpBridge;
  }
  return new Promise((resolve) => {
    const startTime = Date.now();
    const check = () => {
      if (window.mcpBridge) {
        resolve(window.mcpBridge);
        return;
      }
      if (Date.now() - startTime > timeout) {
        resolve(null);
        return;
      }
      requestAnimationFrame(check);
    };
    requestAnimationFrame(check);
  });
}
function createMessageId() {
  return `msg_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`;
}
function serializeState(state, maxSize = 100 * 1024) {
  try {
    const json = JSON.stringify(state);
    if (json.length > maxSize) {
      console.warn(`State too large: ${json.length} bytes (max: ${maxSize})`);
      return null;
    }
    return json;
  } catch (error) {
    console.error("Failed to serialize state:", error);
    return null;
  }
}
function mergeState(target, source) {
  const result = { ...target };
  for (const key of Object.keys(source)) {
    const sourceValue = source[key];
    const targetValue = target[key];
    if (sourceValue !== null && typeof sourceValue === "object" && !Array.isArray(sourceValue) && targetValue !== null && typeof targetValue === "object" && !Array.isArray(targetValue)) {
      result[key] = mergeState(
        targetValue,
        sourceValue
      );
    } else {
      result[key] = sourceValue;
    }
  }
  return result;
}
function debounce(fn, delay) {
  let timeoutId = null;
  return (...args) => {
    if (timeoutId) {
      clearTimeout(timeoutId);
    }
    timeoutId = setTimeout(() => {
      fn(...args);
      timeoutId = null;
    }, delay);
  };
}
function throttle(fn, limit) {
  let inThrottle = false;
  return (...args) => {
    if (!inThrottle) {
      fn(...args);
      inThrottle = true;
      setTimeout(() => {
        inThrottle = false;
      }, limit);
    }
  };
}
function log(level, message, data) {
  const host = detectHost();
  const prefix = `[Widget:${host}]`;
  const logFn = console[level] || console.log;
  if (data !== void 0) {
    logFn(prefix, message, data);
  } else {
    logFn(prefix, message);
  }
}
export {
  App,
  AppBridge,
  PostMessageTransport,
  SET_GLOBALS_EVENT_TYPE,
  WidgetRuntime,
  createMessageId,
  debounce,
  WidgetRuntime as default,
  detectHost,
  getBridge,
  installCompat,
  isChatGPT,
  isMcpApps,
  isMcpUI,
  isWidget,
  log,
  mergeState,
  serializeState,
  throttle,
  waitForBridge
};
