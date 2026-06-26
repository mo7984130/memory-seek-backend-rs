// tests/load/helpers/domains/photo/collection.js
// 收藏夹操作函数（collection_controller）

import http from "k6/http";
import { BASE_URL, logResult } from "../../common.js";
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
        `${BASE_URL}/photo/collections`,
        JSON.stringify({ name, description }),
        { headers },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("create_collection", result);
    return result;
}

/**
 * 查询收藏夹列表
 * @returns {{ success: boolean, duration: number, data?: Array }}
 */
export function listCollections() {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/photo/collections`, {
        headers,
    });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("list_collections", result);
    return result;
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
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("update_collection", result);
    return result;
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
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("delete_collection", result);
    return result;
}
