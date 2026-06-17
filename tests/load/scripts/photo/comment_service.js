// tests/load/scripts/photo/comment_service.js
// 评论模块压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getPhotoUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const commentErrorRate = new Rate('comment_errors');
const commentDuration = new Trend('comment_duration');

export const options = {
  stages: [
    { duration: '30s', target: 10 },
    { duration: '1m', target: 10 },
    { duration: '30s', target: 20 },
    { duration: '1m', target: 20 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
    comment_errors: ['rate<0.01'],
  },
};

export function setup() {
  return {};
}

export default function () {
  const { account, password } = getPhotoUserCredentials(__VU);

  // 登录获取 token
  const loginResult = login(account, password);
  if (!loginResult) return;

  const { uid, token, refreshToken } = loginResult;
  const headers = authHeaders(uid, token);

  // 先获取照片列表，取一个 photo_id
  const listRes = http.get(`${BASE_URL}/photo/?size=1`, { headers });
  if (listRes.status !== 200 || !listRes.json('data') || listRes.json('data').length === 0) {
    console.error('No photos available for commenting');
    return;
  }

  const photoId = listRes.json('data')[0].id;
  if (!photoId) {
    console.error('Could not get photo ID');
    return;
  }

  sleep(0.3);

  // 1. 发表评论
  const createRes = http.post(`${BASE_URL}/photo/comment/${photoId}`, JSON.stringify({
    content: `LoadTest comment from VU${__VU} at ${Date.now()}`,
  }), { headers });

  check(createRes, {
    'create comment status is 200': (r) => r.status === 200,
  });

  commentErrorRate.add(createRes.status !== 200);
  commentDuration.add(createRes.timings.duration);

  if (createRes.status !== 200) {
    console.error(`Create comment failed: ${createRes.body}`);
    return;
  }

  const commentId = createRes.json('data.id');

  sleep(0.3);

  // 2. 查询评论列表
  const listCommentRes = http.get(`${BASE_URL}/photo/comment/${photoId}?size=10`, { headers });

  check(listCommentRes, {
    'list comments status is 200': (r) => r.status === 200,
  });

  commentErrorRate.add(listCommentRes.status !== 200);
  commentDuration.add(listCommentRes.timings.duration);

  sleep(0.3);

  // 3. 点赞评论
  if (commentId) {
    const likeRes = http.post(`${BASE_URL}/photo/comment/${photoId}/${commentId}/like`, null, { headers });

    check(likeRes, {
      'like comment status is 200': (r) => r.status === 200,
    });

    commentErrorRate.add(likeRes.status !== 200);

    sleep(0.3);

    // 4. 取消点赞
    const unlikeRes = http.del(`${BASE_URL}/photo/comment/${photoId}/${commentId}/like`, null, { headers });

    check(unlikeRes, {
      'unlike comment status is 200': (r) => r.status === 200,
    });

    commentErrorRate.add(unlikeRes.status !== 200);

    sleep(0.3);

    // 5. 删除评论
    const deleteRes = http.del(`${BASE_URL}/photo/comment/${photoId}/${commentId}`, null, { headers });

    check(deleteRes, {
      'delete comment status is 200': (r) => r.status === 200,
    });

    commentErrorRate.add(deleteRes.status !== 200);
  }

  sleep(1);
}
