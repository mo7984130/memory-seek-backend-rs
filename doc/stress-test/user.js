import http from "k6/http";
import { check, sleep } from "k6";
import { config } from "./config.js";
import {
  ErrorTypes,
  PerformanceThresholds,
  parseJsonSafely,
  makeRequest,
  createErrorResult,
  createSuccessResult,
  logPerformance,
} from "./utils.js";
import { login, getAuthHeaders } from "./auth.js";
import { generateRandomPassword, randomSleep } from "./utils/password.js";

export function getUserInfo(userId, accessToken) {
  const url = `${config.baseUrl}/user/info`;
  const params = {
    headers: getAuthHeaders(userId, accessToken),
    timeout: config.timeout,
  };

  let res;
  try {
    res = makeRequest("GET", url, null, params);
  } catch (e) {
    console.error(`获取用户信息请求失败: ${e.message}`);
    return createErrorResult(ErrorTypes.REQUEST_FAILED, {
      message: e.message,
      userId,
    });
  }

  const duration = res.timings.duration;
  const checks = check(res, {
    获取用户信息状态码为200: (r) => r.status === 200,
  });

  if (!checks) {
    logPerformance("getUserInfo", duration, false);
    return createErrorResult(ErrorTypes.CHECK_FAILED, {
      status: res.status,
      duration,
      userId,
    });
  }

  const body = parseJsonSafely(res.body, "getUserInfo");
  if (!body) {
    return createErrorResult(ErrorTypes.INVALID_JSON, {
      status: res.status,
      userId,
    });
  }

  logPerformance("getUserInfo", duration, true);
  return createSuccessResult({
    userInfo: body.data,
  });
}

export function changeNickname(userId, accessToken, newNickname) {
  const url = `${config.baseUrl}/user/nickname`;
  const payload = JSON.stringify({ newNickname: newNickname });
  const params = {
    headers: getAuthHeaders(userId, accessToken),
    timeout: config.timeout,
  };

  let res;
  try {
    res = makeRequest("POST", url, payload, params);
  } catch (e) {
    console.error(`修改昵称请求失败: ${e.message}`);
    return createErrorResult(ErrorTypes.REQUEST_FAILED, {
      message: e.message,
      userId,
    });
  }

  const duration = res.timings.duration;
  const checks = check(res, {
    修改昵称状态码为200: (r) => r.status === 200,
  });

  if (!checks) {
    logPerformance("changeNickname", duration, false);
    return createErrorResult(ErrorTypes.CHECK_FAILED, {
      status: res.status,
      duration,
      userId,
    });
  }

  const body = parseJsonSafely(res.body, "changeNickname");
  if (!body) {
    return createErrorResult(ErrorTypes.INVALID_JSON, {
      status: res.status,
      userId,
    });
  }

  logPerformance("changeNickname", duration, true);
  return createSuccessResult({
    nickname: body.data,
  });
}

export function generateInviterCode(userId, accessToken) {
  const url = `${config.baseUrl}/user/inviter-code`;
  const params = {
    headers: getAuthHeaders(userId, accessToken),
    timeout: config.timeout,
  };

  let res;
  try {
    res = makeRequest("GET", url, null, params);
  } catch (e) {
    console.error(`生成邀请码请求失败: ${e.message}`);
    return createErrorResult(ErrorTypes.REQUEST_FAILED, {
      message: e.message,
      userId,
    });
  }

  const duration = res.timings.duration;
  const checks = check(res, {
    生成邀请码状态码为200: (r) => r.status === 200,
  });

  if (!checks) {
    logPerformance("generateInviterCode", duration, false);
    return createErrorResult(ErrorTypes.CHECK_FAILED, {
      status: res.status,
      duration,
      userId,
    });
  }

  const body = parseJsonSafely(res.body, "generateInviterCode");
  if (!body) {
    return createErrorResult(ErrorTypes.INVALID_JSON, {
      status: res.status,
      userId,
    });
  }

  logPerformance("generateInviterCode", duration, true);
  return createSuccessResult({
    inviterCode: body.data?.inviterCode,
    expireAt: body.data?.expireAt,
  });
}

