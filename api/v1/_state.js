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
    this.initTenants();
    this.initDefaultFiles();
  }

  initTenants() {
    this.tenants = new Map();
    this.tenants.set("org_default", {
      tenantId: "org_default",
      name: "Acme Corp (Global)",
      tier: "Enterprise",
      maxAi: 50,
      rateLimit: 1000,
      createdAt: new Date().toISOString(),
    });
    this.tenants.set("org_demo", {
      tenantId: "org_demo",
      name: "Demo Startup",
      tier: "Pro",
      maxAi: 5,
      rateLimit: 100,
      createdAt: new Date().toISOString(),
    });
    this.collabSessions = new Map();
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

  getTenantContext(tenantId = "org_default") {
    let t = this.tenants.get(tenantId);
    if (!t) {
      t = {
        tenantId,
        name: `Tenant ${tenantId}`,
        tier: "Standard",
        maxAi: 10,
        rateLimit: 200,
        createdAt: new Date().toISOString(),
      };
      this.tenants.set(tenantId, t);
    }
    let activeAgents = 0;
    for (const [key, session] of this.collabSessions.entries()) {
      if (key.startsWith(`${tenantId}:`)) {
        activeAgents += session.collaborators.size;
      }
    }
    return {
      ...t,
      activeAgents,
    };
  }

  getOrCreateCollabSession(tenantId = "org_default", workspaceId = "default", fileId = "src/math.rs") {
    const sessionKey = `${tenantId}:${workspaceId}:${fileId}`;
    let session = this.collabSessions.get(sessionKey);
    if (!session) {
      session = {
        tenantId,
        workspaceId,
        fileId,
        collaborators: new Map(),
        lineAttributions: new Map(),
        anchorTable: { anchors: [] },
        nextBroadcastId: 1,
        history: [],
        upfrontPlan: null,
        hints: [],
        certificate: null,
      };
      this.collabSessions.set(sessionKey, session);
    }
    return session;
  }

  registerPresence(tenantId, workspaceId, fileId, presence) {
    const tenant = this.getTenantContext(tenantId);
    const session = this.getOrCreateCollabSession(tenantId, workspaceId, fileId);

    // 配额校验: 如果超出租户最大允许并发 Agent 且是新加入的 agent
    if (!session.collaborators.has(presence.agent_id) && tenant.activeAgents >= tenant.maxAi) {
      const err = new Error(`Tenant '${tenantId}' exceeded max concurrent agent quota (${tenant.maxAi})`);
      err.statusCode = 429;
      throw err;
    }

    const color = presence.color || this.getColorFromId(presence.agent_id);
    const nowMs = Date.now();
    const collaborator = {
      agent_id: presence.agent_id,
      display_name: presence.display_name || presence.agent_id,
      role: presence.role || "AiCoder",
      color,
      cursor: presence.cursor || [0, 0],
      selection: presence.selection || null,
      active_symbol: presence.active_symbol || null,
      last_active_epoch_ms: nowMs,
    };

    session.collaborators.set(presence.agent_id, collaborator);

    this.outbox.push({
      event_id: this.nextEventId++,
      event_type: "PRESENCE_UPDATED",
      tenant_id: tenantId,
      workspace_id: workspaceId,
      file_id: fileId,
      payload: collaborator,
      created_at: new Date().toISOString(),
    });

    return {
      collaborator,
      active_collaborators: Array.from(session.collaborators.values()),
      plan: session.upfrontPlan,
    };
  }

  getColorFromId(id) {
    let hash = 0;
    for (let i = 0; i < id.length; i++) {
      hash = (hash << 5) - hash + id.charCodeAt(i);
      hash |= 0;
    }
    const palette = [
      { r: 52, g: 211, b: 153, hex: "#34D399" }, // Emerald
      { r: 168, g: 85, b: 247, hex: "#A855F7" }, // Purple
      { r: 251, g: 191, b: 36, hex: "#FBBF24" }, // Amber
      { r: 56, g: 189, b: 248, hex: "#38BDF8" }, // Sky
      { r: 244, g: 63, b: 94, hex: "#F43F5E" },  // Rose
      { r: 45, g: 212, b: 191, hex: "#2DD4BF" }, // Teal
    ];
    return palette[Math.abs(hash) % palette.length];
  }

  applyKeystroke(tenantId, workspaceId, fileId, agentId, op, cursorAfter, selectionAfter) {
    const session = this.getOrCreateCollabSession(tenantId, workspaceId, fileId);
    let c = session.collaborators.get(agentId);
    if (!c) {
      this.registerPresence(tenantId, workspaceId, fileId, { agent_id: agentId });
      c = session.collaborators.get(agentId);
    }

    c.cursor = cursorAfter;
    c.selection = selectionAfter || null;
    const nowMs = Date.now();
    c.last_active_epoch_ms = nowMs;

    // 行归属更新
    const row = op.row !== undefined ? op.row : (op.start ? op.start[0] : 0);
    session.lineAttributions.set(row, { agent_id: agentId, timestamp: nowMs });

    const broadcast = {
      broadcast_id: session.nextBroadcastId++,
      tenant_id: tenantId,
      workspace_id: workspaceId,
      file_id: fileId,
      agent_id: agentId,
      display_name: c.display_name,
      role: c.role,
      op,
      cursor_after: cursorAfter,
      selection_after: selectionAfter || null,
      timestamp_epoch_ms: nowMs,
    };

    session.history.push(broadcast);

    this.outbox.push({
      event_id: this.nextEventId++,
      event_type: "KEYSTROKE_BROADCAST",
      tenant_id: tenantId,
      workspace_id: workspaceId,
      file_id: fileId,
      payload: broadcast,
      created_at: new Date().toISOString(),
    });

    return broadcast;
  }

  setUpfrontPlan(tenantId, workspaceId, fileId, plan) {
    const session = this.getOrCreateCollabSession(tenantId, workspaceId, fileId);
    session.upfrontPlan = {
      task_goal: plan.task_goal,
      invariants: plan.invariants || [],
      guidance_zones: plan.guidance_zones || [],
      shared_contracts: plan.shared_contracts || [],
      notes: plan.notes || "",
      created_at: new Date().toISOString(),
    };

    this.outbox.push({
      event_id: this.nextEventId++,
      event_type: "UPFRONT_PLAN_PUBLISHED",
      tenant_id: tenantId,
      workspace_id: workspaceId,
      file_id: fileId,
      payload: session.upfrontPlan,
      created_at: new Date().toISOString(),
    });

    return session.upfrontPlan;
  }

  submitReviewDecision(tenantId, workspaceId, fileId, decision) {
    const session = this.getOrCreateCollabSession(tenantId, workspaceId, fileId);
    const now = new Date().toISOString();
    const certPayload = `${tenantId}:${workspaceId}:${fileId}:${decision.reviewer_id}:${decision.decision_type}:${now}`;
    const certHash = this.computeHash(certPayload);

    const certificate = {
      certificate_id: `CERT-${Date.now().toString(36).toUpperCase()}`,
      tenant_id: tenantId,
      workspace_id: workspaceId,
      file_id: fileId,
      decision: decision.decision_type,
      reviewer_id: decision.reviewer_id,
      notes: decision.notes || "",
      sha256_hash: certHash,
      issued_at: now,
    };

    session.certificate = certificate;

    this.outbox.push({
      event_id: this.nextEventId++,
      event_type: "REVIEW_DECISION_ISSUED",
      tenant_id: tenantId,
      workspace_id: workspaceId,
      file_id: fileId,
      payload: certificate,
      created_at: now,
    });

    return certificate;
  }

  getEvents(workspaceId, afterCursor = 0) {
    return this.outbox.filter(
      (e) => e.workspace_id === workspaceId && e.event_id > afterCursor
    );
  }

  getCollabEvents(tenantId = "org_default", workspaceId = "default", fileId = null, afterCursor = 0) {
    return this.outbox.filter((e) => {
      if (e.tenant_id && e.tenant_id !== tenantId) return false;
      if (e.workspace_id && e.workspace_id !== workspaceId) return false;
      if (fileId && e.file_id && e.file_id !== fileId) return false;
      return e.event_id > afterCursor;
    });
  }
}

// Global singleton for serverless container reuse
if (!global.__ide_coordinator) {
  global.__ide_coordinator = new ServerlessCoordinator();
}

export const coordinator = global.__ide_coordinator;
