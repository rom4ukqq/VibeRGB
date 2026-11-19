import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { Button } from '@fluentui/react-components';
import type { Calibration } from './api';
import type { Messages } from './i18n';

const CHANNELS = [
  ['red', '#ff3b3b'],
  ['green', '#3bd96b'],
  ['blue', '#3b7bff'],
] as const;

type Props = {
  value: Calibration;
  disabled: boolean;
  t: Messages;
  onChange: (value: Calibration) => Promise<unknown>;
  onShowWhite: () => void;
};

/** White balance for one device. Changes are sent while dragging, so the
 *  result shows on the hardware right away. */
export function CalibrationPanel({ value, disabled, t, onChange, onShowWhite }: Props) {
  const [draft, setDraft] = useState(value);
  const timer = useRef<number | undefined>(undefined);
  const latest = useRef(value);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  function update(next: Calibration) {
    setDraft(next);
    latest.current = next;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => void onChange(latest.current), 120);
  }

  const neutral = draft.red === 100 && draft.green === 100 && draft.blue === 100;
  return <section className="calibration glass" aria-label={t.calibration}>
    <div className="calibration-header">
      <div><h2>{t.calibration}</h2><small>{t.calibrationHint}</small></div>
      <div className="calibration-actions">
        <Button size="small" disabled={disabled} onClick={onShowWhite}>{t.showWhite}</Button>
        <Button size="small" appearance="subtle" disabled={disabled || neutral}
          onClick={() => update({ red: 100, green: 100, blue: 100 })}>{t.reset}</Button>
      </div>
    </div>
    <div className="calibration-sliders">
      {CHANNELS.map(([channel, tint]) => <label key={channel} className="slider">
        <span>{t[channel]}<output>{draft[channel]}%</output></span>
        <input type="range" min="0" max="100" value={draft[channel]} disabled={disabled}
          className="gain-track" style={{ '--tint': tint, '--fill': `${draft[channel]}%` } as CSSProperties}
          onChange={event => update({ ...draft, [channel]: Number(event.target.value) })} />
      </label>)}
    </div>
  </section>;
}