export function changePassword(userId, accessToken, oldPassword, newPassword) {
  const url = `${config.baseUrl}/user/password`;
  const payload = JSON.stringify({
    oldPassword: oldPassword,
    newPassword: newPassword,
  });
  const params = {
    headers: getAuthHeaders(userId, accessToken),
    timeout: config.timeout,
  };

  let res;
  try {
    res = makeRequest("POST", url, payload, params);
  } catch (e) {
    console.error(`修改密码请求失败: ${e.message}`);
    return createErrorResult(ErrorTypes.REQUEST_FAILED, {
      message: e.message,
      userId,
    });
  }

  const duration = res.timings.duration;
  const checks = check(res, {
    修改密码状态码为200: (r) => r.status === 200,
  });

  if (!checks) {
    logPerformance("changePassword", duration, false);
    return createErrorResult(ErrorTypes.CHECK_FAILED, {
      status: res.status,
      duration,
      userId,
    });
  }

  const body = parseJsonSafely(res.body, "changePassword");
  if (!body) {
    return createErrorResult(ErrorTypes.INVALID_JSON, {
      status: res.status,
      userId,
    });
  }

  logPerformance("changePassword", duration, true);
  return createSuccessResult({});
}

export function logout(userId, accessToken) {
  const url = `${config.baseUrl}/user/logout`;
  const params = {
    headers: getAuthHeaders(userId, accessToken),
    timeout: config.timeout,
  };

  let res;
  try {
    res = makeRequest("GET", url, null, params);
  } catch (e) {
    console.error(`登出请求失败: ${e.message}`);
    return createErrorResult(ErrorTypes.REQUEST_FAILED, {
      message: e.message,
      userId,
    });
  }

  const duration = res.timings.duration;
  const checks = check(res, {
    登出状态码为200: (r) => r.status === 200,
  });

  if (!checks) {
    logPerformance("logout", duration, false);
    return createErrorResult(ErrorTypes.CHECK_FAILED, {
      status: res.status,
      duration,
      userId,
    });
  }

  const body = parseJsonSafely(res.body, "logout");
  if (!body) {
    return createErrorResult(ErrorTypes.INVALID_JSON, {
      status: res.status,
      userId,
    });
  }

  logPerformance("logout", duration, true);
  return createSuccessResult({});
}

export const options = {
  stages: [
    { duration: "30s", target: 10 }, // Warm-up
    { duration: "1m", target: 50 }, // Ramp-up
    { duration: "2m", target: 50 }, // Steady state
    { duration: "30s", target: 0 }, // Ramp-down
  ],
  thresholds: config.thresholds,
};

export function setup() {
  console.log(`开始setup: 准备登录 ${config.testUsers.length} 个测试账号`);

  const loginResults = [];

  for (const user of config.testUsers) {
    const loginResult = login(user.account, user.password);
    if (loginResult.success) {
      loginResults.push({
        ...user,
        ...loginResult,
      });
    } else {
      console.error(
        `账号 ${user.account} 登录失败: ${JSON.stringify(loginResult)}`,
      );
    }
  }

  console.log(
    `setup完成: 成功登录 ${loginResults.length}/${config.testUsers.length} 个账号`,
  );

  return { users: loginResults };
}

