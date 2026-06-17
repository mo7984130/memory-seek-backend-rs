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
    const res = http.post(`${BASE_URL}/photo/`, formData, {
        headers: { Authorization: headers.Authorization },
    });
    const result = {
        success: res.status === 200,
        duration: res.timings.duration,
        data: res.status === 200 ? res.json("data") : null,
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
    const res = http.get(`${BASE_URL}/photo/?size=${pageSize}`, { headers });
    const result = {
        success: res.status === 200,
        duration: res.timings.duration,
        data: res.status === 200 ? res.json("data") : null,
    };
    logResult("list_photos", result);
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
    const result = {
        success: res.status === 200,
        duration: res.timings.duration,
        data: res.status === 200 ? res.json("data") : null,
    };
    logResult("timeline_stats", result);
    return result;
}
