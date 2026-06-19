// tests/load/scenarios/photo/photo.js
// 照片服务压测场景（photo_service + timeline_stat_service）

import { sleep } from "k6";
import { SharedArray } from "k6/data";
import {
    getPhotoUserCredentials,
    recordResult,
    printSummary,
} from "../../helpers/common.js";

export { printSummary as handleSummary };

import { initSession, logout } from "../../helpers/session.js";
import {
    uploadPhoto,
    listPhotos,
    getTimelineStats,
} from "../../helpers/domains/photo/photo.js";

// 共享图片数据（所有 VU 共享同一份，节省内存）
const testImage = new SharedArray("test-image", function () {
    return [open("../../fixtures/test.jpg", "b")];
});

// ── 独立运行时的 options ──

export const options = {
    stages: [
        { duration: "30s", target: 10 },
        { duration: "1m", target: 10 },
        { duration: "30s", target: 20 },
        { duration: "1m", target: 20 },
        { duration: "30s", target: 0 },
    ],
    thresholds: {
        http_req_duration: ["p(95)<1000"],
        http_req_failed: ["rate<0.01"],
    },
};

// ── 核心逻辑（独立运行和被导入时共用） ──

function runPhotoFlow() {
    const { account, password } = getPhotoUserCredentials(__VU);

    // 登录
    const session = initSession(account, password);
    if (!session) return;

    sleep(0.3);

    // 1. 上传照片
    let result = uploadPhoto(testImage[0]);
    recordResult("upload_photo", result);
    if (!result.success) {
        console.error("Upload failed");
        return;
    }

    sleep(0.5);

    // 2. 查询照片列表
    result = listPhotos(20);
    recordResult("list_photos", result);

    sleep(0.5);

    // 3. 查询时间线统计
    result = getTimelineStats();
    recordResult("timeline_stats", result);

    sleep(0.5);

    // 4. 再次查询照片列表（模拟用户浏览）
    result = listPhotos(20);
    recordResult("list_photos", result);

    sleep(0.5);

    // 登出
    logout();

    sleep(0.5);
}

// ── 独立运行入口 ──

export default function () {
    runPhotoFlow();
}

// ── 被统一入口调用的 exec 函数 ──

export function photoExec() {
    runPhotoFlow();
}
