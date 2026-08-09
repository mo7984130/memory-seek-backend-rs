// tests/load/scenarios/photo/comment.js
// 评论服务压测场景 — arrival-rate 负载模型
//
// 完整流程迭代(发评论/查列表/删评论 自洽), 会话跨迭代复用。

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
import {
    createComment,
    listComments,
    deleteComment,
} from "../../helpers/domains/photo/comment.js";

export { printSummary as handleSummary };

export const options = buildLoadOptions({
    targetRps: 30,
    maxRps: 150,
    preAllocatedVUs: 300,
    maxVUs: 2000,
});

// ── 核心逻辑 ──

function runCommentFlow() {
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

    let result = createComment(photoId, `Comment VU${__VU} ${Date.now()}`);
    recordResult("create_comment", result);
    if (!result.success) return;
    const commentId = result.data.id;

    result = listComments(photoId);
    recordResult("list_comments", result);

    result = deleteComment(photoId, commentId);
    recordResult("delete_comment", result);
}

// ── 独立运行入口 ──

export default function () {
    runCommentFlow();
}

// ── 被统一入口(photo.js)调用的 exec 函数 ──

export function commentExec() {
    runCommentFlow();
}
