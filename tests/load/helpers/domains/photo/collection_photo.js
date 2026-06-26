// tests/load/helpers/domains/photo/collection_photo.js
// 收藏夹-照片关联操作函数（collection_photo_controller）

import http from "k6/http";
import { BASE_URL, logResult } from "../../common.js";
import { getSessionHeaders, maybeRefreshSession } from "../../session.js";

/**
 * 添加照片到收藏夹
 * @param {string} collectionId - 收藏夹 ID
 * @param {string[]} photoIds - 照片 ID 数组
 * @returns {{ success: boolean, duration: number }}
 */
export function addPhotosToCollection(collectionId, photoIds) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.post(
        `${BASE_URL}/photo/collections/${collectionId}/photos`,
        JSON.stringify({ photoIds }),
        { headers },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("add_photos_to_collection", result);
    return result;
}

/**
 * 查询收藏夹照片列表
 * @param {string} collectionId - 收藏夹 ID
 * @param {number} pageSize - 每页数量
 * @returns {{ success: boolean, duration: number, data?: Array }}
 */
export function listCollectionPhotos(collectionId, pageSize = 10) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(
        `${BASE_URL}/photo/collections/${collectionId}/photos?size=${pageSize}`,
        { headers },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data.records") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("list_collection_photos", result);
    return result;
}

/**
 * 从收藏夹移除照片
 * @param {string} collectionId - 收藏夹 ID
 * @param {string} photoId - 照片 ID
 * @returns {{ success: boolean, duration: number }}
 */
export function removePhotoFromCollection(collectionId, photoId) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.del(
        `${BASE_URL}/photo/collections/${collectionId}/photos/${photoId}`,
        null,
        { headers },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("remove_photo_from_collection", result);
    return result;
}
