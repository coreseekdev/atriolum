# SDK 兼容性分析报告

> 基于 sentry-rust、sentry-javascript、sentry-native (C++) SDK 源码分析
> 对照 Atriolum 当前实现逐项比对
> 最后更新：2026-04-14

---

## 总览

| SDK | 协议兼容 | 数据接收 | 已验证 | 关键阻塞问题 |
|-----|---------|---------|--------|------------|
| **Python** | ✅ 完全兼容 | ✅ 已验证 | ✅ 6 项测试通过 | 无 |
| **Rust** | ✅ 完全兼容 | ⚠️ 未实测 | ❌ | 无 |
| **JavaScript** | ✅ 完全兼容 | ⚠️ 未实测 | ❌ | 无 |
| **C++ (Native)** | ✅ 完全兼容 | ✅ 端点已实现 | ❌ | 无（minidump 端点已添加） |

---

## 1. Rust SDK (`sentry-rust`)

### 发送的 Envelope Item 类型

| 类型 | Atriolum 处理 | 备注 |
|------|-------------|------|
| `event` | ✅ | |
| `transaction` | ✅ | |
| `session` | ✅ | |
| `sessions` (聚合) | ✅ | |
| `attachment` | ✅ | |
| `check_in` | ✅ | |
| `log` | ✅ | |

### 兼容性细节

#### ✅ 已兼容

- **Event 字段**: `culprit`、`logentry`、`message`、`fingerprint`、`exception`、`threads`、`breadcrumbs`、`contexts`、`user`、`request`、`tags`、`extra` — 全部正确解析
- **时间戳格式**: Rust SDK 发送 Unix epoch 浮点数（`1595256674.296`），Atriolum 自定义反序列化器同时支持字符串和数字 ✅
- **认证**: `X-Sentry-Auth` header，格式 `Sentry sentry_key=..., sentry_version=7` ✅
- **Context 自动填充**: `os`、`rust`（name="rustc"）、`device`（arch）— 作为 `HashMap<String, Value>` 存储，兼容 ✅
- **Session 格式**: `{sid, init, started, timestamp, status, duration, errors, attrs}` ✅
- **Breadcrumb**: `{timestamp, type, level, category, message, data}` ✅
- **platform 字段**: Rust SDK 默认发送 `"native"`，Atriolum 存储为 `Option<String>` ✅

#### ⚠️ 小差异（不影响功能）

1. **`fingerprint` 默认值**: Rust SDK 默认发送 `["{{ default }}"]`，Atriolum 存储为空 `Vec`。不影响接收，但查询时可能需要特殊处理。
2. **不压缩**: Rust SDK 默认不压缩请求体（无 `Content-Encoding`）。Atriolum 处理 `identity` 编码正确 ✅
3. **`sentry_timestamp` 认证字段**: Rust SDK 可能在 auth header 中包含已弃用的 `sentry_timestamp`。Atriolum 解析时忽略未知字段 ✅

#### ❌ 需要修复

无阻塞问题。

---

## 2. JavaScript SDK (`sentry-javascript`)

### 发送的 Envelope Item 类型

| 类型 | Atriolum 处理 | JS SDK 来源 | 备注 |
|------|-------------|------------|------|
| `event` | ✅ | Browser + Node | |
| `transaction` | ✅ | Browser + Node | |
| `session` | ✅ | Browser | 页面加载自动发送 |
| `sessions` | ✅ | Node | |
| `attachment` | ✅ | Browser + Node | |
| `client_report` | ✅ | Browser + Node | |
| `user_report` | ✅ | Browser | |
| `check_in` | ✅ | Node | |
| `span` | ✅ | Browser + Node | |
| `profile` | ✅ | Browser + Node | |
| `profile_chunk` | ✅ | Browser + Node | |
| `log` | ✅ | Node | |
| `replay_event` | ✅ | Browser | |
| `replay_recording` | ✅ | Browser | |
| `feedback` | ⚠️ | Browser | 存为 raw，无专用处理 |
| `trace_metric` | ⚠️ | Browser + Node | 存为 raw |
| `raw_security` | ❌ | Browser | 未处理 |

### 兼容性细节

#### ✅ 已兼容

- **Envelope 格式**: JS SDK 使用标准 newline-delimited 格式 ✅
- **CORS**: 浏览器 SDK 需要跨域支持，Atriolum 已返回 `Access-Control-Allow-Origin: *` ✅
- **Content-Type**: JS SDK 发送 `application/x-sentry-envelope`，Atriolum 不校验 Content-Type ✅
- **Breadcrumb 类型**: console、DOM、XHR、fetch、history — 全部作为 `{type, category, message, data}` 存储 ✅
- **Context**: JS SDK 自动添加 `culture`（locale、timezone）、`trace`（分布式追踪）— 存为灵活 Map ✅
- **动态采样上下文 (DSC)**: `trace_id`、`public_key`、`sample_rate`、`replay_id` 等 — 在 envelope header 中，Atriolum 保留 ✅

