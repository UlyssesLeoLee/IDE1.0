/**
 * IDE1.0 Web Agent Router — TypeScript SDK
 * High-concurrency Agent Collaboration Client with Fencing Tokens, Leases, and AST Relocation.
 */

export interface SemanticUnit {
  workspace_id: string;
  file_id: string;
  language: string;
  qualified_symbol_path: string;
  node_kind: string;
  signature_hash: string;
  base_content_hash: string;
  node_fingerprint: string;
  start_byte: number;
  end_byte: number;
  start_line: number;
  end_line: number;
}

export interface TaskClaimResponse {
  task_id: string;
  job_id: string;
  workspace_id: string;
  file_id: string;
  symbol_path: string;
  base_revision: number;
  fencing_token: number;
  lease_expires_at: string;
  expected_fingerprint?: string;
}

export interface PatchCommitResponse {
  commit_id: string;
  file_id: string;
  previous_revision: number;
  new_revision: number;
  relocated: boolean;
  status: string;
}

export class IDEAgentClient {
  private baseUrl: string;
  private token?: string;

  constructor(baseUrl: string = "http://localhost:3000/api/v1", token?: string) {
    this.baseUrl = baseUrl.replace(/\/+$/, "");
    this.token = token;
  }

  private headers(idempotencyKey?: string): HeadersInit {
    const h: Record<string, string> = {
      "Content-Type": "application/json",
      "User-Agent": "IDE1.0-TypeScript-Agent-SDK/2.0",
    };
    if (this.token) {
      h["Authorization"] = `Bearer ${this.token}`;
    }
    if (idempotencyKey) {
      h["Idempotency-Key"] = idempotencyKey;
    }
    return h;
  }

  async dispatchJob(
    workspaceId: string,
    goal: string,
    targetFiles: string[] = [],
    priority: number = 100
  ): Promise<{ job_id: string; task_count: number; status: string }> {
    const res = await fetch(`${this.baseUrl}/jobs/dispatch`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify({ workspace_id: workspaceId, goal, target_files: targetFiles, priority }),
    });
    if (!res.ok) throw new Error(`Dispatch failed: ${res.statusText}`);
    return res.json();
  }

  async claimTask(workerId: string, workspaceId: string): Promise<TaskClaimResponse | null> {
    const res = await fetch(`${this.baseUrl}/tasks/claim`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify({ worker_id: workerId, workspace_id: workspaceId }),
    });
    if (res.status === 204) return null;
    if (!res.ok) throw new Error(`Claim failed: ${res.statusText}`);
    return res.json();
  }

  async heartbeat(taskId: string, workerId: string, fencingToken: number): Promise<{ status: string; lease_expires_at: string }> {
    const res = await fetch(`${this.baseUrl}/tasks/${taskId}/heartbeat`, {
      method: "POST",
      headers: this.headers(),
      body: JSON.stringify({ worker_id: workerId, fencing_token: fencingToken }),
    });
    if (!res.ok) throw new Error(`Heartbeat failed: ${res.statusText}`);
    return res.json();
  }

  async commitPatch(
    taskId: string,
    workerId: string,
    fencingToken: number,
    fileId: string,
    baseRevision: number,
    symbolPath: string,
    replacementText: string,
    expectedFingerprint?: string,
    idempotencyKey?: string
  ): Promise<PatchCommitResponse> {
    const res = await fetch(`${this.baseUrl}/patches/commit`, {
      method: "POST",
      headers: this.headers(idempotencyKey),
      body: JSON.stringify({
        task_id: taskId,
        worker_id: workerId,
        fencing_token: fencingToken,
        file_id: fileId,
        base_revision: baseRevision,
        symbol_path: symbolPath,
        expected_fingerprint: expectedFingerprint,
        replacement_text: replacementText,
      }),
    });
    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(`Commit failed (${res.status}): ${err.message || res.statusText}`);
    }
    return res.json();
  }

  async getSemanticUnits(workspaceId: string, fileId: string): Promise<SemanticUnit[]> {
    const res = await fetch(`${this.baseUrl}/files/${fileId}/semantic-units?workspace_id=${workspaceId}`, {
      method: "GET",
      headers: this.headers(),
    });
    if (!res.ok) throw new Error(`Fetch units failed: ${res.statusText}`);
    const data = await res.json();
    return data.units || [];
  }
}
