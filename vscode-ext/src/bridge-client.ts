import type { AckResponse, BridgeStatus } from "./types";

/** HTTP client for the ingenierIA TUI Bridge API. */
export class BridgeClient {
  private readonly baseUrl: string;

  constructor(port: number) {
    this.baseUrl = `http://127.0.0.1:${port}`;
  }

  async getStatus(): Promise<BridgeStatus> {
    return this.get<BridgeStatus>("/api/status");
  }

  async sendContext(
    kind: string,
    path?: string,
    content?: string,
  ): Promise<AckResponse> {
    return this.post<AckResponse>("/api/context", { kind, path, content });
  }

  async approveToolCall(toolCallId: string): Promise<AckResponse> {
    return this.post<AckResponse>("/api/tool_approval", {
      tool_call_id: toolCallId,
      approved: true,
    });
  }

  async denyToolCall(toolCallId: string): Promise<AckResponse> {
    return this.post<AckResponse>("/api/tool_approval", {
      tool_call_id: toolCallId,
      approved: false,
    });
  }

  async ping(): Promise<boolean> {
    try {
      await this.getStatus();
      return true;
    } catch {
      return false;
    }
  }

  private async get<T>(path: string): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`);
    if (!res.ok) throw new Error(`Bridge GET ${path}: ${res.status}`);
    return res.json() as Promise<T>;
  }

  private async post<T>(path: string, body: unknown): Promise<T> {
    const res = await fetch(`${this.baseUrl}${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) throw new Error(`Bridge POST ${path}: ${res.status}`);
    return res.json() as Promise<T>;
  }
}
