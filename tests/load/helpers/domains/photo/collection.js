// tests/load/helpers/domains/photo/collection.js
// 收藏夹操作函数（collection_controller）

import http from "k6/http";
import { BASE_URL } from "../../common.js";
import { getSessionHeaders, maybeRefreshSession } from "../../session.js";

/**
 * 创建收藏夹
 * @param {string} name - 收藏夹名称
 * @param {string} [description] - 描述
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function createCollection(name, description = "") {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.post(
        `${BASE_URL}/photo/collections/`,
        JSON.stringify({ name, description }),
        { headers },
    );
    return {
        success: res.status === 200,
        duration: res.timings.duration,
        data: res.status === 200 ? res.json("data") : null,
    };
}

/**
 * 查询收藏夹列表
 * @param {number} pageSize - 每页数量
 * @returns {{ success: boolean, duration: number, data?: Array }}
 */
export function listCollections(pageSize = 10) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/photo/collections/?size=${pageSize}`, {
        headers,
    });
    return {
        success: res.status === 200,
        duration: res.timings.duration,
        data: res.status === 200 ? res.json("data") : null,
    };
}

/**
 * 更新收藏夹信息
 * @param {string} collectionId - 收藏夹 ID
 * @param {string} name - 新名称
 * @param {string} [description] - 新描述
 * @returns {{ success: boolean, duration: number }}
 */
export function updateCollection(collectionId, name, description = "") {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.patch(
        `${BASE_URL}/photo/collections/${collectionId}`,
        JSON.stringify({ name, description }),
        { headers },
    );
    return {
        success: res.status === 200,
        duration: res.timings.duration,
    };
}

/**
 * 删除收藏夹
 * @param {string} collectionId - 收藏夹 ID
 * @returns {{ success: boolean, duration: number }}
 */
export function deleteCollection(collectionId) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.del(
        `${BASE_URL}/photo/collections/${collectionId}`,
        null,
        { headers },
    );
    return {
        success: res.status === 200,
        duration: res.timings.duration,
    };
}
