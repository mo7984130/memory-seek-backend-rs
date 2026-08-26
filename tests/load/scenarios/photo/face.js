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
    buildLoadOptions,
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

// photo 用户数限制并发上限(seed PHOTO_USERS=2000)
const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);

const PHOTO_COUNT = parseInt(__ENV.PHOTO_COUNT || "40000", 10);
const FACES = parseInt(__ENV.FACES || "40000", 10);
const PERSONS = parseInt(__ENV.PERSONS || "2000", 10);

export const options = buildLoadOptions({
    targetRps: 500,
    preAllocatedVUs: PRE_ALLOCATED_VUS,
    maxVUs: 2000,
});

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
