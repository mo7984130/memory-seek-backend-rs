// tests/load/scenarios/auth.js
// 认证模块压测场景 — arrival-rate 负载模型
//
// 双模式(通过 LOAD_MODE 切换):
//   target: constant-arrival-rate 固定目标 QPS(稳定对比)
//   max   : ramping-arrival-rate 逐步加压找系统上限
//
// 会话跨迭代复用: 每 VU 仅首次迭代登录(accessToken 2h 有效), 后续复用;
// 迭代 = 单个业务请求, rate 即请求 QPS。

import {
    getTestUserCredentials,
    printSummary,
} from "../helpers/common.js";
import {
    initSession,
    getSession,
    maybeRefreshSession,
    refreshSession,
    logout,
} from "../helpers/session.js";

export { printSummary as handleSummary };

const LOAD_MODE = __ENV.LOAD_MODE || "target";
const TARGET_RPS = parseInt(__ENV.TARGET_RPS || "1500", 10);
const MAX_RPS = parseInt(__ENV.MAX_RPS || "100000", 10);
const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "200", 10);
const MAX_VUS = parseInt(__ENV.MAX_VUS || "5000", 10);
const DURATION = __ENV.DURATION || "2m";

export const options = (() => {
    if (LOAD_MODE === "max") {
        const ramp = Math.max(1, Math.floor(MAX_RPS * 0.1));
        return {
            scenarios: {
                load: {
                    executor: "ramping-arrival-rate",
                    startRate: ramp,
                    timeUnit: "1s",
                    preAllocatedVUs: PRE_ALLOCATED_VUS,
                    maxVUs: MAX_VUS,
                    stages: [
                        { duration: "1m", target: ramp },
                        { duration: DURATION, target: MAX_RPS },
                        { duration: "1m", target: MAX_RPS },
                    ],
                },
            },
        };
    }
    return {
        scenarios: {
            load: {
                executor: "constant-arrival-rate",
                rate: TARGET_RPS,
                timeUnit: "1s",
                duration: "2m",
                preAllocatedVUs: PRE_ALLOCATED_VUS,
                maxVUs: MAX_VUS,
            },
        },
    };
})();

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
