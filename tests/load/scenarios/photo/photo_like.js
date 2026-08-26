// 照片点赞服务压测场景 — arrival-rate 负载模型
//
// 完整流程迭代(点赞/查询已点赞/取消点赞，自洽且不积累数据)，
// 会话跨迭代复用。

import {
    getPhotoUserCredentials,
    pickPhotoUserPhotoId,
    recordResult,
    printSummary,
    buildLoadOptions,
    setupPreLogin,
    sessionFromData,
} from "../../helpers/common.js";
import {
    setSession,
    initSession,
    maybeRefreshSession,
} from "../../helpers/session.js";
import {
    likePhoto,
    listLikedPhotos,
    unlikePhoto,
} from "../../helpers/domains/photo/photo_like.js";

export { printSummary as handleSummary };

const PRE_ALLOCATED_VUS = parseInt(__ENV.PRE_ALLOCATED_VUS || "300", 10);

export const options = buildLoadOptions({
    targetRps: 50,
    preAllocatedVUs: PRE_ALLOCATED_VUS,
    maxVUs: 2000,
});

export function setup() {
    return setupPreLogin(getPhotoUserCredentials, PRE_ALLOCATED_VUS);
}

function runPhotoLikeFlow(data) {
    const session = sessionFromData(data, __VU);
    if (session) {
        setSession(session);
    } else {
        const { account, password } = getPhotoUserCredentials(__VU);
        initSession(account, password);
        return;
    }
    maybeRefreshSession();

    const photoId = pickPhotoUserPhotoId(__VU);
    let result = likePhoto(photoId);
    recordResult("like_photo", result);
    if (!result.success) return;

    result = listLikedPhotos();
    recordResult("list_liked_photos", result);

    result = unlikePhoto(photoId);
    recordResult("unlike_photo", result);
}

export default function (data) {
    runPhotoLikeFlow(data);
}

export function photoLikeExec(data) {
    runPhotoLikeFlow(data);
}