export default function (data) {
  if (!data.users || data.users.length === 0) {
    console.error("没有可用的测试账号");
    return;
  }

  const userIndex = (__VU - 1) % data.users.length;
  const user = data.users[userIndex];

  const accessToken = user.accessToken;
  const userId = user.userId;
  const password = user.password;

  randomSleep(0.3, 0.8);

  const scenario = Math.random();

  if (scenario < 0.3) {
    const userInfoResult = getUserInfo(userId, accessToken);
    check(userInfoResult, {
      获取用户信息成功: (r) => r.success === true,
    });

    if (!userInfoResult.success) {
      console.error(`VU ${__VU} 获取用户信息失败`);
    }

    randomSleep(0.3, 0.8);

    const newNickname = `user_${Date.now()}`;
    const nicknameResult = changeNickname(userId, accessToken, newNickname);

    check(nicknameResult, {
      修改昵称成功: (r) => r.success === true,
    });

    if (!nicknameResult.success) {
      console.error(`VU ${__VU} 修改昵称失败`);
    }
  } else if (scenario < 0.5) {
    const inviterCodeResult = generateInviterCode(userId, accessToken);

    check(inviterCodeResult, {
      生成邀请码成功: (r) => r.success === true,
    });

    if (!inviterCodeResult.success) {
      console.error(`VU ${__VU} 生成邀请码失败`);
    }

    randomSleep(0.3, 0.8);

    const secondInviterCodeResult = generateInviterCode(userId, accessToken);
    check(secondInviterCodeResult, {
      重复生成邀请码成功: (r) => r.success === true,
    });
  } else if (scenario < 0.65) {
    const newPassword = generateRandomPassword();
    const changePasswordResult = changePassword(
      userId,
      accessToken,
      password,
      newPassword
    );

    check(changePasswordResult, {
      修改密码成功: (r) => r.success === true,
    });

    if (!changePasswordResult.success) {
      console.error(`VU ${__VU} 修改密码失败`);
    } else {
      user.password = newPassword;
    }
  } else if (scenario < 0.75) {
    const logoutResult = logout(userId, accessToken);
    check(logoutResult, {
      登出成功: (r) => r.success === true,
    });

    if (!logoutResult.success) {
      console.error(`VU ${__VU} 登出失败`);
    }

    randomSleep(0.5, 1.0);

    const userInfoResult = getUserInfo(userId, accessToken);
    check(userInfoResult, {
      登出后获取信息应失败: (r) => r.success === false,
    });
  } else if (scenario < 0.85) {
    const invalidTokenResult = getUserInfo(userId, "invalid_access_token");
    check(invalidTokenResult, {
      无效Token应失败: (r) => r.success === false,
    });

    randomSleep(0.3, 0.8);

    const emptyNicknameResult = changeNickname(userId, accessToken, "");
    check(emptyNicknameResult, {
      空昵称应失败: (r) => r.success === false,
    });

    randomSleep(0.3, 0.8);

    const longNicknameResult = changeNickname(
      userId,
      accessToken,
      "a".repeat(1000)
    );
    check(longNicknameResult, {
      超长昵称应失败: (r) => r.success === false,
    });
  } else if (scenario < 0.95) {
    const userInfoResult = getUserInfo(userId, accessToken);
    check(userInfoResult, {
      获取用户信息成功: (r) => r.success === true,
    });

    randomSleep(0.2, 0.5);

    const inviterCodeResult = generateInviterCode(userId, accessToken);
    check(inviterCodeResult, {
      生成邀请码成功: (r) => r.success === true,
    });

    randomSleep(0.2, 0.5);

    const newNickname = `user_${Date.now()}`;
    const nicknameResult = changeNickname(userId, accessToken, newNickname);
    check(nicknameResult, {
      修改昵称成功: (r) => r.success === true,
    });

    randomSleep(0.2, 0.5);

    const userInfoResult2 = getUserInfo(userId, accessToken);
    check(userInfoResult2, {
      再次获取用户信息成功: (r) => r.success === true,
    });
  } else {
    const concurrentOperations = Math.floor(Math.random() * 3) + 2;

    for (let i = 0; i < concurrentOperations; i++) {
      const op = Math.random();

      if (op < 0.33) {
        const userInfoResult = getUserInfo(userId, accessToken);
        check(userInfoResult, {
          并发获取用户信息成功: (r) => r.success === true,
        });
      } else if (op < 0.66) {
        const newNickname = `user_${Date.now()}_${i}`;
        const nicknameResult = changeNickname(userId, accessToken, newNickname);
        check(nicknameResult, {
          并发修改昵称成功: (r) => r.success === true,
        });
      } else {
        const inviterCodeResult = generateInviterCode(userId, accessToken);
        check(inviterCodeResult, {
          并发生成邀请码成功: (r) => r.success === true,
        });
      }

      sleep(0.1);
    }
  }

  randomSleep(0.5, 2.0);
}
