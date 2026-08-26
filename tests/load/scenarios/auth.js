// tests/load/scenarios/auth.js
// 认证模块压测场景 — arrival-rate 负载模型
//
// 使用固定到达率；容量搜索由 CI 编排层逐档调用本场景。
//
// 会话跨迭代复用: 每 VU 仅首次迭代登录(accessToken 2h 有效), 后续复用;
// 迭代 = 单个业务请求, rate 即请求 QPS。

import {
    getTestUserCredentials,
    printSummary,
    buildLoadOptions,
} from "../helpers/common.js";
import {
    initSession,
    getSession,
    maybeRefreshSession,
    refreshSession,
    logout,
} from "../helpers/session.js";

export { printSummary as handleSummary };

const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "500", 10);

export const options = buildLoadOptions({
    targetRps: 400,
    preAllocatedVUs: PRE_ALLOCATED_VUS,
    maxVUs: 5000,
});

export default function () {
    if (!getSession()) {
        // 首迭代或登出后: 重新登录(initSession 内部已记录 login 指标)
        const { account, password } = getTestUserCredentials(__VU);
        initSession(account, password);
        return;
    }

    maybeRefreshSession();

    // 90% 续期(模拟长期在线), 10% 登出(下一迭代触发登录, 覆盖 login 路径)
    if (Math.random() < 0.1) {
        logout();
    } else {
        refreshSession();
    }
}
