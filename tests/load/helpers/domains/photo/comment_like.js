// tests/load/helpers/domains/photo/comment_like.js
// 评论点赞操作函数（comment_like_controller）

import http from "k6/http";
import { BASE_URL, logResult } from "../../common.js";
import { getSessionHeaders, maybeRefreshSession } from "../../session.js";

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
    const result = {
        success: res.status === 200,
        duration: res.timings.duration,
    };
    logResult("like_comment", result);
    return result;
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
    const result = {
        success: res.status === 200,
        duration: res.timings.duration,
    };
    logResult("unlike_comment", result);
    return result;
}
