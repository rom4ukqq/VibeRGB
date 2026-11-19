import { useEffect, useRef, useState, type CSSProperties } from 'react';
import { createRoot } from 'react-dom/client';
import { isEnabled, enable, disable } from '@tauri-apps/plugin-autostart';
import { useUpdates } from './useUpdates';
import { version as appVersion } from '../package.json';
import { Button, FluentProvider, webDarkTheme } from '@fluentui/react-components';
import { listen } from '@tauri-apps/api/event';
import { api, type ApplyResult, type Assignment, type Device, type Effect, type ProfileLibrary, type RGB } from './api';
import { localizeDiagnostic, messages, sceneName, type Language, type Messages } from './i18n';
import { MatrixBackdrop } from './CanvasMatrix';
import { Profiles } from './profiles';
import { ColorPicker } from './ColorPicker';
import './style.css';

const DEFAULT_COLOR = '#00AAFF';
const STATIC: Effect[] = [
  { id: 'solid', name: 'Color', color: true, brightness: true },
  { id: 'off', name: 'Off', color: false, brightness: false },
];
const SYNC: Effect[] = [
  { id: 'pulse', name: 'Pulse', color: true, brightness: true },
  { id: 'candle', name: 'Candle', color: true, brightness: true },
  { id: 'hue', name: 'Spectrum', color: false, brightness: true },
  { id: 'sync-rainbow', name: 'Rainbow', color: false, brightness: true },
  { id: 'gta-vi', name: 'GTA VI', color: false, brightness: true },
];

type Notice = { label?: keyof Messages; message?: string; count?: number; issues: string[] };
type Tab = 'studio' | 'profiles' | 'settings';
type Group = 'sync' | 'hardware';

function rgb(value: string): RGB {
  return [1, 3, 5].map(index => Number.parseInt(value.slice(index, index + 2), 16)) as RGB;
}

