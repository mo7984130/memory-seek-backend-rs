# Service 代码审查流程设计

## 目标

建立一套系统化的 service 函数审查流程，逐函数审查代码质量，发现问题后与用户协作修复，确保每个函数都经过充分审查。

## 审查范围

全面审查，覆盖以下 5 个维度：

| 类别 | 关注点 |
|------|--------|
| 正确性 | 逻辑错误、边界条件、竞态条件、数据一致性 |
| 安全性 | 注入漏洞、未校验输入、敏感数据泄露、权限检查 |
| 性能 | 不必要的 clone、多余的 allocation、阻塞调用、N+1 查询 |
| 错误处理 | unwrap/expect 使用、错误传播、错误上下文丢失、panic 风险 |
| Rust 惯用写法 | Option/Result 使用、生命周期、trait 设计、迭代器链 |

## 审查顺序

按依赖关系从底层到上层，同一 service 内按函数在文件中出现的顺序：

```
1. auth/auth_service.rs              (4 函数)
2. user/user_service.rs              (7 函数)
3. photo/comment_service.rs          (4 函数)
4. photo/collection_service.rs       (12 函数)
5. photo/timeline_stat_service.rs    (5 函数)
6. photo/photo_service.rs            (6 函数)
7. photo/feature_service.rs          (5 函数)
8. photo/face_service.rs             (11 函数)
```

总计 ~54 个函数。

## 单函数审查流程

```
开始审查函数
    │
    ▼
Claude 读取函数代码 + 相关依赖（mapper、entity、入参类型）
    │
    ▼
输出结构化审查报告
    │
    ▼
用户逐条确认：✅ 修 / ⏭ 跳过 / 💬 讨论
    │
    ▼
Claude 实施修复（逐个 edit，用户可见每步 diff）
    │
    ▼
用户确认修复结果
    │
    ▼
更新进度文件 → 进入下一个函数
```

## 审查报告格式

```markdown
## 🔍 审查: `service_name::function_name`

**文件**: `path/to/file.rs:L行号`
**签名**: `pub async fn foo(param: Type) -> Result<ReturnType>`
**职责**: 一句话描述

### 发现的问题

| # | 类别 | 级别 | 位置 | 描述 |
|---|------|------|------|------|
| 1 | 性能 | ⚠️ warning | L45 | 不必要的 `.clone()`，可改用引用 |
| 2 | 错误处理 | 🔴 critical | L52 | `.unwrap()` 可能 panic |
| 3 | 惯用写法 | 💡 suggestion | L68 | 可用 `.map()` 替代 match + 手动构造 |

### 修复建议

**问题 #1**: ...
**问题 #2**: ...
```

### 严重级别

| 级别 | 含义 | 默认动作 |
|------|------|----------|
| 🔴 critical | 可能导致 panic、数据丢失、安全漏洞 | 必须修复 |
| ⚠️ warning | 性能问题、不规范写法、潜在隐患 | 建议修复 |
| 💡 suggestion | 更优雅的写法、可读性改进 | 可选修复 |

## 进度跟踪

写入 `docs/code-review/service-review-progress.md`：

```markdown
# Service 代码审查进度

## 总览

| Service | 函数数 | 已审查 | 已修复 | 进度 |
|---------|--------|--------|--------|------|
| auth/auth_service.rs | 4 | 3 | 2 | ████████░░ 75% |
| user/user_service.rs | 7 | 0 | 0 | ░░░░░░░░░░ 0% |

## 审查记录

### auth_service.rs

#### ✅ login() — 2026-05-19
- 发现 3 个问题，修复 2 个，跳过 1 个
- 修复: L45 clone→引用, L52 unwrap→? 运算符
- 跳过: L68 match→map（当前可读性更好）

#### 🔍 register() — 审查中
...

#### ⏳ logout() — 待审查
```

## 使用方式

每次新会话中，用户可以通过以下命令启动或继续审查：

- **开始新审查**: "开始审查 service 代码"
- **继续审查**: "继续审查"（Claude 读取进度文件，定位到上次位置）
- **跳到特定函数**: "审查 auth_service 的 login 函数"
- **查看进度**: "查看审查进度"

## 设计决策

1. **逐函数而非逐文件**：粒度更细，每次审查范围可控，用户不会被大量问题淹没
2. **先诊断后修复**：用户对每个问题有完全控制权，避免误改
3. **进度文件持久化**：支持跨会话继续，可追溯审查历史
4. **结构化报告**：统一格式便于对比和回顾，严重级别帮助排优先级
