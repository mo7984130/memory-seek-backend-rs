// tests/load/scenarios/photo/face.js
// 人脸服务压测场景(face_controller): 查询照片人脸 / 未分配人脸照片 / 归属调整

import { sleep } from "k6";
import {
    getPhotoUserCredentials,
    recordResult,
    printSummary,
} from "../../helpers/common.js";

export { printSummary as handleSummary };

import { initSession, logout } from "../../helpers/session.js";
import {
    getFacesByPhotoId,
    getUnassignedFacePhotos,
    changeFaceBelonging,
} from "../../helpers/domains/photo/face.js";

// 数据量(与 seed.sh 对齐; 本地可经 -e 覆盖)
const PHOTO_COUNT = parseInt(__ENV.PHOTO_COUNT || "4000");
const FACES = parseInt(__ENV.FACES || "4000");
const PERSONS = parseInt(__ENV.PERSONS || "200");

// ── 独立运行时的 options ──

export const options = {
    stages: [
        { duration: "30s", target: 5 },
        { duration: "1m", target: 5 },
        { duration: "30s", target: 10 },
        { duration: "1m", target: 10 },
        { duration: "30s", target: 0 },
    ],
    thresholds: {
        http_req_duration: ["p(95)<500"],
        http_req_failed: ["rate<0.01"],
    },
};

// ── 核心逻辑 ──

function runFaceFlow() {
    const { account, password } = getPhotoUserCredentials(__VU);
    const session = initSession(account, password);
    if (!session) return;

    sleep(0.3);

    // 1. 查询随机照片的人脸列表
    const photoId = (Math.floor(Math.random() * PHOTO_COUNT) % PHOTO_COUNT) + 1;
    let result = getFacesByPhotoId(photoId);
    recordResult("get_faces_by_photo", result);

    sleep(0.3);

    // 2. 未分配人脸的照片列表
    result = getUnassignedFacePhotos(32);
    recordResult("get_unassigned_face_photos", result);

    sleep(0.3);

    // 3. 随机调整一张人脸归属(不删除数据, 可重复执行)
    const faceId = (Math.floor(Math.random() * FACES) % FACES) + 1;
    const personId = (Math.floor(Math.random() * PERSONS) % PERSONS) + 1;
    result = changeFaceBelonging(faceId, personId);
    recordResult("change_face_belonging", result);

    sleep(0.3);

    logout();
    sleep(0.5);
}

// ── 独立运行入口 ──

export default function () {
    runFaceFlow();
}

// ── 被统一入口调用的 exec 函数 ──

export function faceExec() {
    runFaceFlow();
}
