# 团队管理依赖系统 `curl` 的说明

本文说明当前 `Teams / 团队管理` 功能为什么依赖系统 `curl`、具体依赖了哪些动作，以及在本地环境中应该如何使用。

---

## 1. 当前结论

当前仓库中的团队管理能力，**依赖系统 PATH 中可执行的 `curl`**：

- Windows: `curl.exe`
- Linux / macOS: `curl`

如果本机没有可用的 `curl`，团队管理相关 RPC 会失败，并出现类似下面的错误：

```text
failed to execute curl.exe: ...
```

受影响的能力包括：

- 拉取团队列表
- 拉取成员列表
- 拉取待接受邀请
- 发送邀请
- 撤回邀请
- 移出成员

普通账号管理、设置页等不一定受这个依赖影响。

---

## 2. 代码位置

核心实现位于：

- `crates/service/src/team_management.rs`

关键点：

1. `system_curl_binary()`  
   根据平台返回 `curl.exe` 或 `curl`
2. `run_curl_request(...)`  
   统一通过 `std::process::Command` 调用系统 `curl`
3. `fetch_team_accounts / fetch_team_users / fetch_team_invites / send_team_invites / delete_team_invite / delete_team_member`  
   团队管理相关请求最终都走 `run_curl_request(...)`

Windows 下额外做了隐藏窗口处理：

- 通过 `CREATE_NO_WINDOW` 启动 `curl.exe`
- 这样不会再弹黑色控制台窗口

说明：即使不弹窗，任务管理器或安全审计工具里仍然可能看到 `curl.exe` 进程，这是正常现象。

---

## 3. 为什么这里选择依赖 `curl`

这不是为了“图省事”单纯 shell 一下，而是一个**有意保留的兼容性选择**。

当前团队管理访问的是：

- `https://chatgpt.com/backend-api`

而这组 Team 相关接口，相比普通 API，请求更接近浏览器侧流量，且在某些网络环境下更容易被 Cloudflare / 上游风控挑战。

当前实现保留系统 `curl`，主要有几个原因：

### 3.1 更接近浏览器请求形态

团队管理请求会带浏览器风格请求头，例如：

- `Origin: https://chatgpt.com`
- `Referer: https://chatgpt.com/`
- 浏览器 `User-Agent`
- `Authorization: Bearer ...`
- `chatgpt-account-id`

用 `curl` 执行这类请求，当前实践上比直接走内部 Rust HTTP 客户端更稳定，也更容易对齐真实浏览器流量。

### 3.2 代理支持更直接

团队管理会读取：

- `CODEXMANAGER_UPSTREAM_PROXY_URL`

并直接把它映射成：

- `curl --proxy <url>`

这样可以直接挂本地 SOCKS / HTTP 代理，或者配合仓库已有的 Cloudflare / WARP / `curl_cffi` 方案使用。

### 3.3 现实网络环境里成功率更高

仓库里已经有相关说明和脚本，核心背景就是：

- 某些机器 / 云环境下，直接访问 `chatgpt.com` 更容易遇到 Cloudflare challenge
- 浏览器指纹代理、WARP、本地代理链路更容易通过

因此团队管理目前保留 `curl` 依赖，是为了优先保证**可用性和成功率**，而不是为了追求实现形式上的“纯 Rust”。

---

## 4. 本地如何使用

### 4.1 确保系统里有 `curl`

先确认命令可用：

```bash
curl --version
```

Windows 新版系统通常自带 `curl.exe`。  
如果系统没有，就需要自行安装并确保加入 PATH。

---

### 4.2 正常使用团队管理

在桌面端或前端页面中进入：

- `Teams / 团队管理`

然后进行：

- 邀请成员
- 撤回邀请
- 移出成员
- 同步团队

这些操作都会自动走系统 `curl`，不需要手动输入命令。

---

### 4.3 如果上游环境容易被 Cloudflare 挑战

可以结合仓库现有脚本：

- `scripts/setup-cloudflare-warp-proxy.sh`
- `scripts/run-curl-cffi-chatgpt-proxy.sh`
- `scripts/curl_cffi_chatgpt_proxy.py`

常见思路是：

1. 先准备本地 WARP / SOCKS / HTTP 代理
2. 再把 CodexManager 的上游改到本地 `curl_cffi` 代理
3. 或设置：

```bash
CODEXMANAGER_UPSTREAM_PROXY_URL=<你的代理地址>
```

团队管理中的 `curl` 请求会自动复用这个代理配置。

---

## 5. 当前边界

当前版本的行为边界如下：

### 已解决

- Windows 下调用 `curl.exe` 不再弹黑框

### 仍然成立

- 团队管理依赖系统 `curl`
- 如果系统没有 `curl`，团队管理会失败
- 即使隐藏窗口，安全软件 / 任务管理器仍可能看到 `curl.exe`

### 如果后续要彻底去掉这个依赖

那需要单独评估并实现：

1. 内置 HTTP 客户端替代方案
2. 浏览器指纹 / Cloudflare 兼容性方案
3. 代理与错误处理回归
4. Windows / Linux / macOS 三端一致性验证

也就是说，这不是一个简单的“把命令行改成 reqwest”就能完全等价替换的问题。
