use actix_web::{HttpRequest, HttpResponse, Responder, get};
use serde_json::json;

use crate::api::auth_utils::{CONSOLE_SECRET_HEADER, validate_console_secret};

#[get("/")]
pub async fn console_page() -> impl Responder {
    let html = r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>APS Backend Console</title>
  <style>
    :root {
      --bg: #f3f6fb;
      --panel: rgba(255,255,255,0.92);
      --text: #10233f;
      --muted: #5c6a7f;
      --line: #d4ddeb;
      --primary: #1458d9;
      --primary-hover: #0f43a4;
      --danger: #cf2e2e;
      --ok: #0f8b5f;
      --shadow: 0 18px 40px rgba(16, 35, 63, 0.10);
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      font-family: "Segoe UI", "PingFang SC", "Noto Sans SC", sans-serif;
      color: var(--text);
      background:
        radial-gradient(circle at top left, rgba(20, 88, 217, 0.16), transparent 36%),
        radial-gradient(circle at right 20%, rgba(15, 139, 95, 0.18), transparent 24%),
        linear-gradient(180deg, #f7faff 0%, #eef3fa 100%);
      min-height: 100vh;
    }
    .shell {
      width: min(1120px, calc(100% - 32px));
      margin: 24px auto 48px;
    }
    .hero {
      border-radius: 20px;
      padding: 24px;
      color: #fff;
      background: linear-gradient(135deg, #0f4ac7 0%, #0f8b5f 100%);
      box-shadow: 0 20px 44px rgba(20, 88, 217, 0.24);
    }
    .hero h1 { margin: 0 0 8px; font-size: 28px; }
    .hero p { margin: 0; max-width: 720px; opacity: 0.96; }
    .panel {
      background: var(--panel);
      border: 1px solid rgba(212, 221, 235, 0.92);
      border-radius: 18px;
      padding: 18px;
      box-shadow: var(--shadow);
      backdrop-filter: blur(8px);
      margin-top: 16px;
    }
    .grid {
      display: grid;
      grid-template-columns: 1.05fr 1fr;
      gap: 16px;
    }
    .cols {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 16px;
    }
    @media (max-width: 900px) {
      .grid, .cols { grid-template-columns: 1fr; }
    }
    .tabs {
      display: flex;
      gap: 10px;
      margin-top: 16px;
      flex-wrap: wrap;
    }
    .tab {
      border: 1px solid var(--line);
      background: rgba(255,255,255,0.9);
      color: var(--text);
      padding: 9px 14px;
      border-radius: 999px;
      cursor: pointer;
      font-weight: 700;
    }
    .tab.active {
      background: var(--primary);
      color: #fff;
      border-color: var(--primary);
    }
    label {
      display: block;
      font-size: 13px;
      color: var(--muted);
      margin-bottom: 6px;
      font-weight: 600;
    }
    input {
      width: 100%;
      border: 1px solid var(--line);
      border-radius: 12px;
      padding: 11px 12px;
      margin-bottom: 12px;
      background: rgba(255,255,255,0.95);
      color: var(--text);
    }
    button {
      border: 0;
      border-radius: 12px;
      background: var(--primary);
      color: #fff;
      padding: 10px 14px;
      font-weight: 700;
      cursor: pointer;
    }
    button:hover { background: var(--primary-hover); }
    button.secondary {
      background: #eef4ff;
      color: var(--primary);
      border: 1px solid #cddcf8;
    }
    button.danger {
      background: #fff2f2;
      color: var(--danger);
      border: 1px solid #f2c6c6;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      font-size: 14px;
    }
    th, td {
      text-align: left;
      border-bottom: 1px solid var(--line);
      padding: 10px 8px;
      vertical-align: top;
    }
    pre {
      background: #0d1628;
      color: #d8e6ff;
      padding: 14px;
      border-radius: 12px;
      overflow-x: auto;
      font-size: 12px;
      min-height: 180px;
    }
    code {
      background: #eef4ff;
      border: 1px solid #d9e4f8;
      padding: 2px 6px;
      border-radius: 6px;
    }
    .muted { color: var(--muted); }
    .ok { color: var(--ok); }
    .danger-text { color: var(--danger); }
    .hidden { display: none; }
    .actions {
      display: flex;
      gap: 10px;
      flex-wrap: wrap;
      margin-top: 8px;
    }
  </style>
</head>
<body>
  <div class="shell">
    <section class="hero">
      <h1>APS 后端内置控制台</h1>
      <p>该页面使用控制台超级密钥进入，不依赖用户登录。当前密钥每次启动时随机生成，并写入 <code>config/default.toml</code> 的 <code>[security].super_secret_key</code>。</p>
    </section>

    <section class="grid">
      <div class="panel">
        <h3>控制台解锁</h3>
        <label>控制台密钥</label>
        <input id="console-secret" type="password" placeholder="粘贴 security.super_secret_key" />
        <div class="actions">
          <button id="unlock-btn">验证并进入</button>
          <button id="clear-secret-btn" class="secondary">清除本地密钥</button>
        </div>
        <p id="unlock-status" class="muted">尚未解锁</p>
      </div>

      <div class="panel">
        <h3>使用说明</h3>
        <p class="muted">1. 启动后端后，打开 <code>config/default.toml</code>。</p>
        <p class="muted">2. 复制 <code>[security].super_secret_key</code> 的值。</p>
        <p class="muted">3. 用该密钥解锁本页，再进行用户管理和接口调试。</p>
      </div>
    </section>

    <div id="console-app" class="hidden">
      <div class="tabs">
        <button class="tab active" data-tab="users">用户管理</button>
        <button class="tab" data-tab="api">API 页面</button>
      </div>

      <section id="tab-users">
        <div class="cols">
          <div class="panel">
            <h3>新建用户</h3>
            <label>用户名</label>
            <input id="new-username" placeholder="new-user" />
            <label>密码</label>
            <input id="new-password" type="password" placeholder="******" />
            <label>邮箱（可选）</label>
            <input id="new-email" placeholder="user@example.com" />
            <button id="create-user-btn">创建用户</button>
            <p id="create-user-status" class="muted"></p>
          </div>

          <div class="panel">
            <h3>用户列表</h3>
            <p class="muted">接口：<code>GET /api/public/console/users</code>，使用 <code>X-Console-Secret</code> 认证。</p>
            <button id="refresh-users-btn">刷新列表</button>
            <p id="users-status" class="muted"></p>
          </div>
        </div>

        <div class="panel">
          <table>
            <thead>
              <tr>
                <th>ID</th>
                <th>用户名</th>
                <th>邮箱</th>
                <th>创建时间</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody id="users-tbody"></tbody>
          </table>
        </div>
      </section>

      <section id="tab-api" class="hidden">
        <div class="panel">
          <h3>接口总览</h3>
          <p class="muted">来源：<code>GET /api/public/endpoints</code></p>
          <button id="load-endpoints-btn">加载接口列表</button>
          <table>
            <thead>
              <tr>
                <th>Method</th>
                <th>Path</th>
                <th>Auth</th>
                <th>Description</th>
              </tr>
            </thead>
            <tbody id="endpoints-tbody"></tbody>
          </table>
        </div>

        <div class="panel">
          <h3>快速调试</h3>
          <p class="muted">调试器会自动附带当前保存的 <code>X-Console-Secret</code>。如需访问用户 JWT 保护接口，可再手动填写 Authorization。</p>
          <label>Method</label>
          <input id="debug-method" value="GET" />
          <label>Path</label>
          <input id="debug-path" value="/api/public/console/users" />
          <label>Authorization（可选）</label>
          <input id="debug-auth" placeholder="Bearer eyJ..." />
          <label>Body JSON（可选）</label>
          <input id="debug-body" placeholder='{"k":"v"}' />
          <button id="debug-call-btn">发起请求</button>
          <pre id="debug-output">等待请求...</pre>
        </div>
      </section>
    </div>
  </div>

  <script>
    const secretStorageKey = "aps_console_super_secret";
    const consoleSecretHeader = "X-Console-Secret";

    function setStatus(id, text, className) {
      const node = document.getElementById(id);
      node.textContent = text;
      node.className = className || "muted";
    }

    function getSavedSecret() {
      return localStorage.getItem(secretStorageKey) || "";
    }

    function setSavedSecret(secret) {
      localStorage.setItem(secretStorageKey, secret);
      document.getElementById("console-secret").value = secret;
    }

    function clearSavedSecret() {
      localStorage.removeItem(secretStorageKey);
      document.getElementById("console-secret").value = "";
      lockConsole();
    }

    function lockConsole() {
      document.getElementById("console-app").classList.add("hidden");
      setStatus("unlock-status", "尚未解锁", "muted");
    }

    function unlockConsole() {
      document.getElementById("console-app").classList.remove("hidden");
      setStatus("unlock-status", "控制台已解锁", "ok");
    }

    async function secretFetch(path, options = {}) {
      const headers = { ...(options.headers || {}) };
      const secret = getSavedSecret();
      if (secret) {
        headers[consoleSecretHeader] = secret;
      }
      return fetch(path, { ...options, headers });
    }

    async function verifySecret(secret) {
      const res = await fetch("/api/public/console/verify", {
        method: "GET",
        headers: { [consoleSecretHeader]: secret },
      });
      return res.ok;
    }

    function renderUsers(users) {
      const tbody = document.getElementById("users-tbody");
      tbody.innerHTML = "";
      users.forEach((user) => {
        const row = document.createElement("tr");
        row.innerHTML = `
          <td>${user.id}</td>
          <td>${user.username}</td>
          <td>${user.email || "-"}</td>
          <td>${user.created_at || "-"}</td>
          <td><button class="danger" data-username="${user.username}">删除</button></td>
        `;
        tbody.appendChild(row);
      });

      tbody.querySelectorAll("button[data-username]").forEach((button) => {
        button.addEventListener("click", async () => {
          const username = button.dataset.username;
          if (!confirm(`确认删除用户 ${username} 吗？这会清理其关联工作区、任务、消息与智能体。`)) {
            return;
          }

          try {
            const res = await secretFetch(`/api/public/console/users/${encodeURIComponent(username)}`, {
              method: "DELETE",
            });
            const text = await res.text();
            if (!res.ok) throw new Error(text || "删除失败");
            setStatus("users-status", `已删除用户 ${username}`, "ok");
            await loadUsers();
          } catch (error) {
            setStatus("users-status", `删除失败: ${error.message}`, "danger-text");
          }
        });
      });
    }

    async function loadUsers() {
      setStatus("users-status", "加载中...", "muted");
      try {
        const res = await secretFetch("/api/public/console/users", { method: "GET" });
        const text = await res.text();
        let data;
        try { data = JSON.parse(text); } catch (_) { data = text; }
        if (!res.ok) throw new Error(typeof data === "string" ? data : JSON.stringify(data));
        renderUsers(data);
        setStatus("users-status", `共 ${data.length} 个用户`, "ok");
      } catch (error) {
        setStatus("users-status", `加载失败: ${error.message}`, "danger-text");
      }
    }

    document.querySelectorAll(".tab").forEach((button) => {
      button.addEventListener("click", () => {
        document.querySelectorAll(".tab").forEach((item) => item.classList.remove("active"));
        button.classList.add("active");
        const current = button.dataset.tab;
        document.getElementById("tab-users").classList.toggle("hidden", current !== "users");
        document.getElementById("tab-api").classList.toggle("hidden", current !== "api");
      });
    });

    document.getElementById("unlock-btn").addEventListener("click", async () => {
      const secret = document.getElementById("console-secret").value.trim();
      if (!secret) {
        setStatus("unlock-status", "请输入控制台密钥", "danger-text");
        return;
      }

      try {
        const ok = await verifySecret(secret);
        if (!ok) throw new Error("密钥校验失败");
        setSavedSecret(secret);
        unlockConsole();
        await loadUsers();
      } catch (error) {
        lockConsole();
        setStatus("unlock-status", `解锁失败: ${error.message}`, "danger-text");
      }
    });

    document.getElementById("clear-secret-btn").addEventListener("click", () => {
      clearSavedSecret();
    });

    document.getElementById("create-user-btn").addEventListener("click", async () => {
      const username = document.getElementById("new-username").value.trim();
      const password = document.getElementById("new-password").value;
      const emailRaw = document.getElementById("new-email").value.trim();
      const email = emailRaw.length ? emailRaw : null;

      if (!username || !password) {
        setStatus("create-user-status", "用户名和密码不能为空", "danger-text");
        return;
      }

      try {
        const res = await secretFetch("/api/public/console/users", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ username, password, email }),
        });
        const text = await res.text();
        if (!res.ok) throw new Error(text || "创建失败");
        setStatus("create-user-status", "创建成功", "ok");
        await loadUsers();
      } catch (error) {
        setStatus("create-user-status", `创建失败: ${error.message}`, "danger-text");
      }
    });

    document.getElementById("refresh-users-btn").addEventListener("click", loadUsers);

    document.getElementById("load-endpoints-btn").addEventListener("click", async () => {
      const tbody = document.getElementById("endpoints-tbody");
      tbody.innerHTML = "";
      const res = await fetch("/api/public/endpoints");
      const data = await res.json();
      (data.endpoints || []).forEach((ep) => {
        const row = document.createElement("tr");
        row.innerHTML = `<td>${ep.method}</td><td><code>${ep.path}</code></td><td>${ep.auth}</td><td>${ep.desc}</td>`;
        tbody.appendChild(row);
      });
    });

    document.getElementById("debug-call-btn").addEventListener("click", async () => {
      const method = (document.getElementById("debug-method").value || "GET").toUpperCase();
      const path = document.getElementById("debug-path").value.trim();
      const auth = document.getElementById("debug-auth").value.trim();
      const bodyRaw = document.getElementById("debug-body").value.trim();
      const headers = {};
      const secret = getSavedSecret();
      if (secret) headers[consoleSecretHeader] = secret;
      if (auth) headers["Authorization"] = auth;

      let body;
      if (bodyRaw) {
        headers["Content-Type"] = "application/json";
        body = bodyRaw;
      }

      const output = document.getElementById("debug-output");
      output.textContent = "请求中...";
      try {
        const res = await fetch(path, { method, headers, body });
        const text = await res.text();
        output.textContent = `Status: ${res.status}\n\n${text}`;
      } catch (error) {
        output.textContent = `请求失败: ${error.message}`;
      }
    });

    (async function bootstrap() {
      const saved = getSavedSecret();
      if (!saved) {
        lockConsole();
        return;
      }
      document.getElementById("console-secret").value = saved;
      try {
        const ok = await verifySecret(saved);
        if (!ok) throw new Error("已保存密钥失效");
        unlockConsole();
        await loadUsers();
      } catch (_) {
        clearSavedSecret();
      }
    })();
  </script>
