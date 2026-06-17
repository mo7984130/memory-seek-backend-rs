// tests/load/scenarios/social.js
// 社交模块独立压测场景（收藏夹 + 评论）

import { sleep } from "k6";
import { Rate, Trend } from "k6/metrics";
import { getPhotoUserCredentials, recordResult } from "../helpers/common.js";
import { initSession, logout } from "../helpers/session.js";
import { listPhotos } from "../helpers/photo_ops.js";
import {
    createCollection,
    listCollections,
    deleteCollection,
} from "../helpers/collection_ops.js";
import {
    addPhotosToCollection,
    listCollectionPhotos,
    removePhotoFromCollection,
} from "../helpers/collection_photo_ops.js";
import {
    createComment,
    listComments,
    likeComment,
    unlikeComment,
    deleteComment,
} from "../helpers/comment_ops.js";

const socialErrorRate = new Rate("social_errors");
const socialDuration = new Trend("social_duration");

export const options = {
    stages: [
        { duration: "30s", target: 10 },
        { duration: "1m", target: 10 },
        { duration: "30s", target: 20 },
        { duration: "1m", target: 20 },
        { duration: "30s", target: 0 },
    ],
    thresholds: {
        http_req_duration: ["p(95)<500"],
        http_req_failed: ["rate<0.01"],
        social_errors: ["rate<0.01"],
    },
};

export default function () {
    const { account, password } = getPhotoUserCredentials(__VU);

    // 登录
    const session = initSession(account, password);
    if (!session) return;

    sleep(0.3);

    // 获取一张照片 ID（用于后续评论操作）
    const photoListResult = listPhotos(1);
    if (!photoListResult.success || !photoListResult.data || photoListResult.data.length === 0) {
        console.error("No photos available for social test");
        logout();
        return;
    }
    const photoId = photoListResult.data[0].id;

    sleep(0.3);

    // ── 收藏夹流程 ──

    // 1. 创建收藏夹
    let result = createCollection(`Social Test ${__VU} ${Date.now()}`, "LoadTest");
    recordResult("create_collection", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);
    if (!result.success) {
        logout();
        return;
    }
    const collectionId = result.data.id;

    sleep(0.3);

    // 2. 查询收藏夹列表
    result = listCollections();
    recordResult("list_collections", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);

    sleep(0.3);

    // 3. 添加照片到收藏夹
    result = addPhotosToCollection(collectionId, [photoId]);
    recordResult("add_photos_to_collection", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);

    sleep(0.3);

    // 4. 查询收藏夹照片列表
    result = listCollectionPhotos(collectionId);
    recordResult("list_collection_photos", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);

    sleep(0.3);

    // 5. 从收藏夹移除照片
    result = removePhotoFromCollection(collectionId, photoId);
    recordResult("remove_photo_from_collection", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);

    sleep(0.3);

    // 6. 删除收藏夹
    result = deleteCollection(collectionId);
    recordResult("delete_collection", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);

    sleep(0.3);

    // ── 评论流程 ──

    // 7. 发表评论
    result = createComment(photoId, `Social test comment from VU${__VU} at ${Date.now()}`);
    recordResult("create_comment", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);
    if (!result.success) {
        logout();
        return;
    }
    const commentId = result.data.id;

    sleep(0.3);

    // 8. 查询评论列表
    result = listComments(photoId);
    recordResult("list_comments", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);

    sleep(0.3);

    // 9. 点赞评论
    result = likeComment(photoId, commentId);
    recordResult("like_comment", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);

    sleep(0.3);

    // 10. 取消点赞
    result = unlikeComment(photoId, commentId);
    recordResult("unlike_comment", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);

    sleep(0.3);

    // 11. 删除评论
    result = deleteComment(photoId, commentId);
    recordResult("delete_comment", result);
    socialErrorRate.add(!result.success);
    socialDuration.add(result.duration);

    sleep(0.5);

    // 登出
    logout();

    sleep(0.5);
}
