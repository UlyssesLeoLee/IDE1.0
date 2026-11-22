// Shared HTTP wrapper for authenticated Vercel REST coordination endpoints
import { authenticateRequest, AuthError, enforcePayloadSize } from "./_auth.js";
import { getStore } from "./_store.js";

export function withProtectedApi({
  methods = ["GET"],
  headers = "Content-Type, Authorization",
  checkPayload = false,
  onError = null,
  handler,
}) {
  const allowMethods = [...methods, "OPTIONS"].join(", ");
  return async function routeHandler(req, res) {
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", allowMethods);
    res.setHeader("Access-Control-Allow-Headers", headers);

    if (req.method === "OPTIONS") return res.status(200).end();
    if (!methods.includes(req.method)) {
      return res.status(405).json({ code: "METHOD_NOT_ALLOWED" });
    }

    try {
      if (checkPayload) enforcePayloadSize(req);
      const principal = authenticateRequest(req);
      const store = getStore();
      await store.enforceRateLimit(principal.tenantId);
      return await handler(req, res, { principal, store });
    } catch (err) {
      if (err instanceof AuthError) {
        return res.status(err.statusCode).json({ code: err.code, message: err.message });
      }
      if (typeof onError === "function") {
        return onError(err, res);
      }
      return res
        .status(err.statusCode || 500)
        .json({ code: err.code || "INTERNAL_ERROR", message: err.message });
    }
  };
}
