#!/usr/bin/env python3
"""
API 自动化测试工具（冒烟测试）

默认覆盖：
1) health
2) auth: register/login/refresh
3) workspace + workflow（创建/查询/执行/执行列表）
4) task automation（创建后自动分配 + 完成闭环）
5) message（发送/列表/未读数/批量已读/批量删除）
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from typing import Any, Dict, List, Optional, Tuple


@dataclass
class StepResult:
    name: str
    ok: bool
    status: int
    duration_ms: int
    detail: str


class ApiError(Exception):
    def __init__(self, status: int, message: str, body: Optional[Any] = None):
        super().__init__(message)
        self.status = status
        self.body = body


class ApiClient:
    def __init__(self, base_url: str, timeout: int = 10):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout

    def request(
        self,
        method: str,
        path: str,
        token: Optional[str] = None,
        json_body: Optional[Dict[str, Any]] = None,
        expected: Optional[List[int]] = None,
    ) -> Tuple[int, Any]:
        url = f"{self.base_url}{path}"
        headers = {"Content-Type": "application/json"}
        if token:
            headers["Authorization"] = f"Bearer {token}"

        data = None
        if json_body is not None:
            data = json.dumps(json_body).encode("utf-8")

        req = urllib.request.Request(url, data=data, headers=headers, method=method.upper())

        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                status = resp.getcode()
                raw = resp.read().decode("utf-8")
        except urllib.error.HTTPError as e:
            status = e.code
            raw = e.read().decode("utf-8", errors="replace")
        except Exception as e:  # pragma: no cover
            raise ApiError(0, f"请求失败: {e}") from e

        body: Any
        try:
            body = json.loads(raw) if raw else {}
        except json.JSONDecodeError:
            body = raw

        if expected is not None and status not in expected:
            raise ApiError(status, f"期望状态码 {expected}，实际 {status}", body)
        return status, body


def now_suffix() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%d%H%M%S")


def extract_data(body: Any) -> Any:
    if isinstance(body, dict) and "data" in body:
        return body["data"]
    return body


def run_step(name: str, fn, results: List[StepResult]) -> Any:
    start = time.time()
    try:
        value = fn()
        duration = int((time.time() - start) * 1000)
        results.append(StepResult(name=name, ok=True, status=200, duration_ms=duration, detail="OK"))
        return value
    except ApiError as e:
        duration = int((time.time() - start) * 1000)
        detail = e.args[0]
        if e.body is not None:
            detail = f"{detail}; body={truncate(str(e.body), 300)}"
        results.append(StepResult(name=name, ok=False, status=e.status, duration_ms=duration, detail=detail))
        return None
    except Exception as e:  # pragma: no cover
        duration = int((time.time() - start) * 1000)
        results.append(StepResult(name=name, ok=False, status=0, duration_ms=duration, detail=str(e)))
        return None


def truncate(text: str, limit: int) -> str:
    if len(text) <= limit:
        return text
    return text[: limit - 3] + "..."


def print_summary(results: List[StepResult]) -> int:
    passed = sum(1 for r in results if r.ok)
    total = len(results)
    print("\n=== API TEST SUMMARY ===")
    print(f"Passed: {passed}/{total}")
    for r in results:
        mark = "PASS" if r.ok else "FAIL"
        print(f"[{mark}] {r.name} ({r.duration_ms}ms) status={r.status} detail={r.detail}")
    return 0 if passed == total else 1


def main() -> int:
    parser = argparse.ArgumentParser(description="API 接口自动化测试工具")
    parser.add_argument("--base-url", default="http://127.0.0.1:8000/api/v1", help="API Base URL")
    parser.add_argument("--timeout", type=int, default=10, help="单请求超时秒数")
    parser.add_argument("--skip-workflow", action="store_true", help="跳过 workflow 相关测试")
    parser.add_argument("--skip-task", action="store_true", help="跳过 task 自动化相关测试")
    parser.add_argument("--skip-message", action="store_true", help="跳过 message 相关测试")
    args = parser.parse_args()

    client = ApiClient(args.base_url, timeout=args.timeout)
    results: List[StepResult] = []

    suffix = now_suffix()
    username = f"apitest_{suffix}"
    email = f"apitest_{suffix}@example.com"
    password = "Test123456!"

    token: Optional[str] = None
    refresh_token: Optional[str] = None
    workspace_id: Optional[str] = None
    workflow_id: Optional[str] = None
    execution_id: Optional[str] = None
    agent_id: Optional[str] = None
    task_id: Optional[str] = None
    user_message_id: Optional[str] = None

    run_step(
        "health",
        lambda: client.request("GET", "/health", expected=[200]),
        results,
    )

    register_resp = run_step(
        "auth.register",
        lambda: client.request(
            "POST",
            "/auth/register",
            json_body={"username": username, "email": email, "password": password},
            expected=[200],
        ),
        results,
    )
    if register_resp is None:
        return print_summary(results)

    login_resp = run_step(
        "auth.login",
        lambda: client.request(
            "POST",
            "/auth/login",
            json_body={"username": username, "password": password},
            expected=[200],
        ),
        results,
    )
    if login_resp is None:
        return print_summary(results)

    _, login_body = login_resp
    login_data = extract_data(login_body)
    token = login_data.get("access_token")
    refresh_token = login_data.get("refresh_token")
    if not token or not refresh_token:
        results.append(StepResult("auth.tokens", False, 0, 0, "登录响应缺少 access_token/refresh_token"))
        return print_summary(results)
    results.append(StepResult("auth.tokens", True, 200, 0, "OK"))

    run_step(
        "auth.refresh",
        lambda: client.request(
            "POST",
            "/auth/refresh",
            json_body={"refresh_token": refresh_token},
            expected=[200],
        ),
        results,
    )

    ws_resp = run_step(
        "workspace.create",
        lambda: client.request(
            "POST",
            "/workspaces",
            token=token,
            json_body={"name": f"API Test WS {suffix}", "description": "api auto test"},
            expected=[200],
        ),
        results,
    )
    if ws_resp is not None:
        _, ws_body = ws_resp
        workspace_id = extract_data(ws_body).get("id")

    if not args.skip_workflow and workspace_id:
        wf_resp = run_step(
            "workflow.create",
            lambda: client.request(
                "POST",
                "/workflows",
                token=token,
                json_body={
                    "name": f"API Test Workflow {suffix}",
                    "description": "workflow smoke test",
                    "workspace_id": workspace_id,
                    "definition": {
                        "nodes": [{"id": "collect"}, {"id": "analyze"}],
                        "edges": [["collect", "analyze"]],
                    },
                },
                expected=[200],
            ),
            results,
        )
        if wf_resp is not None:
            _, wf_body = wf_resp
            workflow_id = extract_data(wf_body).get("id")

        if workflow_id:
            run_step(
                "workflow.list",
                lambda: client.request(
                    "GET",
                    f"/workflows?{urllib.parse.urlencode({'workspace_id': workspace_id})}",
                    token=token,
                    expected=[200],
                ),
                results,
            )
            run_step(
                "workflow.get",
                lambda: client.request("GET", f"/workflows/{workflow_id}", token=token, expected=[200]),
                results,
            )

            exec_resp = run_step(
                "workflow.execute",
                lambda: client.request(
                    "POST",
                    f"/workflows/{workflow_id}/execute",
                    token=token,
                    json_body={"input": {"source": "api_test"}, "options": {"priority": "high"}},
                    expected=[200],
                ),
                results,
            )
            if exec_resp is not None:
                _, exec_body = exec_resp
                data = extract_data(exec_body)
                execution = data.get("execution", {})
                execution_id = execution.get("id")

            run_step(
                "workflow.executions.list",
                lambda: client.request(
                    "GET",
                    f"/workflows/{workflow_id}/executions",
                    token=token,
                    expected=[200],
                ),
                results,
            )
            if execution_id:
                run_step(
                    "workflow.execution.get",
                    lambda: client.request(
                        "GET",
                        f"/workflows/{workflow_id}/executions/{execution_id}",
                        token=token,
                        expected=[200],
                    ),
                    results,
                )

    if not args.skip_task and token and workspace_id:
        agent_resp = run_step(
            "agent.register",
            lambda: client.request(
                "POST",
                "/agents",
                token=token,
                json_body={
                    "name": f"apitest-agent-{suffix}",
                    "description": "task automation test agent",
                    "capabilities": [
                        {
                            "name": "data_analysis",
                            "description": "data analysis",
                            "version": "1.0",
                            "parameters": {},
                        }
                    ],
                    "endpoints": {
                        "task_execution": "http://127.0.0.1:19001/run",
                        "health_check": "http://127.0.0.1:19001/health",
                        "status_update": None,
                    },
                    "limits": {
                        "max_concurrent_tasks": 3,
                        "max_execution_time": 120,
                        "max_memory_usage": None,
                        "rate_limit_per_minute": 120,
                    },
                    "metadata": {"source": "api_test"},
                },
                expected=[200],
            ),
            results,
        )
        if agent_resp is not None:
            _, agent_body = agent_resp
            agent_id = extract_data(agent_body).get("id")

        if agent_id:
            run_step(
                "agent.heartbeat",
                lambda: client.request(
                    "POST",
                    f"/agents/{agent_id}/heartbeat",
                    token=token,
                    json_body={
                        "current_load": 0,
                        "resource_usage": {"cpu": 15.0, "memory": 35.0, "disk": 20.0, "network": 5.0},
                        "active_tasks": [],
                    },
                    expected=[200],
                ),
                results,
            )

        task_resp = run_step(
            "task.create.auto_assign",
            lambda: client.request(
                "POST",
                "/tasks",
                token=token,
                json_body={
                    "title": f"API Task Auto Assign {suffix}",
                    "description": "verify automatic assignment and completion",
                    "priority": "medium",
                    "workspace_id": workspace_id,
                    "requirements": {"capabilities": ["data_analysis"]},
                    "context": {"source": "api_test"},
                    "metadata": {"scenario": "task_automation"},
                },
                expected=[200],
            ),
            results,
        )
        if task_resp is not None:
            _, task_body = task_resp
            task_data = extract_data(task_body)
            task_id = task_data.get("id")
            assigned_agent_id = task_data.get("assigned_agent_id")
            if agent_id and assigned_agent_id == agent_id:
                results.append(StepResult("task.auto_assign.verify", True, 200, 0, "OK"))
            else:
                results.append(
                    StepResult(
                        "task.auto_assign.verify",
                        False,
                        0,
                        0,
                        f"任务未自动分配到预期智能体, expected={agent_id}, actual={assigned_agent_id}",
                    )
                )

        if agent_id and task_id:
            run_step(
                "agent.complete_task",
                lambda: client.request(
                    "POST",
                    f"/agents/{agent_id}/complete-task",
                    token=token,
                    json_body={
                        "task_id": task_id,
                        "success": True,
                        "result": {"summary": "automation done", "score": 100},
                    },
                    expected=[200],
                ),
                results,
            )

            task_get_resp = run_step(
                "task.get.after_complete",
                lambda: client.request("GET", f"/tasks/{task_id}", token=token, expected=[200]),
                results,
            )
            if task_get_resp is not None:
                _, task_get_body = task_get_resp
                task_data = extract_data(task_get_body)
                status = task_data.get("status")
                if status == "completed":
                    results.append(StepResult("task.complete.verify", True, 200, 0, "OK"))
                else:
                    results.append(
                        StepResult(
                            "task.complete.verify",
                            False,
                            0,
                            0,
                            f"任务状态不是 completed, actual={status}",
                        )
                    )

    if not args.skip_message and token:
        send_resp = run_step(
            "message.send.user",
            lambda: client.request(
                "POST",
                "/messages",
                token=token,
                json_body={
                    "target_type": "user",
                    "target_id": extract_data(login_body).get("user", {}).get("id"),
                    "message_type": "notice",
                    "content": "api test message",
                },
                expected=[200],
            ),
            results,
        )
        if send_resp is not None:
            _, send_body = send_resp
            user_message_id = extract_data(send_body).get("id")

        run_step(
            "message.user.list",
            lambda: client.request("GET", "/messages/user", token=token, expected=[200]),
            results,
        )
        run_step(
            "message.user.unread_count",
            lambda: client.request("GET", "/messages/user/unread-count", token=token, expected=[200]),
            results,
        )

        if user_message_id:
            run_step(
                "message.batch.read",
                lambda: client.request(
                    "POST",
                    "/messages/user/read-batch",
                    token=token,
                    json_body={"message_ids": [user_message_id]},
                    expected=[200],
                ),
                results,
            )
            run_step(
                "message.batch.delete",
                lambda: client.request(
                    "POST",
                    "/messages/user/delete-batch",
                    token=token,
                    json_body={"message_ids": [user_message_id]},
                    expected=[200],
                ),
                results,
            )

    return print_summary(results)


if __name__ == "__main__":
    sys.exit(main())
