/** M3 / L2:与 Rust 后端 `LoginResult` 对齐
 *
 *  Rust 端序列化形式(serde tag = "status", rename_all = "lowercase"):
 *    {"status": "success", "account": {...}}        // success 带 account
 *    {"status": "pending"}                          // account skip_serializing_if
 *    {"status": "declined"}
 *    {"status": "expired"}
 *    {"status": "failed", "message": "..."}         // Failed 是 struct variant
 *
 *  注意:`message` 字段是顶层(因为 LoginStatus::Failed 是 struct variant),
 *  不是嵌套在 `status` 里。用 discriminated union 让 TS 自动收窄。
 */

import type { Account } from './account';

/** 登录结果(对应后端 `struct LoginResult`) */
export type LoginResult =
  | { status: 'pending'; account?: Account }
  | { status: 'success'; account?: Account }
  | { status: 'declined'; account?: Account }
  | { status: 'expired'; account?: Account }
  | { status: 'failed'; message: string; account?: Account };

/** 设备码端点响应(对应后端 `struct DeviceCodeResponse`) */
export interface DeviceCodeResponse {
  device_code: string;
  user_code: string;
  verification_uri: string;
  expires_in: number;
  interval: number;
  message?: string;
}
