import { invoke } from "@tauri-apps/api/core";
import type { InstanceInfo } from "./types";

// ---------------------------------------------------------------------------
// Instance management
// ---------------------------------------------------------------------------

export async function listInstances(): Promise<InstanceInfo[]> {
  return invoke<InstanceInfo[]>("list_instances");
}

export async function spawnInstance(): Promise<InstanceInfo> {
  return invoke<InstanceInfo>("spawn_instance");
}

export async function killInstance(id: string): Promise<void> {
  return invoke<void>("kill_instance", { id });
}

export async function killAllInstances(): Promise<void> {
  return invoke<void>("kill_all_instances");
}

// ---------------------------------------------------------------------------
// Browser window
// ---------------------------------------------------------------------------

export async function openBrowserWindow(
  instanceId: string,
  url?: string
): Promise<void> {
  return invoke<void>("open_browser_window", { instanceId, url });
}

export async function navigateBrowserWindow(
  instanceId: string,
  url: string
): Promise<void> {
  return invoke<void>("navigate_browser_window", { instanceId, url });
}

export async function closeBrowserWindow(instanceId: string): Promise<void> {
  return invoke<void>("close_browser_window", { instanceId });
}

// ---------------------------------------------------------------------------
// Challenge
// ---------------------------------------------------------------------------

export async function openChallengeWindow(
  url: string,
  title?: string
): Promise<string> {
  return invoke<string>("open_challenge_window", { url, title });
}

export async function submitChallengeResolution(
  challengeUrl: string,
  cookies: string,
  headers: Record<string, string> = {}
): Promise<void> {
  return invoke<void>("submit_challenge_resolution", {
    challengeUrl,
    cookies,
    headers,
  });
}

export async function cancelChallenge(challengeUrl: string): Promise<void> {
  return invoke<void>("cancel_challenge", { challengeUrl });
}
