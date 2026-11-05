// Shared state for Vercel Serverless Function instance
// Implements File Commit Coordinator with CAS, Fencing Tokens, and Transactional Outbox

import crypto from "crypto";

class ServerlessCoordinator {
  constructor() {
    this.files = new Map();
    this.tasks = new Map();
    this.commits = new Map();
    this.outbox = [];
    this.nextEventId = 1;
    this.initDefaultFiles();
  }

  initDefaultFiles() {
    const demoCode = `pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
`;
    this.registerFile("default", "src/math.rs", demoCode, "rust");
  }

  computeHash(content) {
    return crypto.createHash("sha256").update(content).digest("hex");
  }

  registerFile(workspaceId, path, content, language) {
    const fileId = `${workspaceId}:${path}`;
    const hash = this.computeHash(content);
    const file = {
      fileId,
      workspaceId,
      path,
      content,
      language,
      currentRevision: 1,
      contentHash: hash,
      updatedAt: new Date().toISOString(),
    };
    this.files.set(fileId, file);
    return file;
  }

  getSemanticUnits(fileId) {
    const file = this.files.get(fileId);
    if (!file) return [];
    // Basic symbol extractor for serverless reflection
    const lines = file.content.split("\n");
    const units = [];
    let curByte = 0;
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (line.startsWith("pub fn ") || line.startsWith("fn ") || line.startsWith("def ")) {
        const parts = line.split(/[ (]/);
        const name = parts[1] === "fn" ? parts[2] : parts[1];
        const hash = this.computeHash(line);
        units.push({
          workspace_id: file.workspaceId,
          file_id: file.fileId,
          language: file.language,
          qualified_symbol_path: name,
          node_kind: "function_item",
          signature_hash: hash,
          base_content_hash: hash,
          node_fingerprint: this.computeHash(`${file.fileId}:${name}:${hash}`),
          start_line: i + 1,
          end_line: i + 3,
        });
      }
      curByte += lines[i].length + 1;
    }
    return units;
  }

  dispatchJob(workspaceId, goal, targetFiles, priority = 100) {
    const jobId = `job_${Date.now()}_${Math.random().toString(36).substring(7)}`;
    const taskIds = [];
    for (const fId of targetFiles) {
      const units = this.getSemanticUnits(fId);
      for (const unit of units) {
        const taskId = `${jobId}:${unit.qualified_symbol_path}`;
        const task = {
          taskId,
          jobId,
          workspaceId,
          fileId: fId,
          symbolPath: unit.qualified_symbol_path,
          baseRevision: 1,
          status: "READY",
          workerId: null,
          fencingToken: 0,
          leaseExpiresAt: null,
          expectedFingerprint: unit.node_fingerprint,
          priority,
          createdAt: new Date().toISOString(),
        };
        this.tasks.set(taskId, task);
        taskIds.push(taskId);
      }
    }
    return { jobId, taskIds, status: "QUEUED" };
  }

  claimTask(workerId, workspaceId) {
    for (const task of this.tasks.values()) {
      if (task.status === "READY" && task.workspaceId === workspaceId) {
        task.status = "CLAIMED";
        task.workerId = workerId;
        task.fencingToken += 1;
        task.leaseExpiresAt = new Date(Date.now() + 60000).toISOString();
        return task;
      }
    }
    return null;
  }

  commitPatch(proposal, idempotencyKey) {
    if (idempotencyKey && this.commits.has(idempotencyKey)) {
      return this.commits.get(idempotencyKey);
    }

    const task = this.tasks.get(proposal.task_id);
    if (!task) {
      throw new Error(`Task '${proposal.task_id}' not found`);
    }

    if (task.fencingToken !== proposal.fencing_token) {
      const err = new Error(`Fencing token stale (current: ${task.fencingToken}, provided: ${proposal.fencing_token})`);
      err.statusCode = 412;
      throw err;
    }

    const file = this.files.get(proposal.file_id);
    if (!file) {
      throw new Error(`File '${proposal.file_id}' not found`);
    }

    const isFastPath = file.currentRevision === proposal.base_revision;
    const previousRev = file.currentRevision;
    file.currentRevision += 1;
    task.status = "COMMITTED";

    const commitId = `commit-${proposal.task_id}-rev${file.currentRevision}`;
    const result = {
      commit_id: commitId,
      file_id: proposal.file_id,
      previous_revision: previousRev,
      new_revision: file.currentRevision,
      relocated: !isFastPath,
      status: "COMMITTED",
    };

    if (idempotencyKey) {
      this.commits.set(idempotencyKey, result);
    }

    this.outbox.push({
      event_id: this.nextEventId++,
      event_type: "FILE_COMMITTED",
      workspace_id: file.workspaceId,
      file_id: file.fileId,
      revision: file.currentRevision,
      payload: {
        commit_id: commitId,
        worker_id: proposal.worker_id,
        symbol_path: proposal.symbol_path,
        relocated: !isFastPath,
      },
      created_at: new Date().toISOString(),
    });

    return result;
  }

  getEvents(workspaceId, afterCursor = 0) {
    return this.outbox.filter(
      (e) => e.workspace_id === workspaceId && e.event_id > afterCursor
    );
  }
}

// Global singleton for serverless container reuse
if (!global.__ide_coordinator) {
  global.__ide_coordinator = new ServerlessCoordinator();
}

export const coordinator = global.__ide_coordinator;
