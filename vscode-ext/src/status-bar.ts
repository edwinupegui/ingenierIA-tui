import * as vscode from "vscode";
import type { BridgeStatus } from "./types";

const ICON_CONNECTED = "$(plug)";
const ICON_DISCONNECTED = "$(debug-disconnect)";

/** Manages the ingenierIA status bar item. */
export class StatusBar implements vscode.Disposable {
  private readonly item: vscode.StatusBarItem;

  constructor() {
    this.item = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      50,
    );
    this.item.command = "ingenieria.connect";
    this.setDisconnected();
    this.item.show();
  }

  update(status: BridgeStatus): void {
    const pending = status.pending_approvals.length;
    const pendingLabel = pending > 0 ? ` | ${pending} pending` : "";
    this.item.text = `${ICON_CONNECTED} ingenierIA v${pendingLabel}`;
    this.item.tooltip = [
      `Screen: ${status.app_screen}`,
      `Chat: ${status.chat_status}`,
      `Diagnostics: ${status.diagnostics_count}`,
      `Monitors: ${status.monitors_active}`,
      `Agents: ${status.agents_active}`,
      pending > 0 ? `Pending approvals: ${pending}` : "",
    ]
      .filter(Boolean)
      .join("\n");
    this.item.backgroundColor =
      pending > 0
        ? new vscode.ThemeColor("statusBarItem.warningBackground")
        : undefined;
  }

  setDisconnected(): void {
    this.item.text = `${ICON_DISCONNECTED} ingenierIA`;
    this.item.tooltip = "Click to connect to ingenierIA TUI";
    this.item.backgroundColor = undefined;
  }

  dispose(): void {
    this.item.dispose();
  }
}
