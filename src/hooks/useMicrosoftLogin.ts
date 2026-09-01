/** M3 / L2:微软设备码流登录状态机 hook
 *
 *  状态流转:
 *    Idle → Requesting → Polling → Success / Declined / Expired / Failed
 *
 *  - `start()` 调 `start_microsoft_login` 拿 device_code / user_code
 *  - `setTimeout(interval * 1000)` 周期调 `poll_microsoft_login`
 *  - 终态(Success / Declined / Expired / Failed)立即 clear timer
 *  - `cancel()` 主动清 timer + reset 内部 ref(unmount 自动 cancel)
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import { pollMicrosoftLogin, startMicrosoftLogin } from '../lib/tauri';
import type { Account } from '../types/account';

type HookState =
  | { status: 'idle' }
  | { status: 'requesting' }
  | {
      status: 'polling';
      userCode: string;
      verificationUri: string;
      expiresIn: number;
      interval: number;
    }
  | { status: 'success'; account: Account }
  | { status: 'declined' }
  | { status: 'expired' }
  | { status: 'failed'; message: string };

export function useMicrosoftLogin() {
  const [state, setState] = useState<HookState>({ status: 'idle' });
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const deviceCodeRef = useRef<string | null>(null);

  const cancel = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    deviceCodeRef.current = null;
  }, []);

  // unmount 自动 cancel
  useEffect(() => () => cancel(), [cancel]);

  const scheduleNextPoll = useCallback((ms: number, fn: () => void) => {
    timerRef.current = setTimeout(fn, ms);
  }, []);

  const doPoll = useCallback(async () => {
    const code = deviceCodeRef.current;
    if (!code) return;
    try {
      const r = await pollMicrosoftLogin(code);
      switch (r.status) {
        case 'success':
          cancel();
          if (r.account) {
            setState({ status: 'success', account: r.account });
          } else {
            setState({ status: 'failed', message: '登录成功但未返回账户信息' });
          }
          return;
        case 'pending':
          setState((prev) =>
            prev.status === 'polling'
              ? { ...prev, expiresIn: Math.max(0, prev.expiresIn - 5) }
              : prev
          );
          scheduleNextPoll(5000, () => {
            void doPoll();
          });
          return;
        case 'declined':
          cancel();
          setState({ status: 'declined' });
          return;
        case 'expired':
          cancel();
          setState({ status: 'expired' });
          return;
        case 'failed':
          cancel();
          setState({ status: 'failed', message: r.message ?? '未知错误' });
          return;
      }
    } catch (e) {
      cancel();
      setState({ status: 'failed', message: String(e) });
    }
  }, [cancel, scheduleNextPoll]);

  const start = useCallback(async () => {
    cancel();
    setState({ status: 'requesting' });
    try {
      const r = await startMicrosoftLogin();
      deviceCodeRef.current = r.device_code;
      setState({
        status: 'polling',
        userCode: r.user_code,
        verificationUri: r.verification_uri,
        expiresIn: r.expires_in,
        interval: r.interval,
      });
      // 首次 poll 等满一个 interval(微软官方建议)
      scheduleNextPoll(r.interval * 1000, () => {
        void doPoll();
      });
    } catch (e) {
      setState({ status: 'failed', message: String(e) });
    }
  }, [cancel, doPoll, scheduleNextPoll]);

  return { state, start, cancel };
}
