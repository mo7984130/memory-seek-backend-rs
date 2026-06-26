// tests/load/scenarios/photo/collection_photo.js
// 收藏夹-照片关联服务压测场景（collection_photo_controller）

import { sleep } from "k6";
import {
    getPhotoUserCredentials,
    recordResult,
    printSummary,
} from "../../helpers/common.js";

export { printSummary as handleSummary };

import { initSession } from "../../helpers/session.js";
import { listPhotos } from "../../helpers/domains/photo/photo.js";
import { createCollection, deleteCollection } from "../../helpers/domains/photo/collection.js";
import {
    addPhotosToCollection,
    listCollectionPhotos,
    removePhotoFromCollection,
} from "../../helpers/domains/photo/collection_photo.js";

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

function runCollectionPhotoFlow() {
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

    // 创建一个收藏夹（用于关联操作）
    const collResult = createCollection(`CP ${__VU} ${Date.now()}`, "LoadTest");
    if (!collResult.success) return;
    const collectionId = collResult.data.id;

    sleep(0.3);

    // 1. 添加照片到收藏夹
    let result = addPhotosToCollection(collectionId, [photoId]);
    recordResult("add_photos_to_collection", result);

    sleep(0.3);

    // 2. 查询收藏夹照片列表
    result = listCollectionPhotos(collectionId);
    recordResult("list_collection_photos", result);

    sleep(0.3);

    // 3. 从收藏夹移除照片
    result = removePhotoFromCollection(collectionId, photoId);
    recordResult("remove_photo_from_collection", result);

    sleep(0.3);

    // 清理：删除收藏夹
    deleteCollection(collectionId);

    sleep(0.5);
}

// ── 独立运行入口 ──

export default function () {
    runCollectionPhotoFlow();
}

// ── 被统一入口调用的 exec 函数 ──

export function collectionPhotoExec() {
    runCollectionPhotoFlow();
}
