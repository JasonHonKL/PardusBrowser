import type { InstanceInfo, LogEntry } from "./types";
import * as api from "./api";

export class InstanceManager {
  private instances: InstanceInfo[] = [];
  private tableBody: HTMLElement;
  private noInstances: HTMLElement;
  private onLog: (entry: LogEntry) => void;

  constructor(
    tableBody: HTMLElement,
    noInstances: HTMLElement,
    onLog: (entry: LogEntry) => void
  ) {
    this.tableBody = tableBody;
    this.noInstances = noInstances;
    this.onLog = onLog;
  }

  async refresh(): Promise<void> {
    try {
      this.instances = await api.listInstances();
    } catch {
      this.instances = [];
    }
    this.render();
  }

  async spawn(): Promise<void> {
    try {
      const inst = await api.spawnInstance();
      this.instances.push(inst);
      this.render();
      this.onLog({ level: "info", message: `Instance ${inst.id} spawned on port ${inst.port}`, timestamp: ts() });
      // Auto-open browser window
      await this.openBrowser(inst.id);
    } catch (e) {
      this.onLog({ level: "error", message: `Spawn failed: ${e}`, timestamp: ts() });
    }
  }

  async kill(id: string): Promise<void> {
    try {
      await api.killInstance(id);
      this.instances = this.instances.filter((i) => i.id !== id);
      this.render();
      this.onLog({ level: "info", message: `Instance ${id} killed`, timestamp: ts() });
    } catch (e) {
      this.onLog({ level: "error", message: `Kill failed: ${e}`, timestamp: ts() });
    }
  }

  async killAll(): Promise<void> {
    try {
      await api.killAllInstances();
      this.instances = [];
      this.render();
      this.onLog({ level: "info", message: "All instances killed", timestamp: ts() });
    } catch (e) {
      this.onLog({ level: "error", message: `Kill all failed: ${e}`, timestamp: ts() });
    }
  }

  async openBrowser(id: string): Promise<void> {
    try {
      await api.openBrowserWindow(id);
      this.onLog({ level: "info", message: `Browser view opened for ${id}`, timestamp: ts() });
      await this.refresh();
    } catch (e) {
      this.onLog({ level: "error", message: `Open browser failed: ${e}`, timestamp: ts() });
    }
  }

  async closeBrowser(id: string): Promise<void> {
    try {
      await api.closeBrowserWindow(id);
      this.onLog({ level: "info", message: `Browser view closed for ${id}`, timestamp: ts() });
      await this.refresh();
    } catch (e) {
      this.onLog({ level: "error", message: `Close browser failed: ${e}`, timestamp: ts() });
    }
  }

  async navigateInBrowser(id: string, url: string): Promise<void> {
    try {
      await api.navigateBrowserWindow(id, url);
      this.onLog({ level: "info", message: `Navigating ${id} to ${url}`, timestamp: ts() });
    } catch (e) {
      this.onLog({ level: "error", message: `Navigate failed: ${e}`, timestamp: ts() });
    }
  }

  private render(): void {
    this.tableBody.innerHTML = "";

    if (this.instances.length === 0) {
      this.noInstances.style.display = "block";
      return;
    }
    this.noInstances.style.display = "none";

    for (const inst of this.instances) {
      const tr = document.createElement("tr");

      const statusClass = inst.agent_status === "waiting-challenge" ? "challenge" : inst.agent_status;
      const browserBtn = inst.browser_window_open
        ? `<button class="btn-sm" data-close-browser="${inst.id}">Close View</button>`
        : `<button class="btn-sm" data-open-browser="${inst.id}">Open View</button>`;

      tr.innerHTML = `
        <td>${inst.id}</td>
        <td>${inst.port}</td>
        <td class="mono">${inst.ws_url}</td>
        <td><span class="status ${statusClass}">${inst.agent_status}</span></td>
        <td>
          <div class="url-row">
            <input type="text" class="url-input" placeholder="https://..." value="${inst.current_url || ""}" data-url-input="${inst.id}" />
            <button class="btn-sm" data-navigate="${inst.id}">Go</button>
          </div>
        </td>
        <td>
          ${browserBtn}
          <button class="btn-sm" data-copy="${inst.ws_url}">Copy WS</button>
          <button class="btn-danger btn-sm" data-kill="${inst.id}">Kill</button>
        </td>
      `;

      tr.querySelector(`[data-kill="${inst.id}"]`)?.addEventListener("click", () => this.kill(inst.id));
      tr.querySelector(`[data-copy="${inst.ws_url}"]`)?.addEventListener("click", () => {
        navigator.clipboard.writeText(inst.ws_url);
      });
      tr.querySelector(`[data-open-browser="${inst.id}"]`)?.addEventListener("click", () => this.openBrowser(inst.id));
      tr.querySelector(`[data-close-browser="${inst.id}"]`)?.addEventListener("click", () => this.closeBrowser(inst.id));
      tr.querySelector(`[data-navigate="${inst.id}"]`)?.addEventListener("click", () => {
        const input = tr.querySelector(`[data-url-input="${inst.id}"]`) as HTMLInputElement;
        if (input?.value) this.navigateInBrowser(inst.id, input.value);
      });

      const urlInput = tr.querySelector(`[data-url-input="${inst.id}"]`);
      urlInput?.addEventListener("keydown", (e) => {
        if ((e as KeyboardEvent).key === "Enter") {
          const val = (e.target as HTMLInputElement).value;
          if (val) this.navigateInBrowser(inst.id, val);
        }
      });

      this.tableBody.appendChild(tr);
    }
  }

  destroy(): void {}
}

function ts(): string {
  return new Date().toISOString().slice(11, 19);
}
