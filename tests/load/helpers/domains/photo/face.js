// tests/load/helpers/domains/photo/face.js
// 人脸操作函数(face_controller)

import http from "k6/http";
import { BASE_URL, logResult } from "../../common.js";
import { getSessionHeaders, maybeRefreshSession } from "../../session.js";

/**
 * 获取某张照片的人脸列表
 * @param {number} photoId - 照片 ID
 * @returns {{ success: boolean, duration: number, data?: Array }}
 */
export function getFacesByPhotoId(photoId) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/photo/face/photo/${photoId}`, { headers });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("get_faces_by_photo", result);
    return result;
}

/**
 * 获取包含未分配人脸的照片列表(游标分页)
 * @param {number} [size] - 分页大小
 * @returns {{ success: boolean, duration: number, data?: Object }}
 */
export function getUnassignedFacePhotos(size = 32) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.get(`${BASE_URL}/photo/face/unassigned-photos?size=${size}`, {
        headers,
    });
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        data: ok ? res.json("data") : null,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("get_unassigned_face_photos", result);
    return result;
}

/**
 * 修改人脸归属: 将人脸移动到指定人物
 * @param {number} faceId - 人脸 ID
 * @param {number} personId - 人物 ID
 * @returns {{ success: boolean, duration: number }}
 */
export function changeFaceBelonging(faceId, personId) {
    maybeRefreshSession();
    const headers = getSessionHeaders();
    const res = http.post(
        `${BASE_URL}/photo/face/feature/${faceId}/belonging/${personId}`,
        null,
        { headers },
    );
    const ok = res.status === 200;
    const result = {
        success: ok,
        duration: res.timings.duration,
        error: ok ? undefined : { status: res.status, body: res.body },
    };
    logResult("change_face_belonging", result);
    return result;
}