#### ⚠️ 需要关注

1. **缺少 Rate Limit 响应头**:
   - JS SDK 在 `beforeSend` 中检查响应头决定是否丢弃事件
   - Atriolum 当前不返回 `X-Sentry-Rate-Limits` 或 `Retry-After` 头
   - **影响**: SDK 不会自行限速，高频场景可能淹没服务器
   - **修复**: 成功响应添加空 rate-limit 头，或在限速时返回 429

2. **`feedback` 类型**: JS SDK 的用户反馈功能发送 `feedback` 类型（不是 `user_report`），Atriolum 当前列为 unknown 并存为 raw
   - **影响**: 反馈数据被保存但不被查询
   - **修复**: 将 `feedback` 加入 `KnownItemType`

3. **`raw_security` 类型**: CSP 安全报告，完全未处理
   - **影响**: CSP 违规报告丢失
   - **修复**: 添加 `raw_security` 到 KnownItemType，存为 raw

#### ❌ 关键差异

4. **压缩**: Node.js SDK 对 >32KB 的 payload 自动 gzip 压缩
   - Atriolum 已支持 gzip 解压 ✅
   - Browser SDK 不压缩（fetch API 限制）✅

5. **`sent_at` 时钟漂移校正**: JS SDK 在 envelope header 中发送 `sent_at` 字段
   - Atriolum 解析但不使用该字段
   - **影响**: 服务器与客户端时钟偏差大时，事件时间戳可能不准确
   - **修复**: 在事件 timestamp 上应用 `sent_at - server_received_at` 校正

6. **Breadcrumb 时间戳格式**: JS SDK 发送 Unix epoch 数字（`1704067200.0`），Python 发送 RFC 3339 字符串
   - Atriolum Breadcrumb.timestamp 使用 `serde_json::Value`，两种格式均可存储 ✅

---

## 3. C++ SDK (`sentry-native`)

### 发送的 Envelope Item 类型

| 类型 | Atriolum 处理 | 备注 |
|------|-------------|------|
| `event` | ✅ | |
| `transaction` | ✅ | |
| `session` | ✅ | |
| `attachment` | ✅ | |
| `client_report` | ✅ | |
| `log` | ✅ | |
| `user_feedback` | ⚠️ | 存为 raw |

### Minidump 处理（关键阻塞）

#### ❌ 缺少 `/api/{id}/minidump/` 端点

C++ SDK 的崩溃报告流程：

```
1. 应用崩溃
2. SDK 捕获 minidump（~100KB-10MB）
3. 将 minidump 作为 attachment 发送，attachment_type = "event.minidump"
4. 或使用 multipart/form-data POST 到 /api/{id}/minidump/
```

**当前状态**: Atriolum 没有 `/api/{id}/minidump/` 端点。

**影响**: C++ SDK 的核心崩溃报告功能**完全不可用**。

**修复方案**:
```
POST /api/{project_id}/minidump/
  Content-Type: multipart/form-data
  字段: upload_file_minidump (二进制), sentry (JSON 事件数据)

处理:
  1. 解析 multipart 请求
  2. 提取 minidump 二进制数据
  3. 提取 sentry JSON（包含事件元数据）
  4. 构造 Envelope：
     - event item: 来自 sentry JSON
     - attachment item: minidump 二进制，filename="minidump.dmp", attachment_type="event.minidump"
  5. 通过 IngestProcessor 处理
```

### 兼容性细节

#### ✅ 已兼容

- **Event 结构**: C++ SDK 发送标准 Event JSON，包含 contexts（os, device, trace）、exception、threads ✅
- **Context 自动填充**: `os`（name, version, build, kernel_version）、`device`（architecture）✅
- **Session 格式**: 与其他 SDK 相同的 `{sid, init, started, status, ...}` ✅
- **认证**: 标准 `X-Sentry-Auth` header ✅
- **platform 字段**: 发送 `"native"` ✅

#### ⚠️ 需要关注

1. **离线队列**: C++ SDK 有磁盘持久化队列，启动时会批量发送积压 envelope
   - Atriolum 需要能处理连续多个 POST 请求
   - 当前实现可以处理（每次请求独立） ✅

