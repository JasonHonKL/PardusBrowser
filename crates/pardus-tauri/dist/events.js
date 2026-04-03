import { listen } from "@tauri-apps/api/event";
export function onChallengeDetected(callback) {
    return listen("challenge-detected", (event) => {
        callback(event.payload);
    });
}
export function onChallengeSolved(callback) {
    return listen("challenge-solved", (event) => {
        callback(event.payload);
    });
}
export function onChallengeFailed(callback) {
    return listen("challenge-failed", (event) => {
        callback(event.payload.challenge_url, event.payload.reason);
    });
}
export function createLogger(container) {
    return (entry) => {
        const div = document.createElement("div");
        div.className = `log-entry ${entry.level}`;
        div.textContent = `[${entry.timestamp}] ${entry.message}`;
        container.appendChild(div);
        container.scrollTop = container.scrollHeight;
    };
}
export function log(level, message) {
    return {
        level,
        message,
        timestamp: new Date().toISOString().slice(11, 19),
    };
}
