/** M3 / L1:账户系统类型(与 Rust 后端 serde tag 序列化对齐)
 *
 *  Rust 端 `Account` 枚举用 `#[serde(tag = "type", rename_all = "lowercase")]`,
 *  序列化后是扁平结构(不是嵌套对象),例如:
 *    { "type": "offline", "id": "...", "username": "Steve", "created_at": "2026-..." }
 *    { "type": "microsoft", "id": "...", "username": "x", "uuid": "...", "access_token": "...", "refresh_token": "...", "expires_at": "..." }
 *
 *  前端用 discriminated union 接收,L1 只用 Offline 变体,Microsoft 留给 L2。
 */

export type AccountType = 'offline' | 'microsoft';

/** M3:离线账户(M1 实际能创建的就是这个) */
export interface OfflineAccount {
  type: 'offline';
  id: string;
  username: string;
  /** ISO 8601 UTC 字符串 */
  created_at: string;
}

/** M3:微软账户(L1 占位,L2 实装) */
export interface MicrosoftAccount {
  type: 'microsoft';
  id: string;
  username: string;
  uuid: string;
  access_token: string;
  refresh_token: string;
  expires_at: string;
  /** Xbox User ID —— spec §4.1,launch 注入 `auth_xuid` 需要 */
  xuid: string;
}

/** M3:账户联合类型(前端拿到后用 `if (acc.type === 'offline')` 收窄) */
export type Account = OfflineAccount | MicrosoftAccount;
