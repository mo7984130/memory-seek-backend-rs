// tests/load/helpers/domains/photo/photo.js
// 照片模块操作函数（photo_controller + timeline_stat_controller）

import http from "k6/http";
import { BASE_URL, logResult } from "../../common.js";
import { getSessionHeaders, maybeRefreshSession } from "../../session.js";

/**
 * 上传照片
 * @param {Uint8Array} imageBytes - 图片二进制数据
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function uploadPhoto(imageBytes) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const formData = {
        file: http.file(imageBytes, "test.jpg", "image/jpeg"),
    };
    const res = http.post(`${BASE_URL}/photo`, formData, {
        headers: { Authorization: headers.Authorization },
    });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("upload_photo", result);
    return result;
}

/**
 * 查询照片列表
 * @param {number} pageSize - 每页数量
 * @returns {{ success: boolean, duration: number, data?: Array }}
 */
export function listPhotos(pageSize = 20) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/photo?size=${pageSize}`, { headers });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data.records") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("list_photos", result);
    return result;
}

/**
 * 删除照片
 * @param {string[]} photoIds - 照片 ID 数组
 * @returns {{ success: boolean, duration: number }}
 */
export function deletePhotos(photoIds) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const body = JSON.stringify({ photoIds });
    const res = http.del(`${BASE_URL}/photo`, body, {
        headers: { ...headers, "Content-Type": "application/json" },
    });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("delete_photos", result);
    return result;
}

/**
 * 查询时间线统计
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function getTimelineStats() {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/photo/timeline/stats`, { headers });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("timeline_stats", result);
    return result;
}
