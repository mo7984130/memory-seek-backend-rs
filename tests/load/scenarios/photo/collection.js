// tests/load/scenarios/photo/collection.js
// 收藏夹服务压测场景（collection_controller）

import { sleep } from "k6";
import {
    getPhotoUserCredentials,
    recordResult,
    printSummary,
} from "../../helpers/common.js";

export { printSummary as handleSummary };

import { initSession } from "../../helpers/session.js";
import {
    createCollection,
    listCollections,
    updateCollection,
    deleteCollection,
} from "../../helpers/domains/photo/collection.js";

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

function runCollectionFlow() {
    const { account, password } = getPhotoUserCredentials(__VU);
    const session = initSession(account, password);
    if (!session) return;

    sleep(0.3);

    // 1. 创建收藏夹
    let result = createCollection(`Collection ${__VU} ${Date.now()}`, "LoadTest");
    recordResult("create_collection", result);
    if (!result.success) return;
    const collectionId = result.data.id;

    sleep(0.3);

    // 2. 查询收藏夹列表
    result = listCollections();
    recordResult("list_collections", result);

    sleep(0.3);

    // 3. 更新收藏夹信息
    result = updateCollection(collectionId, `Updated ${__VU}`, "Updated desc");
    recordResult("update_collection", result);

    sleep(0.3);

    // 4. 删除收藏夹
    result = deleteCollection(collectionId);
    recordResult("delete_collection", result);

    sleep(0.5);
}

// ── 独立运行入口 ──

export default function () {
    runCollectionFlow();
}

// ── 被统一入口调用的 exec 函数 ──

export function collectionExec() {
    runCollectionFlow();
}
