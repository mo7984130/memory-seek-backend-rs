// tests/load/helpers/domains/photo/person.js
// 人物操作函数(person_controller)

import http from "k6/http";
import { BASE_URL, logResult } from "../../common.js";
import { getSessionHeaders, maybeRefreshSession } from "../../session.js";

/**
 * 人物列表(游标分页, 按 face_count 倒序)
 * @param {number} [size] - 分页大小
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function getPersons(size = 32) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/photo/person?size=${size}`, { headers, tags: { name: "get_persons" } });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("get_persons", result);
    return result;
}

/**
 * 按关键词前缀搜索人物(完整名字或姓名首字母)
 * @param {string} keyword - 搜索关键词
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function searchPersons(keyword) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(
        `${BASE_URL}/photo/person/search?keyword=${encodeURIComponent(keyword)}`,
        { headers, tags: { name: "search_persons" } },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("search_persons", result);
    return result;
}

/**
 * 获取人物的照片列表
 * @param {number} personId - 人物 ID
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function getPersonPhotos(personId) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/photo/person/${personId}/photos`, {
        headers,
        tags: { name: "get_person_photos" },
    });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("get_person_photos", result);
    return result;
}

/**
 * 重命名人物
 * @param {number} personId - 人物 ID
 * @param {string} newName - 新名字
 * @returns {{ success: boolean, duration: number }}
 */
export function renamePerson(personId, newName) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.post(
        `${BASE_URL}/photo/person/${personId}/name`,
        JSON.stringify({ newName }),
        { headers, tags: { name: "rename_person" } },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("rename_person", result);
    return result;
}
