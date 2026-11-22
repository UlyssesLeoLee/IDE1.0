// Vercel Serverless Function: GET /api/v1/events (Durable Postgres Outbox + Auth)
import { authorizeScope } from "./_auth.js";
import { withProtectedApi } from "./_http.js";

export default withProtectedApi({
  methods: ["GET"],
  handler: async (req, res, { principal, store }) => {
    const workspaceId = String(req.query?.workspace_id || principal.defaultWorkspace || "default");
    authorizeScope(principal, workspaceId, "read");
    const afterCursor = parseInt(req.query?.after_cursor || "0", 10) || 0;

    const events = await store.getEvents(principal.tenantId, workspaceId, afterCursor);
    return res.status(200).json({ events });
  },
});
