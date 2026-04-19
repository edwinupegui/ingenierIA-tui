import * as vscode from "vscode";
import type { BridgeClient } from "./bridge-client";
import type { PendingApproval } from "./types";

/**
 * Tracks pending tool approvals and shows VS Code notifications.
 *
 * Keeps a set of already-notified tool_call_ids to avoid duplicate popups.
 */
export class ToolApprovalNotifier implements vscode.Disposable {
  private readonly notified = new Set<string>();
  private readonly client: BridgeClient;

  constructor(client: BridgeClient) {
    this.client = client;
  }

  /** Called on each poll cycle with fresh pending approvals. */
  async handlePending(approvals: PendingApproval[]): Promise<void> {
    // Clean up stale entries.
    const currentIds = new Set(approvals.map((a) => a.tool_call_id));
    for (const id of this.notified) {
      if (!currentIds.has(id)) this.notified.delete(id);
    }

    for (const approval of approvals) {
      if (this.notified.has(approval.tool_call_id)) continue;
      this.notified.add(approval.tool_call_id);
      this.showNotification(approval);
    }
  }

  private async showNotification(approval: PendingApproval): Promise<void> {
    const detail = approval.reason ?? approval.permission;
    const label = `${approval.tool_name}(${truncate(approval.arguments, 60)})`;

    const choice = await vscode.window.showWarningMessage(
      `ingenierIA: tool requires approval — ${label}`,
      { detail, modal: false },
      "Approve",
      "Deny",
    );

    if (choice === "Approve") {
      await this.client.approveToolCall(approval.tool_call_id);
    } else if (choice === "Deny") {
      await this.client.denyToolCall(approval.tool_call_id);
    }
    // If dismissed, do nothing — the TUI still shows the modal.
  }

  dispose(): void {
    this.notified.clear();
  }
}

function truncate(s: string, max: number): string {
  return s.length <= max ? s : s.slice(0, max - 1) + "…";
}
