// tests/load/scenarios/photo/comment_like.js
// 评论点赞服务压测场景（comment_like_controller）

import { sleep } from "k6";
import {
    getPhotoUserCredentials,
    recordResult,
    printSummary,
} from "../../helpers/common.js";

export { printSummary as handleSummary };

import { initSession } from "../../helpers/session.js";
import { listPhotos } from "../../helpers/domains/photo/photo.js";
import { createComment, listComments, deleteComment } from "../../helpers/domains/photo/comment.js";
import {
    likeComment,
    unlikeComment,
} from "../../helpers/domains/photo/comment_like.js";

// ── 独立运行时的 options ──

export const options = {
    stages: [
        { duration: "30s", target: 5 },
        { duration: "1m", target: 5 },
        { duration: "30s", target: 10 },
        { duration: "1m", target: 10 },
        { duration: "30s", target: 0 },
    ],
    thresholds: {
        http_req_duration: ["p(95)<500"],
        http_req_failed: ["rate<0.01"],
    },
};

// ── 核心逻辑 ──

function runCommentLikeFlow() {
    const { account, password } = getPhotoUserCredentials(__VU);
    const session = initSession(account, password);
    if (!session) return;

    sleep(0.3);

    // 获取一张照片 ID
    const photoListResult = listPhotos(1);
    if (!photoListResult.success || !photoListResult.data?.length) {
        return;
    }
    const photoId = photoListResult.data[0].id;

    sleep(0.3);

    // 创建一条评论（用于点赞）
    const commentResult = createComment(photoId, `Like target VU${__VU} ${Date.now()}`);
    if (!commentResult.success) return;
    const commentId = commentResult.data.id;

    sleep(0.3);

    // 1. 点赞评论
    let result = likeComment(photoId, commentId);
    recordResult("like_comment", result);

    sleep(0.3);

    // 2. 取消点赞
    result = unlikeComment(photoId, commentId);
    recordResult("unlike_comment", result);

    sleep(0.3);

    // 清理：删除评论
    deleteComment(photoId, commentId);

    sleep(0.5);
}

// ── 独立运行入口 ──

export default function () {
    runCommentLikeFlow();
}

// ── 被统一入口调用的 exec 函数 ──

export function commentLikeExec() {
    runCommentLikeFlow();
}
