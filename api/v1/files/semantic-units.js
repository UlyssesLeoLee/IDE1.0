// Vercel Serverless Function: GET /api/v1/files/semantic-units (Durable Postgres + Auth)
import { authorizeScope, resolveFileTarget } from "../_auth.js";
import { withProtectedApi } from "../_http.js";
import { extractSemanticUnits } from "../_tasks.js";

export default withProtectedApi({
  methods: ["GET"],
  handler: async (req, res, { principal, store }) => {
    const rawFileId = req.query?.file_id || "default:src/math.rs";
    const target = resolveFileTarget(rawFileId, principal);
    authorizeScope(principal, target.workspaceId, "read", target.filePath);

    const doc = await store.readFile(target);
    const units = extractSemanticUnits(
      target.tenantId,
      target.workspaceId,
      doc.file_id,
      doc.language,
      doc.content
    );
    return res.status(200).json({ file_id: doc.file_id, units });
  },
});
