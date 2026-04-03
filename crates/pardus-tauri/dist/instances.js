import * as api from "./api";
export class InstanceManager {
    instances = [];
    tableBody;
    noInstances;
    onLog;
    challengeListenerCleanup = null;
    constructor(tableBody, noInstances, onLog) {
        this.tableBody = tableBody;
        this.noInstances = noInstances;
        this.onLog = onLog;
    }
    async refresh() {
        try {
            this.instances = await api.listInstances();
        }
        catch {
            this.instances = [];
        }
        this.render();
    }
    async spawn() {
        try {
            const inst = await api.spawnInstance();
            this.instances.push(inst);
            this.render();
            this.onLog({ level: "info", message: `Instance ${inst.id} spawned on port ${inst.port}`, timestamp: ts() });
        }
        catch (e) {
            this.onLog({ level: "error", message: `Spawn failed: ${e}`, timestamp: ts() });
        }
    }
    async kill(id) {
        try {
            await api.killInstance(id);
            this.instances = this.instances.filter((i) => i.id !== id);
            this.render();
            this.onLog({ level: "info", message: `Instance ${id} killed`, timestamp: ts() });
        }
        catch (e) {
            this.onLog({ level: "error", message: `Kill failed: ${e}`, timestamp: ts() });
        }
    }
    async killAll() {
        try {
            await api.killAllInstances();
            this.instances = [];
            this.render();
            this.onLog({ level: "info", message: "All instances killed", timestamp: ts() });
        }
        catch (e) {
            this.onLog({ level: "error", message: `Kill all failed: ${e}`, timestamp: ts() });
        }
    }
    render() {
        this.tableBody.innerHTML = "";
        if (this.instances.length === 0) {
            this.noInstances.style.display = "block";
            return;
        }
        this.noInstances.style.display = "none";
        for (const inst of this.instances) {
            const tr = document.createElement("tr");
            tr.innerHTML = `
        <td>${inst.id}</td>
        <td>${inst.port}</td>
        <td class="mono">${inst.ws_url}</td>
        <td><span class="status running">running</span></td>
        <td>
          <button class="btn-danger btn-sm" data-kill="${inst.id}">Kill</button>
          <button class="btn-sm" data-copy="${inst.ws_url}">Copy WS</button>
        </td>
      `;
            tr.querySelector(`[data-kill="${inst.id}"]`)?.addEventListener("click", () => this.kill(inst.id));
            tr.querySelector(`[data-copy="${inst.ws_url}"]`)?.addEventListener("click", () => {
                navigator.clipboard.writeText(inst.ws_url);
            });
            this.tableBody.appendChild(tr);
        }
    }
    destroy() {
        this.challengeListenerCleanup?.();
    }
}
function ts() {
    return new Date().toISOString().slice(11, 19);
}
