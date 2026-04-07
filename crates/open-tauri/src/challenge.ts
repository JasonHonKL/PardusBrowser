import type { LogEntry } from "./types";
import * as api from "./api";
import { log } from "./events";

export interface ActiveChallenge {
  url: string;
  kinds: string[];
  score: number;
  startTime: number;
}

export class ChallengeManager {
  private activeChallenges: Map<string, ActiveChallenge> = new Map();
  private panelBody: HTMLElement;
  private panelEl: HTMLElement;
  private onLog: (entry: LogEntry) => void;

  constructor(
    panelEl: HTMLElement,
    panelBody: HTMLElement,
    onLog: (entry: LogEntry) => void
  ) {
    this.panelEl = panelEl;
    this.panelBody = panelBody;
    this.onLog = onLog;
  }

  handleDetected(info: { url: string; kinds: string[]; risk_score: number }): void {
    if (this.activeChallenges.has(info.url)) {
      return;
    }

    const challenge: ActiveChallenge = {
      url: info.url,
      kinds: info.kinds,
      score: info.risk_score,
      startTime: Date.now(),
    };
    this.activeChallenges.set(info.url, challenge);

    this.onLog(log("warn", `Challenge detected: ${info.kinds.join(", ")} (score: ${info.risk_score}) — ${info.url}`));

    // The Rust backend automatically opens a browser window with the challenge banner.
    // No need to open a separate challenge window here.
    this.render();
  }

  handleSolved(info: { url: string }): void {
    this.activeChallenges.delete(info.url);
    this.onLog(log("info", `Challenge resolved: ${info.url}`));
    this.render();
  }

  handleFailed(url: string, reason: string): void {
    this.activeChallenges.delete(url);
    this.onLog(log("error", `Challenge failed: ${reason} — ${url}`));
    this.render();
  }

  async submitCookies(url: string, cookies: string): Promise<void> {
    try {
      await api.submitChallengeResolution(url, cookies);
      this.activeChallenges.delete(url);
      this.onLog(log("info", `Challenge resolved for ${url}`));
      this.render();
    } catch (e) {
      this.onLog(log("error", `Failed to submit resolution: ${e}`));
    }
  }

  async cancel(url: string): Promise<void> {
    try {
      await api.cancelChallenge(url);
      this.activeChallenges.delete(url);
      this.onLog(log("info", `Challenge cancelled for ${url}`));
      this.render();
    } catch (e) {
      this.onLog(log("error", `Failed to cancel: ${e}`));
    }
  }

  private render(): void {
    this.panelBody.innerHTML = "";

    if (this.activeChallenges.size === 0) {
      this.panelEl.style.display = "none";
      return;
    }
    this.panelEl.style.display = "block";

    for (const [url, challenge] of this.activeChallenges) {
      const elapsed = Math.round((Date.now() - challenge.startTime) / 1000);
      const div = document.createElement("div");
      div.className = "challenge-item";
      div.innerHTML = `
        <div class="challenge-header">
          <span class="challenge-kinds">${challenge.kinds.join(", ")}</span>
          <span class="challenge-score">score: ${challenge.score}</span>
          <span class="challenge-elapsed">${elapsed}s</span>
        </div>
        <div class="challenge-url">${url}</div>
        <div class="challenge-actions">
          <span class="challenge-hint">Solve the CAPTCHA in the browser window — cookies will sync automatically</span>
          <button class="btn-sm" data-cookies="${url}">Paste Cookies</button>
          <button class="btn-sm" data-cancel="${url}">Cancel</button>
        </div>
      `;
      div.querySelector(`[data-cookies="${url}"]`)?.addEventListener("click", () => {
        const cookies = prompt("Paste the Cookie header value obtained after solving:");
        if (cookies) {
          this.submitCookies(url, cookies);
        }
      });
      div.querySelector(`[data-cancel="${url}"]`)?.addEventListener("click", () => {
        this.cancel(url);
      });
      this.panelBody.appendChild(div);
    }
  }
}
