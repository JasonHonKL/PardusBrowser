import { InstanceManager } from "./instances";
import { ChallengeManager } from "./challenge";
import {
  onChallengeDetected,
  onChallengeSolved,
  onChallengeFailed,
  createLogger,
  log,
} from "./events";

function init(): void {
  const tableBody = document.getElementById("instance-table") as HTMLTableSectionElement;
  const noInstances = document.getElementById("no-instances")!;
  const logContainer = document.getElementById("log-entries")!;
  const challengePanel = document.getElementById("challenge-panel")!;
  const challengeBody = document.getElementById("challenge-items")!;

  const logger = createLogger(logContainer);

  const instanceManager = new InstanceManager(tableBody, noInstances, logger);
  const challengeManager = new ChallengeManager(challengePanel, challengeBody, logger);

  document.getElementById("btn-spawn")?.addEventListener("click", () => instanceManager.spawn());
  document.getElementById("btn-kill-all")?.addEventListener("click", () => instanceManager.killAll());

  // Challenge events from the Rust backend
  onChallengeDetected((info) => {
    challengeManager.handleDetected(info);
  });
  onChallengeSolved((info) => {
    challengeManager.handleSolved(info);
  });
  onChallengeFailed((url, reason) => {
    challengeManager.handleFailed(url, reason);
  });

  instanceManager.refresh();
  logger(log("info", "Open Browser app initialized"));
}

document.addEventListener("DOMContentLoaded", init);
