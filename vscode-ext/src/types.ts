/** Mirrors BridgeStatus from protocol.rs */
export interface BridgeStatus {
  version: string;
  app_screen: string;
  chat_status: string;
  diagnostics_count: number;
  monitors_active: number;
  agents_active: number;
  pending_approvals: PendingApproval[];
}

/** Mirrors PendingApprovalItem from protocol.rs */
export interface PendingApproval {
  tool_call_id: string;
  tool_name: string;
  arguments: string;
  permission: string;
  reason: string | null;
}

/** Mirrors ContextUpdate from protocol.rs */
export interface ContextUpdate {
  kind: string;
  path?: string;
  content?: string;
}

/** Mirrors AckResponse from protocol.rs */
export interface AckResponse {
  ok: boolean;
  message?: string;
}