2. **gzip 压缩**: C++ SDK 可选 gzip（`SENTRY_TRANSPORT_COMPRESSION`）
   - Atriolum 已支持 gzip ✅

3. **Minidump 附件**: 当 minidump 通过 envelope attachment 发送时（不是单独端点），Atriolum 可以存储
   - 但 `attachment_type: "event.minidump"` 字段需要保留
   - **检查**: 当前 `ItemHeader` 有 `attachment_type` 字段 ✅，存储时保留 ✅

#### ❌ 需要修复

1. **缺少 `/api/{id}/minidump/` 端点** — **阻塞级**
2. **缺少 multipart/form-data 解析** — minidump 端点必需

---

## 跨 SDK 共性问题

### 1. 缺少 Rate Limit 响应

所有 SDK 都检查服务端返回的 rate limit 信息：

```
HTTP/1.1 429 Too Many Requests
X-Sentry-Rate-Limits: 60:transaction, 60:error
Retry-After: 60
```

Atriolum 当前不返回这些头。SDK 在无 rate limit 信息时会持续发送，可能导致服务器过载。

**优先级**: 中。MVP 阶段可暂不实现，但生产环境必须。

### 2. `sent_at` 时钟漂移校正

Envelope header 包含 `sent_at` 字段：

```json
{"event_id":"...","sent_at":"2026-04-13T10:30:00.123Z"}
```

SDK 期望服务器用 `sent_at` 与服务器收到时间对比，校正事件时间戳。Atriolum 当前忽略此字段。

**优先级**: 低。仅影响时间精度，不影响功能。

### 3. Client Report 处理

SDK 定期发送 `client_report`，包含丢弃事件的统计：

```json
{
  "timestamp": "...",
  "discarded_events": [
    {"reason": "queue_overflow", "category": "error", "quantity": 5}
  ]
}
```

Atriolum 存储 client_report 但不解析/汇总。这些数据对运维有价值（了解 SDK 端丢弃了多少事件）。

**优先级**: 低。MVP 阶段仅存储即可。

### 4. 缺少 `feedback` 和 `raw_security` KnownItemType

JS SDK 发送 `feedback`（用户反馈）和 `raw_security`（CSP 报告）类型。当前作为 unknown 存为 raw。

**优先级**: 低。数据不丢失，只是查询不便。

---

## 修复优先级排序

| 优先级 | 问题 | 状态 | 影响 | 涉及 SDK |
|--------|------|------|------|----------|
| **P0** | 添加 `/api/{id}/minidump/` 端点 | ✅ 已完成 | C++ SDK 崩溃报告完全不可用 | C++ |
| **P1** | 添加 rate limit 响应头 | ✅ 已完成 | 高频场景 SDK 不限速 | 全部 |
| **P2** | `feedback`、`raw_security` 加入 KnownItemType | ✅ 已完成 | 数据可查询 | JS |
| **P2** | `metric`、`trace_metric`、`user_feedback` 加入 KnownItemType | ✅ 已完成 | 数据可查询 | 全部 |
| **P1** | 添加 brotli 解压支持 | ✅ 已完成 | Python SDK brotli 压缩请求 | Python |
| **P2** | 添加 `/api/{id}/chunk-upload/` 端点 | ✅ 已完成 | JS SDK 回放分块上传 | JS |
| **P2** | `user_report` 独立存储 | ✅ 已完成 | 数据分类存储 | JS |
| **P3** | `sent_at` 时钟漂移校正 | ❌ 未做 | 事件时间精度 | 全部 |
| **P3** | 解析汇总 client_report | ❌ 未做 | 运维可见性 | 全部 |

---

## 已验证的 Python SDK 测试结果

| 测试项 | 结果 | 验证的数据 |
|--------|------|-----------|
| 异常捕获 (ZeroDivisionError) | ✅ | exception.type, exception.value, stacktrace.frames |
| 异常捕获 (KeyError) | ✅ | exception + stacktrace 完整解析 |
| 多级别消息 (fatal/error/warning/info/debug) | ✅ | level 字段正确存储 |
| Breadcrumbs | ✅ | values 数组，包含 category/message/data |
| Tags | ✅ | Map<String, Value> |
| Extra | ✅ | Map<String, Value> |
| User context | ✅ | id/email/username |
| Contexts (runtime, trace) | ✅ | CPython 版本、trace_id/span_id |
| Release + Environment | ✅ | |
| 结构化日志 | ✅ | JSONL 格式存储 |
| gzip 压缩 | ✅ | |
| 认证（X-Sentry-Auth） | ✅ | |
| 认证（DSN in envelope） | ✅ | |
| CORS | ✅ | |
| 自动创建项目 | ✅ | |
