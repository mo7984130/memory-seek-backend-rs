// tests/load/scenarios/photo/face.js
// 人脸服务压测场景 — arrival-rate 负载模型
//
// 双模式(target/max), 会话跨迭代复用, 迭代 = 单个业务请求。
// 覆盖: 查询照片人脸 / 未分配人脸照片 / 归属调整(只读为主 + 轻写)。

import {
    getPhotoUserCredentials,
    printSummary,
} from "../../helpers/common.js";
import {
    initSession,
    getSession,
    maybeRefreshSession,
} from "../../helpers/session.js";
import {
    getFacesByPhotoId,
    getUnassignedFacePhotos,
    changeFaceBelonging,
} from "../../helpers/domains/photo/face.js";

export { printSummary as handleSummary };

const LOAD_MODE = __ENV.LOAD_MODE || "target";
const TARGET_RPS = parseInt(__ENV.TARGET_RPS || "500", 10);
const MAX_RPS = parseInt(__ENV.MAX_RPS || "2000", 10);
// photo 用户数限制并发上限(seed PHOTO_USERS=200)
const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "100", 10);
const MAX_VUS = parseInt(__ENV.MAX_VUS || "200", 10);
const DURATION = __ENV.DURATION || "2m";

const PHOTO_COUNT = parseInt(__ENV.PHOTO_COUNT || "4000", 10);
const FACES = parseInt(__ENV.FACES || "4000", 10);
const PERSONS = parseInt(__ENV.PERSONS || "200", 10);

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
        const { account, password } = getPhotoUserCredentials(__VU);
        initSession(account, password);
        return;
    }

    maybeRefreshSession();

    const r = Math.random();
    if (r < 0.4) {
        const photoId = (Math.floor(Math.random() * PHOTO_COUNT) % PHOTO_COUNT) + 1;
        getFacesByPhotoId(photoId);
    } else if (r < 0.7) {
        getUnassignedFacePhotos(32);
    } else {
        const faceId = (Math.floor(Math.random() * FACES) % FACES) + 1;
        const personId = (Math.floor(Math.random() * PERSONS) % PERSONS) + 1;
        changeFaceBelonging(faceId, personId);
    }
}
