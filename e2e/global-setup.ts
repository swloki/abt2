import { request } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import 'dotenv/config';

const STORAGE_STATE = '.auth/admin.json';

/**
 * globalSetup：跑一次 API 登录，把 session cookie 持久化到 storageState。
 * 之后每条 test 默认带登录态（config use.storageState）。
 *
 * 关键：登录失败也返回 200（重渲染表单 HTML），必须靠 Set-Cookie: id= 判断成败。
 * cookie 名 `id`（tower-sessions 默认），由 SessionManagerLayer 自动写入。
 */
export default async function globalSetup() {
  const baseURL = process.env.ABT_BASE_URL ?? 'http://127.0.0.1:8000';
  const username = process.env.ABT_LOGIN_USER ?? 'admin';
  const password = process.env.ABT_LOGIN_PASS ?? 'chenxi0514';

  const ctx = await request.newContext({ baseURL });
  try {
    const r = await ctx.post('/login', { form: { username, password } });

    // 登录失败也返回 200（重渲染 #login-form-area HTML），靠 Set-Cookie 判断
    const setCookie = r.headers()['set-cookie'] ?? '';
    if (!setCookie.includes('id=')) {
      throw new Error(
        `globalSetup 登录失败：未拿到 session cookie (id=)。\n` +
          `  检查：服务是否在 ${baseURL} 运行？凭据 ${username} 是否正确？\n` +
          `  status=${r.status()}`,
      );
    }

    await mkdir('.auth', { recursive: true });
    await ctx.storageState({ path: STORAGE_STATE });
  } finally {
    await ctx.dispose();
  }
}
