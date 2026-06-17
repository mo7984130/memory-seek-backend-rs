// tests/load/helpers/comment_ops.js
// 评论模块操作函数

import http from "k6/http";
import { BASE_URL } from "./common.js";
import { getSessionHeaders, maybeRefreshSession } from "./session.js";

/**
 * 发表评论
 * @param {string} photoId - 照片 ID
 * @param {string} content - 评论内容
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function createComment(photoId, content) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.post(
        `${BASE_URL}/photo/comment/${photoId}`,
        JSON.stringify({ content }),
        { headers },
    );
    return {
        success: res.status === 200,
        duration: res.timings.duration,
        data: res.status === 200 ? res.json("data") : null,
    };
}

/**
 * 查询评论列表
 * @param {string} photoId - 照片 ID
 * @param {number} pageSize - 每页数量
 * @returns {{ success: boolean, duration: number, data?: Array }}
 */
export function listComments(photoId, pageSize = 10) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(
        `${BASE_URL}/photo/comment/${photoId}?size=${pageSize}`,
        { headers },
    );
    return {
        success: res.status === 200,
        duration: res.timings.duration,
        data: res.status === 200 ? res.json("data") : null,
    };
}

/**
 * 删除评论
 * @param {string} photoId - 照片 ID
 * @param {string} commentId - 评论 ID
 * @returns {{ success: boolean, duration: number }}
 */
export function deleteComment(photoId, commentId) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.del(
        `${BASE_URL}/photo/comment/${photoId}/${commentId}`,
        null,
        { headers },
    );
    return {
        success: res.status === 200,
        duration: res.timings.duration,
    };
}

/**
 * 点赞评论
 * @param {string} photoId - 照片 ID
 * @param {string} commentId - 评论 ID
 * @returns {{ success: boolean, duration: number }}
 */
export function likeComment(photoId, commentId) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.post(
        `${BASE_URL}/photo/comment/${photoId}/${commentId}/like`,
        null,
        { headers },
    );
    return {
        success: res.status === 200,
        duration: res.timings.duration,
    };
}

/**
 * 取消点赞
 * @param {string} photoId - 照片 ID
 * @param {string} commentId - 评论 ID
 * @returns {{ success: boolean, duration: number }}
 */
export function unlikeComment(photoId, commentId) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.del(
        `${BASE_URL}/photo/comment/${photoId}/${commentId}/like`,
        null,
        { headers },
    );
    return {
        success: res.status === 200,
        duration: res.timings.duration,
    };
}
