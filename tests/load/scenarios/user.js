// tests/load/scenarios/user.js
// 用户模块压测场景 — arrival-rate 负载模型
//
// 双模式(target/max), 会话跨迭代复用, 迭代 = 单个业务请求。
// 说明: change_password 会使服务端登出并改变账号密码(需往返改回), 不适合
// 高频单 op 压测, 故迭代不含该操作(仅覆盖 get_me/change_nickname/登出路径)。

import {
    getTestUserCredentials,
    printSummary,
} from "../helpers/common.js";
import {
    initSession,
    getSession,
    maybeRefreshSession,
    logout,
} from "../helpers/session.js";
import { getMe, changeNickname } from "../helpers/domains/user/user.js";

export { printSummary as handleSummary };

const LOAD_MODE = __ENV.LOAD_MODE || "target";
const TARGET_RPS = parseInt(__ENV.TARGET_RPS || "500", 10);
const MAX_RPS = parseInt(__ENV.MAX_RPS || "100000", 10);
const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);
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
        const { account, password } = getTestUserCredentials(__VU);
        initSession(account, password);
        return;
    }

    maybeRefreshSession();

    const r = Math.random();
    if (r < 0.6) {
        getMe();
    } else if (r < 0.85) {
        changeNickname(`U${__VU}_${String(Date.now()).slice(-6)}`);
    } else {
        logout();
    }
}
