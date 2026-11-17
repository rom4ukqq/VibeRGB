import { useState } from 'react';
import { Button } from '@fluentui/react-components';
import type { Profile } from './api';
import type { Messages } from './i18n';

type Props = {
  t: Messages;
  profiles: Profile[];
  canSave: boolean;
  pending: boolean;
  onSave: (name: string) => Promise<boolean>;
  onReplace: (id: string) => Promise<boolean>;
  onRename: (id: string, name: string) => Promise<boolean>;
  onDelete: (id: string) => Promise<boolean>;
  onApply: (id: string) => Promise<boolean>;
};

export function Profiles({ t, profiles, canSave, pending, onSave, onReplace, onRename, onDelete, onApply }: Props) {
  const [name, setName] = useState('');
  const [renaming, setRenaming] = useState<{ id: string; name: string }>();
  const [confirmation, setConfirmation] = useState<{ id: string; action: 'delete' | 'replace' }>();

  return <>
    <form className="panel save-form" onSubmit={event => {
      event.preventDefault();
      if (!pending && canSave && name.trim()) void onSave(name).then(saved => { if (saved) setName(''); });
    }}>
      <label>{t.profileName}
        <input value={name} maxLength={64} onChange={event => setName(event.target.value)} />
      </label>
      <Button className="primary-action" appearance="primary" type="submit" disabled={pending || !canSave || !name.trim()}>{t.save}</Button>
      {!canSave && <small>{t.saveHint}</small>}
    </form>
    <section className="profiles" aria-label={t.savedProfiles}>
      {!profiles.length && <p className="glass empty-state">{t.emptyProfiles}</p>}
      {profiles.map(profile => <article key={profile.id} className="profile">
        <div className="profile-header">
          <div><h2>{profile.name}</h2><small>{t.profileZones}: {profile.assignments.length}</small></div>
          <Button className="primary-action" appearance="primary" disabled={pending} onClick={() => void onApply(profile.id)}>{t.apply}</Button>
        </div>
        {renaming?.id === profile.id ? <form className="inline-actions" onSubmit={event => {
          event.preventDefault();
          if (!pending && renaming.name.trim()) {
            void onRename(profile.id, renaming.name).then(saved => { if (saved) setRenaming(undefined); });
          }
        }}>
          <input autoFocus aria-label={t.newName} value={renaming.name} maxLength={64}
            onChange={event => setRenaming({ id: profile.id, name: event.target.value })} />
          <Button appearance="primary" className="primary-action" type="submit" disabled={pending || !renaming.name.trim()}>{t.save}</Button>
          <Button type="button" appearance="subtle" onClick={() => setRenaming(undefined)}>{t.cancel}</Button>
        </form> : confirmation?.id === profile.id ? <div className="inline-actions">
          <span>{confirmation.action === 'delete' ? t.deleteQuestion : t.replaceQuestion}</span>
          <Button disabled={pending} onClick={() => { const action = confirmation.action === 'delete' ? onDelete : onReplace;
            void action(profile.id).then(saved => { if (saved) setConfirmation(undefined); }); }}>{confirmation.action === 'delete' ? t.remove : t.replace}</Button>
          <Button appearance="subtle" onClick={() => setConfirmation(undefined)}>{t.cancel}</Button>
        </div> : <div className="inline-actions">
          <Button appearance="subtle" disabled={pending} onClick={() => {
            setConfirmation(undefined);
            setRenaming({ id: profile.id, name: profile.name });
          }}>{t.rename}</Button>
          <Button appearance="subtle" disabled={pending || !canSave} onClick={() => {
            setRenaming(undefined);
            setConfirmation({ id: profile.id, action: 'replace' });
          }}>{t.replace}</Button>
          <Button appearance="subtle" disabled={pending} onClick={() => {
            setRenaming(undefined);
            setConfirmation({ id: profile.id, action: 'delete' });
          }}>{t.remove}</Button>
        </div>}
      </article>)}
    </section>
  </>;
}
