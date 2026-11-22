// Vercel Serverless Function: POST /api/v1/patches/commit (Durable Postgres CAS + Auth)
import { authorizeScope, resolveFileTarget } from "../_auth.js";
import { withProtectedApi } from "../_http.js";
import { commitTaskPatchDurable } from "../_tasks.js";

export default withProtectedApi({
  methods: ["POST"],
  headers: "Content-Type, Authorization, Idempotency-Key",
  checkPayload: true,
  onError: (err, res) => {
    const status = err.statusCode || 409;
    return res.status(status).json({
      code: err.code || (status === 412 ? "FENCING_TOKEN_STALE" : "SEMANTIC_WRITE_CONFLICT"),
      message: err.message,
    });
  },
  handler: async (req, res, { principal, store }) => {
    const idempotencyKey = req.headers?.["idempotency-key"] || null;
    const proposal = req.body || {};
    const target = resolveFileTarget(proposal.file_id, principal);
    authorizeScope(principal, target.workspaceId, "write", target.filePath);

    const result = await commitTaskPatchDurable(store, {
      target,
      actorId: principal.actorId,
      proposal,
      idempotencyKey,
    });

    return res.status(result.ok ? 200 : 409).json(result);
  },
});
