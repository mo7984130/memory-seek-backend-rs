// tests/load/scenarios/photo.js
// Photo 模块统一压测入口 — 按服务拆分的多 scenario

import { printSummary } from "../helpers/common.js";

export { printSummary as handleSummary };

// 重新导出 exec 函数（k6 scenarios 要求顶层 export）
export { photoExec } from "./photo/photo.js";
export { collectionExec } from "./photo/collection.js";
export { collectionPhotoExec } from "./photo/collection_photo.js";
export { commentExec } from "./photo/comment.js";
export { commentLikeExec } from "./photo/comment_like.js";

export const options = {
    scenarios: {
        photo_service: {
            exec: "photoExec",
            executor: "ramping-vus",
            startVUs: 0,
            stages: [
                { duration: "30s", target: 10 },
                { duration: "1m", target: 10 },
                { duration: "30s", target: 20 },
                { duration: "1m", target: 20 },
                { duration: "30s", target: 0 },
            ],
        },
        collection_service: {
            exec: "collectionExec",
            executor: "ramping-vus",
            startVUs: 0,
            stages: [
                { duration: "30s", target: 5 },
                { duration: "1m", target: 5 },
                { duration: "30s", target: 10 },
                { duration: "1m", target: 10 },
                { duration: "30s", target: 0 },
            ],
        },
        collection_photo_service: {
            exec: "collectionPhotoExec",
            executor: "ramping-vus",
            startVUs: 0,
            stages: [
                { duration: "30s", target: 5 },
                { duration: "1m", target: 5 },
                { duration: "30s", target: 10 },
                { duration: "1m", target: 10 },
                { duration: "30s", target: 0 },
            ],
        },
        comment_service: {
            exec: "commentExec",
            executor: "ramping-vus",
            startVUs: 0,
            stages: [
                { duration: "30s", target: 10 },
                { duration: "1m", target: 10 },
                { duration: "30s", target: 15 },
                { duration: "1m", target: 15 },
                { duration: "30s", target: 0 },
            ],
        },
        comment_like_service: {
            exec: "commentLikeExec",
            executor: "ramping-vus",
            startVUs: 0,
            stages: [
                { duration: "30s", target: 5 },
                { duration: "1m", target: 5 },
                { duration: "30s", target: 10 },
                { duration: "1m", target: 10 },
                { duration: "30s", target: 0 },
            ],
        },
    },
    thresholds: {
        http_req_duration: ["p(95)<500"],
        http_req_failed: ["rate<0.02"],
    },
};

// k6 要求 default export 存在，但 scenarios 模式下不使用
export default function () {}
