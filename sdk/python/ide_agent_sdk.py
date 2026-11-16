"""
IDE1.0 Web Agent Router — Python SDK
High-concurrency Agent Collaboration Client with Fencing Tokens, Leases, and AST Relocation.
"""

import json
import urllib.request
import urllib.error
from typing import Dict, Any, Optional, List

class IDEAgentClient:
    def __init__(self, base_url: str = "http://localhost:3000/api/v1", token: Optional[str] = None):
        self.base_url = base_url.rstrip("/")
        self.token = token

    def _headers(self, idempotency_key: Optional[str] = None) -> Dict[str, str]:
        headers = {
            "Content-Type": "application/json",
            "User-Agent": "IDE1.0-Python-Agent-SDK/2.0"
        }
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        if idempotency_key:
            headers["Idempotency-Key"] = idempotency_key
        return headers

    def _post(self, path: str, payload: Dict[str, Any], idempotency_key: Optional[str] = None) -> Dict[str, Any]:
        url = f"{self.base_url}{path}"
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(url, data=data, headers=self._headers(idempotency_key), method="POST")
        try:
            with urllib.request.urlopen(req) as resp:
                if resp.status == 204:
                    return {}
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            err_body = e.read().decode("utf-8")
            raise RuntimeError(f"HTTP {e.code} Error: {err_body}")

    def _get(self, path: str) -> Dict[str, Any]:
        url = f"{self.base_url}{path}"
        req = urllib.request.Request(url, headers=self._headers(), method="GET")
        with urllib.request.urlopen(req) as resp:
            return json.loads(resp.read().decode("utf-8"))

    def dispatch_job(self, workspace_id: str, goal: str, target_files: Optional[List[str]] = None, priority: int = 100) -> Dict[str, Any]:
        """Dispatch high level job into semantic tasks."""
        payload = {
            "workspace_id": workspace_id,
            "goal": goal,
            "target_files": target_files or [],
            "priority": priority
        }
        return self._post("/jobs/dispatch", payload)

    def claim_task(self, worker_id: str, workspace_id: str) -> Optional[Dict[str, Any]]:
        """Claim a ready semantic unit task with lease and fencing token."""
        payload = {
            "worker_id": worker_id,
            "workspace_id": workspace_id
        }
        return self._post("/tasks/claim", payload)

    def heartbeat(self, task_id: str, worker_id: str, fencing_token: int) -> Dict[str, Any]:
        """Renew lease with fencing token."""
        payload = {
            "worker_id": worker_id,
            "fencing_token": fencing_token
        }
        return self._post(f"/tasks/{task_id}/heartbeat", payload)

    def commit_patch(
        self,
        task_id: str,
        worker_id: str,
        fencing_token: int,
        file_id: str,
        base_revision: int,
        symbol_path: str,
        replacement_text: str,
        expected_fingerprint: Optional[str] = None,
        idempotency_key: Optional[str] = None
    ) -> Dict[str, Any]:
        """Commit an AST-aware patch proposal to the File Commit Coordinator."""
        payload = {
            "task_id": task_id,
            "worker_id": worker_id,
            "fencing_token": fencing_token,
            "file_id": file_id,
            "base_revision": base_revision,
            "symbol_path": symbol_path,
            "expected_fingerprint": expected_fingerprint,
            "replacement_text": replacement_text
        }
        return self._post("/patches/commit", payload, idempotency_key=idempotency_key)

    def get_semantic_units(self, workspace_id: str, file_id: str) -> List[Dict[str, Any]]:
        """Fetch parsed semantic units and fingerprints for a file."""
        res = self._get(f"/files/{file_id}/semantic-units?workspace_id={workspace_id}")
        return res.get("units", [])

    def get_events(self, workspace_id: str, after_cursor: int = 0) -> List[Dict[str, Any]]:
        """Read transactional outbox events."""
        res = self._get(f"/events?workspace_id={workspace_id}&after_cursor={after_cursor}")
        return res.get("events", [])
