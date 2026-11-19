import { useRef, type CSSProperties } from 'react';
import type { Messages } from './i18n';

// Saturated colors that LEDs reproduce well; pastel tones wash out to white on hardware.
export const SWATCHES = [
  '#FF0000', '#FF3300', '#FF8800', '#FFCC00', '#66FF00', '#00FF55',
  '#00FFCC', '#00AAFF', '#0033FF', '#6600FF', '#CC00FF', '#FF0077', '#FFFFFF',
];

const HEX = /^#[a-f\d]{6}$/i;

function toHueSaturation(hex: string): [number, number] {
  const [r, g, b] = [1, 3, 5].map(index => Number.parseInt(hex.slice(index, index + 2), 16) / 255);
  const max = Math.max(r, g, b), delta = max - Math.min(r, g, b);
  if (!delta) return [0, 0];
  const hue = max === r ? ((g - b) / delta) % 6 : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4;
  return [(hue * 60 + 360) % 360, delta / max];
}

function fromHueSaturation(hue: number, saturation: number) {
  const channel = (n: number) => {
    const k = (n + hue / 60) % 6;
    const value = 1 - saturation * Math.max(0, Math.min(k, 4 - k, 1));
    return Math.round(value * 255).toString(16).padStart(2, '0');
  };
  return `#${channel(5)}${channel(3)}${channel(1)}`.toUpperCase();
}

export function ColorPicker({ value, onChange, t }: { value: string; onChange: (hex: string) => void; t: Messages }) {
  const valid = HEX.test(value);
  const lastHue = useRef(0);
  const [measuredHue, saturation] = valid ? toHueSaturation(value) : [lastHue.current, 1];
  if (valid && saturation > 0) lastHue.current = measuredHue;
  const hue = saturation > 0 ? measuredHue : lastHue.current;

  return <fieldset className="color-field">
    <legend>{t.color}</legend>
    <div className="swatches">{SWATCHES.map(preset => <button key={preset} type="button" className="swatch"
      style={{ background: preset }} aria-label={preset} aria-pressed={value === preset} onClick={() => onChange(preset)} />)}</div>
    <label className="slider">
      <span>{t.tone}<output>{Math.round(hue)}°</output></span>
      <input type="range" className="hue-track" min="0" max="359" value={Math.round(hue)}
        onChange={event => onChange(fromHueSaturation(Number(event.target.value), saturation || 1))} />
    </label>
    <label className="slider">
      <span>{t.saturation}<output>{Math.round(saturation * 100)}%</output></span>
      <input type="range" className="saturation-track" min="0" max="100" value={Math.round(saturation * 100)}
        style={{ '--hue-color': fromHueSaturation(hue, 1) } as CSSProperties}
        onChange={event => onChange(fromHueSaturation(hue, Number(event.target.value) / 100))} />
    </label>
    <div className="hex-row">
      <span className="color-chip" style={{ background: valid ? value : 'transparent' }} aria-hidden="true" />
      <input className="hex" aria-label="HEX" value={value} maxLength={7} aria-invalid={!valid} spellCheck={false}
        onChange={event => onChange(event.target.value.toUpperCase())} />
    </div>
    {!valid && <small className="validation">{t.colorFormat}</small>}
  </fieldset>;
}
