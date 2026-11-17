import { useState } from 'react';
import { Button } from '@fluentui/react-components';
import type { Profile } from './api';
import type { Messages } from './i18n';

type Props = {
  t: Messages;
  profiles: Profile[];
  canSave: boolean;
  pending: boolean;
  onSave: (name: string) => Promise<void>;
  onRename: (id: string, name: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  onApply: (id: string) => Promise<void>;
};

export function Profiles({ t, profiles, canSave, pending, onSave, onRename, onDelete, onApply }: Props) {
  const [name, setName] = useState('');
  const [renaming, setRenaming] = useState<{ id: string; name: string }>();
  const [deleting, setDeleting] = useState<string>();

  return <>
    <form className="panel save-form" onSubmit={event => {
      event.preventDefault();
      if (!pending && canSave && name.trim()) void onSave(name);
    }}>
      <label>{t.profileName}
        <input value={name} maxLength={64} onChange={event => setName(event.target.value)} />
      </label>
      <Button appearance="primary" type="submit" disabled={pending || !canSave || !name.trim()}>{t.save}</Button>
      {!canSave && <small>{t.saveHint}</small>}
    </form>
    <section className="profiles" aria-label={t.savedProfiles}>
      {!profiles.length && <p className="muted">{t.emptyProfiles}</p>}
      {profiles.map(profile => <article key={profile.id} className="profile">
        <div className="profile-header">
          <div><h2>{profile.name}</h2><small>{t.profileZones}: {profile.assignments.length}</small></div>
          <Button appearance="primary" disabled={pending} onClick={() => void onApply(profile.id)}>{t.apply}</Button>
        </div>
        {renaming?.id === profile.id ? <form className="inline-actions" onSubmit={event => {
          event.preventDefault();
          if (!pending && renaming.name.trim()) {
            void onRename(profile.id, renaming.name);
            setRenaming(undefined);
          }
        }}>
          <input autoFocus aria-label={t.newName} value={renaming.name} maxLength={64}
            onChange={event => setRenaming({ id: profile.id, name: event.target.value })} />
          <Button type="submit" disabled={pending || !renaming.name.trim()}>{t.save}</Button>
          <Button appearance="subtle" onClick={() => setRenaming(undefined)}>{t.cancel}</Button>
        </form> : deleting === profile.id ? <div className="inline-actions">
          <span>{t.deleteQuestion}</span>
          <Button disabled={pending} onClick={() => { void onDelete(profile.id); setDeleting(undefined); }}>{t.remove}</Button>
          <Button appearance="subtle" onClick={() => setDeleting(undefined)}>{t.cancel}</Button>
        </div> : <div className="inline-actions">
          <Button appearance="subtle" disabled={pending} onClick={() => {
            setDeleting(undefined);
            setRenaming({ id: profile.id, name: profile.name });
          }}>{t.rename}</Button>
          <Button appearance="subtle" disabled={pending} onClick={() => {
            setRenaming(undefined);
            setDeleting(profile.id);
          }}>{t.remove}</Button>
        </div>}
      </article>)}
    </section>
  </>;
}
