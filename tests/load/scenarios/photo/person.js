// tests/load/scenarios/photo/person.js
// 人物服务压测场景(person_controller): 列表 / 搜索 / 人物照片 / 重命名

import { sleep } from "k6";
import {
    getPhotoUserCredentials,
    recordResult,
    printSummary,
} from "../../helpers/common.js";

export { printSummary as handleSummary };

import { initSession, logout } from "../../helpers/session.js";
import {
    getPersons,
    searchPersons,
    getPersonPhotos,
    renamePerson,
} from "../../helpers/domains/photo/person.js";

// 数据量(与 seed.sh 对齐; 本地可经 -e 覆盖)
const PERSONS = parseInt(__ENV.PERSONS || "200");

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

function runPersonFlow() {
    const { account, password } = getPhotoUserCredentials(__VU);
    const session = initSession(account, password);
    if (!session) return;

    sleep(0.3);

    // 1. 人物列表
    let result = getPersons(32);
    recordResult("get_persons", result);

    sleep(0.3);

    // 2. 按首字母前缀搜索
    result = searchPersons("P_");
    recordResult("search_persons", result);

    sleep(0.3);

    // 3. 随机人物的照片列表
    const personId = (Math.floor(Math.random() * PERSONS) % PERSONS) + 1;
    result = getPersonPhotos(personId);
    recordResult("get_person_photos", result);

    sleep(0.3);

    // 4. 随机重命名人物(不删除数据, 可重复执行)
    result = renamePerson(personId, `LoadTest_${__VU}_${Date.now()}`);
    recordResult("rename_person", result);

    sleep(0.3);

    logout();
    sleep(0.5);
}

// ── 独立运行入口 ──

export default function () {
    runPersonFlow();
}

// ── 被统一入口调用的 exec 函数 ──

export function personExec() {
    runPersonFlow();
}
