// tests/load/scenarios/photo/face.js
// 人脸服务压测场景 — arrival-rate 负载模型, setup 预登录
//
// 双模式(target/max), 会话由 setup 预登录(login 不计入压测窗口),
// 迭代 = 单个业务请求(查询照片人脸 / 未分配人脸照片 / 归属调整)。

import {
    getPhotoUserCredentials,
    setupPreLogin,
    sessionFromData,
    recordResult,
    printSummary,
} from "../../helpers/common.js";
import {
    setSession,
    initSession,
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
const MAX_RPS = parseInt(__ENV.MAX_RPS || "100000", 10);
// photo 用户数限制并发上限(seed PHOTO_USERS=2000)
const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);
const MAX_VUS = parseInt(__ENV.MAX_VUS || "2000", 10);
const DURATION = __ENV.DURATION || "2m";

const PHOTO_COUNT = parseInt(__ENV.PHOTO_COUNT || "40000", 10);
const FACES = parseInt(__ENV.FACES || "40000", 10);
const PERSONS = parseInt(__ENV.PERSONS || "2000", 10);

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
    return setupPreLogin(getPhotoUserCredentials, PRE_ALLOCATED_VUS);
}

export default function (data) {
    const session = sessionFromData(data, __VU);
    if (session) {
        setSession(session);
    } else {
        const { account, password } = getPhotoUserCredentials(__VU);
        initSession(account, password);
        return;
    }

    maybeRefreshSession();

    const r = Math.random();
    if (r < 0.4) {
        const photoId = (Math.floor(Math.random() * PHOTO_COUNT) % PHOTO_COUNT) + 1;
        recordResult("get_faces_by_photo", getFacesByPhotoId(photoId));
    } else if (r < 0.7) {
        recordResult("get_unassigned_face_photos", getUnassignedFacePhotos(32));
    } else {
        const faceId = (Math.floor(Math.random() * FACES) % FACES) + 1;
        const personId = (Math.floor(Math.random() * PERSONS) % PERSONS) + 1;
        recordResult("change_face_belonging", changeFaceBelonging(faceId, personId));
    }
}
