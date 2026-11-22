// Vercel Serverless Function: POST /api/v1/jobs/dispatch (Durable Postgres + Auth)
import { authorizeScope, resolveFileTarget } from "../_auth.js";
import { withProtectedApi } from "../_http.js";
import { dispatchJobDurable } from "../_tasks.js";

export default withProtectedApi({
  methods: ["POST"],
  headers: "Content-Type, Authorization, Idempotency-Key",
  checkPayload: true,
  handler: async (req, res, { principal, store }) => {
    const { workspace_id, goal, target_files, priority } = req.body || {};
    if (!workspace_id || !goal) {
      return res.status(400).json({ code: "BAD_REQUEST", message: "workspace_id and goal are required" });
    }
    authorizeScope(principal, workspace_id, "write");

    const result = await dispatchJobDurable(store, {
      tenantId: principal.tenantId,
      workspaceId: workspace_id,
      actorId: principal.actorId,
      goal,
      targetFiles: target_files,
      priority: priority || 100,
      resolveTargetFn: (rawId) => {
        const t = resolveFileTarget(rawId, { ...principal, defaultWorkspace: workspace_id });
        authorizeScope(principal, t.workspaceId, "write", t.filePath);
        return t;
      },
    });

    return res.status(202).json({
      job_id: result.jobId,
      workspace_id,
      task_count: result.taskIds.length,
      status: result.status,
    });
  },
});
