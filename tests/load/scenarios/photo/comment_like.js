// tests/load/scenarios/photo/comment_like.js
// 评论点赞服务压测场景 — arrival-rate 负载模型
//
// 完整流程迭代(建评论/点赞/取消点赞/删评论 自洽), 会话跨迭代复用。

import {
    getPhotoUserCredentials,
    recordResult,
    printSummary,
    buildLoadOptions,
} from "../../helpers/common.js";
import {
    initSession,
    getSession,
    maybeRefreshSession,
} from "../../helpers/session.js";
import { listPhotos } from "../../helpers/domains/photo/photo.js";
import { createComment, deleteComment } from "../../helpers/domains/photo/comment.js";
import {
    likeComment,
    unlikeComment,
} from "../../helpers/domains/photo/comment_like.js";

export { printSummary as handleSummary };

export const options = buildLoadOptions({
    targetRps: 30,
    maxRps: 150,
    preAllocatedVUs: 300,
    maxVUs: 2000,
});

// ── 核心逻辑 ──

function runCommentLikeFlow() {
    if (!getSession()) {
        const { account, password } = getPhotoUserCredentials(__VU);
        initSession(account, password);
        return;
    }
    maybeRefreshSession();

    const photoListResult = listPhotos(1);
    if (!photoListResult.success || !photoListResult.data?.length) {
        return;
    }
    const photoId = photoListResult.data[0].id;

    const commentResult = createComment(photoId, `Like target VU${__VU} ${Date.now()}`);
    if (!commentResult.success) return;
    const commentId = commentResult.data.id;

    let result = likeComment(photoId, commentId);
    recordResult("like_comment", result);

    result = unlikeComment(photoId, commentId);
    recordResult("unlike_comment", result);

    deleteComment(photoId, commentId);
}

// ── 独立运行入口 ──

export default function () {
    runCommentLikeFlow();
}

// ── 被统一入口(photo.js)调用的 exec 函数 ──

export function commentLikeExec() {
    runCommentLikeFlow();
}
