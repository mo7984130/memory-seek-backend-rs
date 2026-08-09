// tests/load/scenarios/user.js
// 用户模块压测场景 — arrival-rate 负载模型, setup 预登录
//
// 双模式(target/max), 会话由 setup 预登录(login 不计入压测窗口),
// 迭代 = 单个业务请求(get_me / change_nickname), 会话全程复用。
// 说明: change_password 会使服务端登出并改变账号密码, 不适合高频压测, 故不包含。

import {
    getTestUserCredentials,
    setupPreLogin,
    sessionFromData,
    recordResult,
    printSummary,
} from "../helpers/common.js";
import {
    setSession,
    initSession,
    maybeRefreshSession,
} from "../helpers/session.js";
import { getMe, changeNickname } from "../helpers/domains/user/user.js";

export { printSummary as handleSummary };

const LOAD_MODE = __ENV.LOAD_MODE || "target";
const TARGET_RPS = parseInt(__ENV.TARGET_RPS || "300", 10);
const MAX_RPS = parseInt(__ENV.MAX_RPS || "100000", 10);
const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);
const MAX_VUS = parseInt(__ENV.MAX_VUS || "5000", 10);
const DURATION = __ENV.DURATION || "2m";

export const options = (() => {
    if (LOAD_MODE === "max") {
        const ramp = Math.max(1, Math.floor(MAX_RPS * 0.1));
        return {
            setupTimeout: "180s",
            scenarios: {
                load: {
                    executor: "ramping-arrival-rate",
                    startRate: ramp,
                    timeUnit: "1s",
                    preAllocatedVUs: PRE_ALLOCATED_VUS,
                    maxVUs: MAX_VUS,
                    stages: [
                        { duration: "1m", target: ramp },
                        { duration: "2m", target: MAX_RPS },
                        { duration: "1m", target: MAX_RPS },
                    ],
                },
            },
        };
    }
    return {
        setupTimeout: "180s",
        scenarios: {
            load: {
                executor: "constant-arrival-rate",
                rate: TARGET_RPS,
                timeUnit: "1s",
                duration: DURATION,
                preAllocatedVUs: PRE_ALLOCATED_VUS,
                maxVUs: MAX_VUS,
            },
        },
    };
})();

// setup 预登录: login 不计入压测窗口
export function setup() {
    return setupPreLogin(getTestUserCredentials, PRE_ALLOCATED_VUS);
}

export default function (data) {
    const session = sessionFromData(data, __VU);
    if (session) {
        setSession(session);
    } else {
        // 兜底: 预登录缺失(VU 超出预分配或登录失败), 现场登录
        const { account, password } = getTestUserCredentials(__VU);
        initSession(account, password);
        return;
    }

    maybeRefreshSession();

    if (Math.random() < 0.6) {
        recordResult("get_me", getMe());
    } else {
        recordResult("change_nickname", changeNickname(`U${__VU}_${String(Date.now()).slice(-6)}`));
    }
}
