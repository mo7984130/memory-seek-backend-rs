// 照片点赞操作函数（photo_like_controller）

import http from "k6/http";
import { BASE_URL, logResult } from "../../common.js";
import { getSessionHeaders, maybeRefreshSession } from "../../session.js";

/**
 * 点赞照片
 * @param {string} photoId - 照片 ID
 * @returns {{ success: boolean, duration: number }}
 */
export function likePhoto(photoId) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.post(
        `${BASE_URL}/photo/photos/${photoId}/like`,
        null,
        { headers, tags: { name: "like_photo" } },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("like_photo", result);
    return result;
}

/**
 * 查询当前用户点赞的照片
 * @param {number} pageSize - 每页数量
 * @returns {{ success: boolean, duration: number, data?: Array }}
 */
export function listLikedPhotos(pageSize = 32) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/photo/photos/liked?size=${pageSize}`, {
        headers,
        tags: { name: "list_liked_photos" },
    });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data.records") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("list_liked_photos", result);
    return result;
}

/**
 * 取消点赞照片
 * @param {string} photoId - 照片 ID
 * @returns {{ success: boolean, duration: number }}
 */
export function unlikePhoto(photoId) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.del(
        `${BASE_URL}/photo/photos/${photoId}/like`,
        null,
        { headers, tags: { name: "unlike_photo" } },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("unlike_photo", result);
    return result;
}
