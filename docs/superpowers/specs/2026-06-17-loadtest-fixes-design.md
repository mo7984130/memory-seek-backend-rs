# 负载测试修复设计文档

## 概述

修复负载测试代码中与实际 API 逻辑不符的问题，并进行优化。

## 问题分析

### 严重问题

1. **authHeaders 函数调用错误**
   - 所有 service 文件都错误地调用 `authHeaders(token)`
   - 实际需要两个参数：`uid` 和 `token`
   - 问题文件：user_service.js, photo_service.js, collection_service.js, collection_photo_service.js, comment_service.js

2. **login 函数返回值不完整**
   - common.js 中的 login 函数只返回 accessToken
   - 实际需要返回 uid、token 和 refreshToken

3. **photo_service.js 上传照片 Authorization 格式错误**
   - 使用了 `Bearer ${token}`
   - 应该是 `Bearer ${uid} ${token}`

### 潜在问题

1. **Makefile 中 auth 目标硬编码了 BASE_URL**
   - 第45行硬编码了 `http://8.148.75.72:7985`
   - 应该使用变量

2. **缺少 fixtures 目录**
   - photo_service.js 引用了 `../../fixtures/test.jpg`
   - 该目录可能不存在

## 解决方案

### 1. 修改 common.js 中的 login 函数

```javascript
export function login(account, password) {
    const res = http.post(`${BASE_URL}/auth/login`, JSON.stringify({
        account,
        password,
    }), {
        headers: { "Content-Type": "application/json" },
    });

    if (res.status === 200) {
        return {
            uid: res.json("data.id"),
            token: res.json("data.accessToken"),
            refreshToken: res.json("data.refreshToken"),
        };
    }

    console.error(`Login failed for ${account}: ${res.status} ${res.body}`);
    return null;
}
```

### 2. 修改所有 service 文件

更新所有 service 文件使用新的 login 返回值：

```javascript
// 旧代码
const token = login(account, password);
if (!token) return;
const headers = authHeaders(token);

// 新代码
const loginResult = login(account, password);
if (!loginResult) return;
const { uid, token, refreshToken } = loginResult;
const headers = authHeaders(uid, token);
```

**需要修改的文件：**
- user_service.js
- photo_service.js
- collection_service.js
- collection_photo_service.js
- comment_service.js

### 3. 修复 photo_service.js 上传照片的 Authorization 格式

```javascript
// 旧代码
headers: { 'Authorization': `Bearer ${token}` }

// 新代码
headers: { 'Authorization': `Bearer ${uid} ${token}` }
```

### 4. 修复 Makefile 中硬编码的 BASE_URL

```makefile
# 旧代码
k6 run --quiet \
    --iterations 1 \
    --vus 1 \
    -e BASE_URL=http://8.148.75.72:7985 \
    --out json=$(RESULTS_DIR)/auth_raw.json \
    --summary-export=$(RESULTS_DIR)/auth_summary.json \
    scripts/auth/auth_service.js

# 新代码
k6 run --quiet \
    --iterations 1 \
    --vus 1 \
    -e BASE_URL=$(BASE_URL) \
    --out json=$(RESULTS_DIR)/auth_raw.json \
    --summary-export=$(RESULTS_DIR)/auth_summary.json \
    scripts/auth/auth_service.js
```

### 5. 添加 fixtures 目录和测试图片

创建 `tests/load/fixtures/` 目录，并添加一个测试图片 `test.jpg`。

## 实现步骤

1. 修改 common.js 中的 login 函数，返回完整信息
2. 修改 user_service.js，使用新的 login 返回值
3. 修改 photo_service.js，修复 Authorization 格式
4. 修改 collection_service.js，使用新的 login 返回值
5. 修改 collection_photo_service.js，使用新的 login 返回值
6. 修改 comment_service.js，使用新的 login 返回值
7. 修复 Makefile 中硬编码的 BASE_URL
8. 添加 fixtures 目录和测试图片

## 验证方法

1. 运行单个测试验证修复：
   ```bash
   k6 run --iterations 1 --vus 1 -e BASE_URL=http://localhost:7985 scripts/auth/auth_service.js
   ```

2. 运行完整测试套件：
   ```bash
   make -C tests/load auth REMOTE_HOST=<IP>
   make -C tests/load user REMOTE_HOST=<IP>
   make -C tests/load photo REMOTE_HOST=<IP>
   ```

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| login 函数返回值变更影响其他代码 | 高 | 检查所有调用点，确保兼容性 |
| 测试图片过大影响测试性能 | 低 | 使用小尺寸测试图片 |
| Makefile 变量未定义 | 中 | 添加默认值和错误检查 |