</body>
</html>"#;

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

#[get("/api/public/console/verify")]
pub async fn verify_console_secret(req: HttpRequest) -> impl Responder {
    let secret = req
        .headers()
        .get(CONSOLE_SECRET_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");

    if validate_console_secret(secret) {
        HttpResponse::Ok().json(json!({ "ok": true }))
    } else {
        HttpResponse::Unauthorized().json(json!({ "ok": false }))
    }
}

#[get("/api/public/endpoints")]
pub async fn public_endpoints() -> impl Responder {
    HttpResponse::Ok().json(json!({
        "name": "agent-parallel-system",
        "version": "0.0.1",
        "endpoints": [
            { "method": "GET", "path": "/", "auth": "No", "desc": "内置控制台首页" },
            { "method": "GET", "path": "/api/public/console/verify", "auth": "X-Console-Secret", "desc": "验证控制台密钥" },
            { "method": "GET", "path": "/api/public/console/users", "auth": "X-Console-Secret", "desc": "查询用户列表" },
            { "method": "POST", "path": "/api/public/console/users", "auth": "X-Console-Secret", "desc": "创建用户" },
            { "method": "DELETE", "path": "/api/public/console/users/{username}", "auth": "X-Console-Secret", "desc": "删除用户" },
            { "method": "POST", "path": "/auth/register", "auth": "No", "desc": "用户注册" },
            { "method": "POST", "path": "/auth/login", "auth": "No", "desc": "用户登录" },
            { "method": "POST", "path": "/auth/refresh", "auth": "No", "desc": "刷新令牌" },
            { "method": "GET", "path": "/api/v1/users", "auth": "Bearer Access Token", "desc": "用户列表（JWT）" },
            { "method": "GET", "path": "/api/v1/system_info", "auth": "Bearer Access Token", "desc": "系统监控信息" },
            { "method": "GET", "path": "/api/v1/workspace", "auth": "Bearer Access Token", "desc": "查询工作区" },
            { "method": "POST", "path": "/api/v1/workspace", "auth": "Bearer Access Token", "desc": "创建工作区" },
            { "method": "GET", "path": "/api/v1/agent", "auth": "Bearer Access Token", "desc": "智能体列表" },
            { "method": "GET", "path": "/api/v1/tasks", "auth": "Bearer Access Token", "desc": "任务列表" },
            { "method": "GET", "path": "/api/v1/mcp/tools", "auth": "Bearer Access Token", "desc": "MCP 工具列表" },
            { "method": "GET", "path": "/api/v1/memory/nodes", "auth": "Bearer Access Token", "desc": "记忆节点列表" },
            { "method": "GET", "path": "/ws/chat", "auth": "Token in query", "desc": "WebSocket 聊天" },
            { "method": "GET", "path": "/logs/stream", "auth": "Token in query", "desc": "日志流 SSE" }
        ]
    }))
}