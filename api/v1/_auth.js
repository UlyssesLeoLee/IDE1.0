// Bearer Token / Short-Lived JWT Auth, Tenant/Workspace/File ACL & Rate Limiting (P0-03)

import crypto from "crypto";

export const MAX_PAYLOAD_BYTES = 2 * 1024 * 1024; // 2 MiB hard request cap
export const DEFAULT_RATE_LIMIT_PER_MIN = 300;

export class AuthError extends Error {
  constructor(statusCode, code, message) {
    super(message);
    this.statusCode = statusCode;
    this.code = code;
  }
}

function base64UrlEncode(bufOrStr) {
  const buf = Buffer.isBuffer(bufOrStr) ? bufOrStr : Buffer.from(bufOrStr, "utf8");
  return buf.toString("base64").replace(/=/g, "").replace(/\+/g, "-").replace(/\//g, "_");
}

function base64UrlDecode(str) {
  const padLen = (4 - (str.length % 4)) % 4;
  const b64 = str.replace(/-/g, "+").replace(/_/g, "/") + "=".repeat(padLen);
  return Buffer.from(b64, "base64");
}

function timingSafeStrEqual(a, b) {
  const ha = crypto.createHash("sha256").update(String(a)).digest();
  const hb = crypto.createHash("sha256").update(String(b)).digest();
  return crypto.timingSafeEqual(ha, hb);
}

export function signJwt(payload, secret, ttlSeconds = 3600) {
  const now = Math.floor(Date.now() / 1000);
  const claims = {
    iat: now,
    exp: now + ttlSeconds,
    ...payload,
  };
  const headerB64 = base64UrlEncode(JSON.stringify({ alg: "HS256", typ: "JWT" }));
  const payloadB64 = base64UrlEncode(JSON.stringify(claims));
  const data = `${headerB64}.${payloadB64}`;
  const sig = crypto.createHmac("sha256", secret).update(data).digest();
  return `${data}.${base64UrlEncode(sig)}`;
}

export function verifyJwt(token, secret) {
  const parts = String(token || "").split(".");
  if (parts.length !== 3) return null;
  const [headerB64, payloadB64, sigB64] = parts;
  let header;
  let payload;
  try {
    header = JSON.parse(base64UrlDecode(headerB64).toString("utf8"));
    payload = JSON.parse(base64UrlDecode(payloadB64).toString("utf8"));
  } catch {
    throw new AuthError(401, "INVALID_TOKEN", "Malformed JWT encoding");
  }
  if (header.alg !== "HS256") {
    throw new AuthError(401, "INVALID_TOKEN", "Unsupported JWT algorithm");
  }

  const expectedSig = crypto
    .createHmac("sha256", secret)
    .update(`${headerB64}.${payloadB64}`)
    .digest();
  const providedSig = base64UrlDecode(sigB64);
  if (
    expectedSig.length !== providedSig.length ||
    !crypto.timingSafeEqual(expectedSig, providedSig)
  ) {
    throw new AuthError(401, "INVALID_TOKEN", "Invalid JWT signature");
  }

  const now = Math.floor(Date.now() / 1000);
  if (typeof payload.exp !== "number" || now >= payload.exp) {
    throw new AuthError(401, "TOKEN_EXPIRED", "Bearer JWT has expired");
  }
  if (typeof payload.nbf === "number" && now < payload.nbf) {
    throw new AuthError(401, "TOKEN_NOT_YET_VALID", "Bearer JWT is not active yet");
  }
  if (!payload.tenant_id || !payload.sub) {
    throw new AuthError(401, "INVALID_TOKEN", "JWT missing required tenant_id or sub claim");
  }

  return {
    actorId: String(payload.sub),
    tenantId: String(payload.tenant_id),
    workspaces: Array.isArray(payload.workspaces) ? payload.workspaces.map(String) : ["*"],
    scopes: Array.isArray(payload.scopes) ? payload.scopes.map(String) : ["read", "write"],
    defaultWorkspace: payload.default_workspace ? String(payload.default_workspace) : null,
    authMethod: "jwt",
  };
}

function loadStaticTokens(env) {
  const raw = env.IDE_MCP_TOKENS;
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

export function authenticateRequest(req, env = process.env) {
  const jwtSecret = env.IDE_MCP_JWT_SECRET || "";
  const staticTokens = loadStaticTokens(env);

  if (!jwtSecret && staticTokens.length === 0) {
    throw new AuthError(
      503,
      "AUTH_NOT_CONFIGURED",
      "MCP authorization secrets are not configured on this server"
    );
  }

  const authHeader = req.headers?.["authorization"] || req.headers?.["Authorization"] || "";
  const m = String(authHeader).match(/^Bearer\s+(.+)$/i);
  if (!m) {
    throw new AuthError(401, "UNAUTHORIZED", "Missing Bearer token in Authorization header");
  }
  const token = m[1].trim();
  if (!token) {
    throw new AuthError(401, "UNAUTHORIZED", "Empty Bearer token");
  }

  if (token.split(".").length === 3 && jwtSecret) {
    return verifyJwt(token, jwtSecret);
  }

  for (const entry of staticTokens) {
    if (entry?.token && timingSafeStrEqual(token, entry.token)) {
      if (entry.expires_at && Date.now() > Date.parse(entry.expires_at)) {
        throw new AuthError(401, "TOKEN_EXPIRED", "Static Bearer token has expired");
      }
      return {
        actorId: String(entry.actor_id || entry.sub || "service-agent"),
        tenantId: String(entry.tenant_id || "default"),
        workspaces: Array.isArray(entry.workspaces) ? entry.workspaces.map(String) : ["*"],
        scopes: Array.isArray(entry.scopes) ? entry.scopes.map(String) : ["read", "write"],
        defaultWorkspace: entry.default_workspace ? String(entry.default_workspace) : null,
        authMethod: "bearer_token",
      };
    }
  }

  throw new AuthError(401, "UNAUTHORIZED", "Invalid Bearer token");
}

export function authorizeScope(principal, workspaceId, requiredScope = "read") {
  if (!principal) {
    throw new AuthError(401, "UNAUTHORIZED", "Authentication required");
  }
  const wsAllowed =
    principal.workspaces.includes("*") ||
    principal.workspaces.includes(workspaceId) ||
    principal.workspaces.some(
      (prefix) => prefix.endsWith("*") && workspaceId.startsWith(prefix.slice(0, -1))
    );
  if (!wsAllowed) {
    throw new AuthError(
      403,
      "FORBIDDEN_WORKSPACE",
      `Access denied to workspace '${workspaceId}' for tenant '${principal.tenantId}'`
    );
  }

  const hasScope =
    principal.scopes.includes("*") ||
    principal.scopes.includes("admin") ||
    principal.scopes.includes(requiredScope);
  if (!hasScope) {
    throw new AuthError(
      403,
      "INSUFFICIENT_SCOPE",
      `Required scope '${requiredScope}' missing for actor '${principal.actorId}'`
    );
  }
}

export function resolveFileTarget(rawFileId, principal) {
  if (typeof rawFileId !== "string" || !rawFileId.trim()) {
    throw new AuthError(400, "INVALID_FILE_ID", "file_id must be a non-empty string");
  }
  if (rawFileId.length > 512 || rawFileId.includes("\0")) {
    throw new AuthError(400, "INVALID_FILE_ID", "Invalid characters or length in file_id");
  }
  const idx = rawFileId.indexOf(":");
  const workspaceId =
    idx > 0
      ? rawFileId.slice(0, idx).trim()
      : principal?.defaultWorkspace || "default";
  const filePath = (idx > 0 ? rawFileId.slice(idx + 1) : rawFileId).trim();

  if (
    !workspaceId ||
    !/^[A-Za-z0-9_.-]+$/.test(workspaceId) ||
    !filePath ||
    filePath.startsWith("/") ||
    filePath.includes("..") ||
    filePath.includes("\\")
  ) {
    throw new AuthError(
      400,
      "INVALID_FILE_PATH",
      `Unsafe or malformed file_id '${rawFileId}'`
    );
  }

  return {
    tenantId: principal?.tenantId || "default",
    workspaceId,
    filePath,
    fileId: `${workspaceId}:${filePath}`,
  };
}

export function enforcePayloadSize(req) {
  const clHeader = req.headers?.["content-length"];
  if (clHeader && Number(clHeader) > MAX_PAYLOAD_BYTES) {
    throw new AuthError(
      413,
      "PAYLOAD_TOO_LARGE",
      `Request body exceeds ${MAX_PAYLOAD_BYTES} bytes limit`
    );
  }
  if (typeof req.body === "string" && Buffer.byteLength(req.body, "utf8") > MAX_PAYLOAD_BYTES) {
    throw new AuthError(
      413,
      "PAYLOAD_TOO_LARGE",
      `Request body exceeds ${MAX_PAYLOAD_BYTES} bytes limit`
    );
  }
}
