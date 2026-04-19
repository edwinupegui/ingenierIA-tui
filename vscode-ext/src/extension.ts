import * as vscode from "vscode";
import { BridgeClient } from "./bridge-client";
import { StatusBar } from "./status-bar";
import { ToolApprovalNotifier } from "./tool-approval";

let pollTimer: ReturnType<typeof setInterval> | undefined;
let client: BridgeClient | undefined;
let statusBar: StatusBar | undefined;
let notifier: ToolApprovalNotifier | undefined;

export function activate(context: vscode.ExtensionContext): void {
  statusBar = new StatusBar();
  context.subscriptions.push(statusBar);

  context.subscriptions.push(
    vscode.commands.registerCommand("ingenieria.connect", connect),
    vscode.commands.registerCommand("ingenieria.disconnect", disconnect),
    vscode.commands.registerCommand("ingenieria.sendContext", sendActiveFile),
  );

  // Handle file open requests from TUI via a URI handler.
  context.subscriptions.push(
    vscode.window.registerUriHandler({
      handleUri(uri: vscode.Uri) {
        if (uri.path.startsWith("/open")) {
          const params = new URLSearchParams(uri.query);
          const file = params.get("path");
          const line = Number(params.get("line") ?? "1");
          const col = Number(params.get("column") ?? "1");
          if (file) openFileAt(file, line, col);
        }
      },
    }),
  );

  const config = vscode.workspace.getConfiguration("ingenieria.bridge");
  if (config.get<boolean>("autoConnect", true)) {
    connect();
  }
}

export function deactivate(): void {
  disconnect();
}

function connect(): void {
  disconnect();

  const config = vscode.workspace.getConfiguration("ingenieria.bridge");
  const port = config.get<number>("port", 19542);
  const intervalMs = config.get<number>("pollIntervalMs", 2000);

  client = new BridgeClient(port);
  notifier = new ToolApprovalNotifier(client);

  pollTimer = setInterval(() => poll(), intervalMs);
  poll();
}

function disconnect(): void {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = undefined;
  }
  notifier?.dispose();
  notifier = undefined;
  client = undefined;
  statusBar?.setDisconnected();
}

async function poll(): Promise<void> {
  if (!client || !statusBar) return;
  try {
    const status = await client.getStatus();
    statusBar.update(status);
    await notifier?.handlePending(status.pending_approvals);
  } catch {
    statusBar.setDisconnected();
  }
}

async function sendActiveFile(): Promise<void> {
  if (!client) {
    vscode.window.showWarningMessage("ingenierIA: not connected to TUI bridge.");
    return;
  }

  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    vscode.window.showInformationMessage("ingenierIA: no active editor.");
    return;
  }

  const doc = editor.document;
  await client.sendContext("active_file", doc.uri.fsPath, doc.getText());
  vscode.window.showInformationMessage(
    `ingenierIA: sent ${doc.fileName} as context.`,
  );
}

async function openFileAt(
  filePath: string,
  line: number,
  column: number,
): Promise<void> {
  const uri = vscode.Uri.file(filePath);
  const doc = await vscode.workspace.openTextDocument(uri);
  const pos = new vscode.Position(Math.max(0, line - 1), Math.max(0, column - 1));
  await vscode.window.showTextDocument(doc, {
    selection: new vscode.Range(pos, pos),
  });
}
