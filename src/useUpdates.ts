import { useCallback, useEffect, useRef, useState } from 'react';
import { check, type Update } from '@tauri-apps/plugin-updater';

export function useUpdates(automatic: boolean) {
  const candidate = useRef<Update | null>(null);
  const busy = useRef(false);
  const [status, setStatus] = useState<'idle' | 'checking' | 'current' | 'available' | 'installing' | 'updateFailed'>('idle');
  const [error, setError] = useState('');
  const [version, setVersion] = useState('');
  const [percent, setPercent] = useState<number>();

  const checkNow = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setStatus('checking');
    setError('');
    try {
      const found = await check({ timeout: 20000 });
      const previous = candidate.current;
      candidate.current = found;
      setVersion(found?.version ?? '');
      setStatus(found ? 'available' : 'current');
      await previous?.close();
    } catch (reason) {
      setError(String(reason));
      setStatus(candidate.current ? 'available' : 'updateFailed');
    } finally {
      busy.current = false;
    }
  }, []);

  useEffect(() => {
    if (!automatic) return;
    void checkNow();
    const timer = window.setInterval(checkNow, 6 * 60 * 60 * 1000);
    return () => window.clearInterval(timer);
  }, [automatic, checkNow]);

  async function install() {
    const update = candidate.current;
    if (!update || busy.current) return;
    busy.current = true;
    setStatus('installing');
    setError('');
    setPercent(undefined);
    let total = 0, downloaded = 0;
    try {
      await update.downloadAndInstall(event => {
        if (event.event === 'Started') total = event.data.contentLength ?? 0;
        if (event.event === 'Progress') {
          downloaded += event.data.chunkLength;
          if (total) setPercent(Math.min(100, Math.round(downloaded * 100 / total)));
        }
      });
    } catch (reason) {
      setError(String(reason));
      setStatus('available');
    } finally {
      busy.current = false;
    }
  }
  return { status, version, error, percent, checkNow, install };
}