function App() {
  const [tab, setTab] = useState<Tab>('studio');
  const [language, setLanguage] = useState<Language>(() => localStorage.getItem('vibergb.language') === 'ru' ? 'ru' : 'en');
  const [autostart, setAutostart] = useState<boolean>();
  const [background, setBackground] = useState(() => localStorage.getItem('vibergb.background') !== 'off');
  const [autoCheck, setAutoCheck] = useState(() => localStorage.getItem('vibergb.autoCheck') !== 'off');
  const updates = useUpdates(autoCheck);
  const [group, setGroup] = useState<Group>('sync');
  const [devices, setDevices] = useState<Device[]>([]);
  const [library, setLibrary] = useState<ProfileLibrary>({ profiles: [], startupId: null });
  const revision = useRef(-1);
  const [active, setActive] = useState<Assignment[]>([]);
  const [deviceId, setDeviceId] = useState('');
  const [zoneId, setZoneId] = useState('');
  const [scene, setScene] = useState('solid');
  const [hardwareEffect, setHardwareEffect] = useState('');
  const [color, setColor] = useState(DEFAULT_COLOR);
  const [brightness, setBrightness] = useState(100);
  const [speed, setSpeed] = useState(100);
  const [pending, setPending] = useState(true);
  const [notice, setNotice] = useState<Notice>();
  const t = messages[language];
  const localize = (value: string) => localizeDiagnostic(value, language);

  function acceptState(state: ApplyResult) {
    if (state.revision < revision.current) return false;
    revision.current = state.revision;
    setActive(state.active);
    return true;
  }

  useEffect(() => {
    document.documentElement.lang = language;
    localStorage.setItem('vibergb.language', language);
    void api.language(language).catch(error => setNotice({ label: 'localeFailed', issues: [String(error)] }));
  }, [language]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const subscribed = listen<ApplyResult>('lighting-error', ({ payload }) => {
      if (!disposed && acceptState(payload)) setNotice({ label: 'animationFailed', issues: payload.issues });
    }).then(stop => {
      if (disposed) stop();
      else unlisten = stop;
    }).catch(error => {
      if (!disposed) setNotice({ label: 'eventsFailed', issues: [String(error)] });
    });
    Promise.allSettled([subscribed.then(() => api.devices()), api.profiles(), isEnabled()]).then(([hardware, saved, launch]) => {
      if (disposed) return;
      if (hardware.status === 'fulfilled') {
        setDevices(hardware.value.devices);
        acceptState(hardware.value.state);
      }
      if (saved.status === 'fulfilled') setLibrary(saved.value);
      if (launch.status === 'fulfilled') setAutostart(launch.value);
      const issues = [hardware, saved, launch].flatMap(result => result.status === 'rejected' ? [String(result.reason)] : []);
      if (hardware.status === 'fulfilled') issues.push(...hardware.value.state.issues);
      if (issues.length) setNotice({ label: 'loadFailed', issues });
      setPending(false);
    });
    return () => { disposed = true; unlisten?.(); };
  }, []);

  const connected = devices.filter(device => device.zones.length);
  const device = devices.find(item => item.id === deviceId);
  const entries = (device ? [device] : connected).flatMap(item => item.zones
    .filter(zone => !zoneId || zone.id === zoneId)
    .map(zone => ({ device: item, zone })));
  const availableEffects = [...new Map(entries.flatMap(({ zone }) =>
    zone.effects.map(effect => [effect.id, effect] as const))).values()];
  const modes = group === 'sync' ? [STATIC[0], ...SYNC, STATIC[1]] : availableEffects;
  const mode = modes.find(item => item.id === (group === 'sync' ? scene : hardwareEffect)) ?? modes[0];
  const eligible = mode ? entries.filter(({ zone }) =>
    group === 'hardware' ? zone.effects.some(effect => effect.id === mode.id)
      : STATIC.some(item => item.id === mode.id) || zone.direct) : [];
  const dimmable = mode && (group === 'sync' ? mode.brightness : eligible.some(({ zone }) =>
    zone.effects.some(effect => effect.id === mode.id && effect.brightness)));
  const validColor = /^#[a-f\d]{6}$/i.test(color);
  const animations = active.filter(item => SYNC.some(effect => effect.id === item.mode));
  const adjustableSpeed = group === 'sync' && SYNC.some(effect => effect.id === mode?.id);

  async function run(action: () => Promise<void>) {
    if (pending) return false;
    setPending(true);
    setNotice(undefined);
    try {
      await action();
      return true;
    } catch (error) {
      setNotice({ label: 'failed', issues: [String(error)] });
      return false;
    } finally {
      setPending(false);
    }
  }

  function showResult(result: ApplyResult, skipped: string[] = []) {
    if (!acceptState(result)) return;
    setNotice({
      label: result.count ? 'updated' : 'noUpdates',
      count: result.count || undefined,
      issues: [...result.issues, ...skipped],
    });
  }

  function apply() {
    if (!mode) return;
    const assignments = eligible.map(({ device: item, zone }): Assignment => ({
      target: { deviceId: item.id, zoneId: zone.id },
      mode: mode.id,
      color: mode.color ? rgb(color) : [0, 0, 0],
      brightness: group === 'sync' || zone.effects.some(effect => effect.id === mode.id && effect.brightness)
        ? (mode.id === 'off' ? 0 : brightness) : 100,
      speed: adjustableSpeed ? speed : 100,
    }));
    const skipped = entries.filter(entry => !eligible.includes(entry)).map(({ device: item, zone }) =>
      `${item.name} / ${zone.name} — Mode unavailable.`);
    void run(async () => showResult(await api.apply(assignments), skipped));
  }

  function selectDevice(id: string) {
    setDeviceId(id);
    setZoneId('');
    setTab('studio');
  }

  async function refreshDevices() {
    const { devices: found, state } = await api.devices(true);
    if (acceptState(state) && state.issues.length) setNotice({ label: 'refreshIssues', issues: state.issues });
    const current = found.find(item => item.id === deviceId);
    if (deviceId && !current) selectDevice('');
    else if (zoneId && !current?.zones.some(zone => zone.id === zoneId)) setZoneId('');
    setDevices(found);
  }

  function installPawnio() {
    void run(async () => {
      const result = await api.installPawnio();
      if (result !== 'cancelled') await refreshDevices();
      if (result === 'restartRequired') {
        setNotice({ label: 'pawnioRestart', issues: [] });
      } else if (result === 'installed') {
        setNotice({ label: 'pawnioInstalled', issues: [] });
      }
    });
  }

  const previewColor = mode?.id === 'gta-vi' ? '#c219c1' : mode?.color && validColor ? color : DEFAULT_COLOR;
  const cycleSeconds = mode?.id === 'pulse' ? 4 : mode?.id === 'candle' ? 3 : mode?.id === 'gta-vi' ? 12 : 6;
  const previewStyle = {
    '--scene-color': previewColor,
    '--level': mode?.id === 'off' ? 0 : dimmable ? brightness / 100 : 1,
    '--cycle': `${cycleSeconds * (adjustableSpeed ? 100 / speed : 1)}s`,
  } as CSSProperties;
  const unavailable = device && !device.zones.length;
  const brightnessPartial = group === 'hardware' && dimmable && eligible.some(({ zone }) =>
    !zone.effects.some(effect => effect.id === mode.id && effect.brightness));

  return <FluentProvider theme={webDarkTheme} className="app-provider">
    <div className="shell">
      {background && <MatrixBackdrop animate />}
      <aside className="rail">
        <div className="brand"><span className="brand-mark" aria-hidden="true" />VibeRGB</div>
        <nav className="navigation" aria-label={t.sections}>
          {(['studio', 'profiles', 'settings'] as const).map(page =>
            <button key={page} aria-current={tab === page ? 'page' : undefined} onClick={() => setTab(page)}>
              {t[page]}{page === 'settings' && updates.version && <span className="update-dot" aria-label={t.available} />}
            </button>)}
        </nav>
        <div className="device-heading"><h2>{t.devices}</h2><Button size="small" appearance="subtle" disabled={pending} aria-label={t.refresh} title={t.refresh} onClick={() => void run(refreshDevices)}>↻</Button></div>
        <div className="device-list">
          <button className="device-row" aria-pressed={tab === 'studio' && deviceId === ''} onClick={() => selectDevice('')}>
            <span className="device-icon available" aria-hidden="true" /><span><strong>{t.all}</strong><small>{t.active}: {connected.length}</small></span>
          </button>
          {devices.map(item => <button key={item.id} className="device-row" aria-pressed={tab === 'studio' && deviceId === item.id} onClick={() => selectDevice(item.id)}>
            <span className={item.zones.length ? 'device-icon available' : 'device-icon inactive'} aria-hidden="true" />
            <span><strong>{localize(item.name)}</strong><small>{item.zones.length ? `${t.zones}: ${item.zones.length}` : t.unavailable}</small></span>
          </button>)}
        </div>
      </aside>

      <main className="main">
        <div className="content page-enter" key={tab}>
          <header className="page-header">
            <h1>{t[tab]}</h1>
            {tab === 'studio' && animations.length > 0 && <Button className="stop-button" disabled={pending}
              onClick={() => void run(async () => showResult(await api.apply(animations.map(item => ({ ...item, mode: 'off', brightness: 0 })))))}>
              {t.stop}
            </Button>}
          </header>
          {notice && <div className="notice" role="status">
            <div><strong>{notice.label ? t[notice.label] : localize(notice.message ?? '')}{notice.count ? `: ${notice.count}` : ''}</strong>{notice.issues.length > 0 && <ul>{notice.issues.map(issue => <li key={issue}>{localize(issue)}</li>)}</ul>}</div>
            <Button appearance="subtle" size="small" aria-label={t.dismiss} onClick={() => setNotice(undefined)}>×</Button>
          </div>}
          {tab === 'settings' ? <div className="settings">
            <section className="glass settings-group">
            <h2>{t.interface}</h2>
            <label className="setting-row">{t.language}
              <select value={language} onChange={event => setLanguage(event.target.value === 'ru' ? 'ru' : 'en')}>
                <option value="en" lang="en">English</option>
                <option value="ru" lang="ru">Русский</option>
              </select>
            </label>
            <label className="setting-row">{t.animatedBackground}
              <input type="checkbox" role="switch" checked={background} onChange={event => {
                const value = event.target.checked;
                localStorage.setItem('vibergb.background', value ? 'on' : 'off');
                setBackground(value);
              }} />
            </label>
            </section>
            <section className="glass settings-group">
            <h2>{t.launch}</h2>
            <label className="setting-row"><span>{t.loginStart}<small>{t.trayStart}</small></span>
              <input type="checkbox" role="switch" checked={autostart ?? false} disabled={pending || autostart === undefined}
                onChange={event => {
                  const enabled = event.target.checked;
                  void run(async () => {
                    await (enabled ? enable() : disable());
                    setAutostart(await isEnabled());
                  });
                }} />
            </label>
            <label className="setting-row">{t.startupProfile}
              <select value={library.startupId ?? ''} disabled={pending} onChange={event => {
                const id = event.target.value || null;
                void run(async () => setLibrary(await api.startup(id)));
              }}>
                <option value="">{t.noStartup}</option>
                {library.profiles.map(profile => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
              </select>
            </label>
            </section>
            <section className="glass settings-group">
              <h2>{t.updates} <small>v{appVersion}</small></h2>
              <label className="setting-row">{t.autoCheck}
                <input type="checkbox" role="switch" checked={autoCheck} onChange={event => {
                  const value = event.target.checked;
                  localStorage.setItem('vibergb.autoCheck', value ? 'on' : 'off');
                  setAutoCheck(value);
                }} />
              </label>
              <div className="setting-row" role="status">
                <span>{t[updates.status]} {updates.version}{updates.status === 'installing' && updates.percent !== undefined && ` · ${updates.percent}%`}
                  {updates.error && <small>{updates.error}</small>}
                </span>
                {updates.status === 'available'
                  ? <Button appearance="primary" className="primary-action" onClick={updates.install}>{t.installUpdate}</Button>
                  : <Button disabled={updates.status === 'checking' || updates.status === 'installing'} onClick={updates.checkNow}>{t.checkUpdate}</Button>}
              </div>
            </section>
          </div> : tab === 'profiles' ? <Profiles t={t} profiles={library.profiles} canSave={active.length > 0} pending={pending}
            onSave={name => run(async () => { setLibrary(await api.save(name)); })}
            onReplace={id => run(async () => setLibrary(await api.replace(id)))}
            onRename={(id, name) => run(async () => { setLibrary(await api.rename(id, name)); })}
            onDelete={id => run(async () => { setLibrary(await api.delete(id)); })}
            onApply={id => run(async () => showResult(await api.applyProfile(id)))} /> :
          <div className="studio">
            <section className="preview glass" style={previewStyle} aria-label={t.preview}>
              <div className="preview-heading">{t.preview}</div>
              <div className={`render render-${mode?.id ?? 'off'}`} aria-hidden="true">
                <div className="strip">{Array.from({ length: 24 }, (_, index) =>
                  <span key={index} className="led" style={{ '--i': index } as CSSProperties} />)}</div>
              </div>
              <div className="preview-caption">
                <div><strong>{mode ? sceneName(mode.id, mode.name, t) : t.noEffect}</strong><small>{device ? localize(device.name) : t.all}</small></div>
                <span className="zone-count">{t.zones}: {eligible.length} / {entries.length}</span>
              </div>
            </section>

            <section className="controls glass" aria-label={t.controls}>
              <div className="segment" aria-label={t.type}>
                <button aria-pressed={group === 'sync'} onClick={() => setGroup('sync')}>{t.sync}</button>
                <button aria-pressed={group === 'hardware'} onClick={() => setGroup('hardware')}>{t.hardware}</button>
              </div>
              {group === 'hardware' && <small>{t.independent}</small>}
              {!!device?.zones.length && <label className="zone-select">{t.zone}
                <select value={zoneId} onChange={event => setZoneId(event.target.value)}>
                  <option value="">{t.allZones}</option>
                  {device.zones.map(zone => <option key={zone.id} value={zone.id}>{localize(zone.name)}</option>)}
                </select>
              </label>}
              {unavailable ? <>
                <p className="muted">{localize(device.detail)}</p>
                {device.needsPawnioInstall && <Button className="primary-action" appearance="primary" disabled={pending} onClick={installPawnio}>
                  {t.installPawnio}
                </Button>}
                {device.needsPawnioElevation && <Button className="primary-action" appearance="primary" disabled={pending}
                  onClick={() => void run(api.restartForRgbMemory)}>
                  {t.restartAsAdmin}
                </Button>}
              </> : <>
                <div key={group} className="choice-grid page-enter" aria-label={t.effect}>
                  {modes.map(item => <button key={item.id} className="choice" aria-pressed={mode?.id === item.id}
                    onClick={() => (group === 'sync' ? setScene : setHardwareEffect)(item.id)}>{sceneName(item.id, item.name, t)}</button>)}
                </div>
                {mode?.color && <ColorPicker value={color} onChange={setColor} t={t} />}
                {dimmable && <div className="brightness">
                  <label htmlFor="brightness">{t.brightness} <output htmlFor="brightness">{brightness}%</output></label>
                  <input id="brightness" type="range" min="0" max="100" value={brightness}
                    style={{ '--fill': `${brightness}%` } as CSSProperties}
                    onChange={event => setBrightness(Number(event.target.value))} />
                  {brightnessPartial && <small>{t.partialBrightness}</small>}
                </div>}
                {adjustableSpeed && <div className="brightness page-enter">
                  <label htmlFor="speed">{t.speed} <output htmlFor="speed">{(speed / 100).toLocaleString(language)}×</output></label>
                  <input id="speed" type="range" min="25" max="250" step="5" value={speed}
                    style={{ '--fill': `${(speed - 25) / 2.25}%` } as CSSProperties}
                    onChange={event => setSpeed(Number(event.target.value))} />
                </div>}
                {!entries.length && <p className="muted">{pending ? t.scanning : t.noZones}</p>}
                {!!entries.length && !eligible.length && <p className="muted">{t.unsupported}</p>}
                <Button className="apply-button primary-action" appearance="primary" disabled={pending || !eligible.length || (mode?.color && !validColor)}
                  onClick={apply}>{t.apply}</Button>
              </>}
            </section>
          </div>}
        </div>
      </main>
    </div>
  </FluentProvider>;
}

createRoot(document.getElementById('root')!).render(<App />);
