import { invoke } from "@tauri-apps/api/core";
export async function listInstances() {
    return invoke("list_instances");
}
export async function spawnInstance() {
    return invoke("spawn_instance");
}
export async function killInstance(id) {
    return invoke("kill_instance", { id });
}
export async function killAllInstances() {
    return invoke("kill_all_instances");
}
export async function openChallengeWindow(url, title) {
    return invoke("open_challenge_window", { url, title });
}
export async function submitChallengeResolution(challengeUrl, cookies, headers = {}) {
    return invoke("submit_challenge_resolution", {
        challengeUrl,
        cookies,
        headers,
    });
}
export async function cancelChallenge(challengeUrl) {
    return invoke("cancel_challenge", { challengeUrl });
}
