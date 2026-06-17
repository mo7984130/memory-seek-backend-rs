// tests/load/scenarios/photo.js
// 照片模块独立压测场景

import { sleep } from "k6";
import { Rate, Trend } from "k6/metrics";
import { SharedArray } from "k6/data";
import {
    getPhotoUserCredentials,
    recordResult,
} from "../helpers/common.js";
import { initSession, logout } from "../helpers/session.js";
import {
    uploadPhoto,
    listPhotos,
    getTimelineStats,
} from "../helpers/domains/photo/photo.js";

const photoErrorRate = new Rate("photo_errors");
const photoDuration = new Trend("photo_duration");

// 共享图片数据（所有 VU 共享同一份，节省内存）
const testImage = new SharedArray("test-image", function () {
    return [open("../fixtures/test.jpg", "b")];
});

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
        photo_errors: ["rate<0.01"],
    },
};

export default function () {
    const { account, password } = getPhotoUserCredentials(__VU);

    // 登录
    const session = initSession(account, password);
    if (!session) return;

    sleep(0.3);

    // 1. 上传照片
    let result = uploadPhoto(testImage[0]);
    recordResult("upload_photo", result);
    photoErrorRate.add(!result.success);
    photoDuration.add(result.duration);
    if (!result.success) {
        console.error(`Upload failed`);
        return;
    }

    sleep(0.5);

    // 2. 查询照片列表
    result = listPhotos(20);
    recordResult("list_photos", result);
    photoErrorRate.add(!result.success);
    photoDuration.add(result.duration);

    sleep(0.5);

    // 3. 查询时间线统计
    result = getTimelineStats();
    recordResult("timeline_stats", result);
    photoErrorRate.add(!result.success);
    photoDuration.add(result.duration);

    sleep(0.5);

    // 4. 再次查询照片列表（模拟用户浏览）
    result = listPhotos(20);
    recordResult("list_photos", result);
    photoErrorRate.add(!result.success);
    photoDuration.add(result.duration);

    sleep(0.5);

    // 登出
    logout();

    sleep(0.5);
}
