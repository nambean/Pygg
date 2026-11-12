import { useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { Search } from 'lucide-react';
import { t } from '../lib/i18n';
import { useSheetFocus } from '../hooks/useSheetFocus';

/** One thing the palette can do. Commands that spend money or delete something only open the screen that asks. */
export type Command = {id: string; group: string; label: string; hint?: string; keywords?: string; run: () => void};

/** Ctrl+K / Cmd+K (SCREENS.md §2.6): type to filter, arrows to move, Enter to run, Esc to close. */
export function CommandPalette({commands, onClose}: {commands: Command[]; onClose: () => void}) {
  const [query, setQuery] = useState('');
  const [active, setActive] = useState(0);
  const dialog = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLUListElement>(null);
  useEffect(() => {
    const win = document.querySelector('.window');
    win?.setAttribute('inert', '');
    return () => win?.removeAttribute('inert');
  }, []);
  // After the effect above, as in Sheet: the window is no longer inert when focus goes back to it.
  useSheetFocus(dialog);

  const shown = useMemo(() => {
    const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    return commands.filter(c => {
      const text = `${c.group} ${c.label} ${c.keywords ?? ''}`.toLowerCase();
      return words.every(w => text.includes(w));
    });
  }, [commands, query]);
  useEffect(() => setActive(0), [query]);
  useEffect(() => {
    list.current?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({block: 'nearest'});
  }, [active]);

  const run = (command: Command | undefined) => {
    if (!command) return;
    onClose();
    command.run();
  };
  const onKey = (event: React.KeyboardEvent) => {
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); onClose(); }
    else if (event.key === 'ArrowDown') { event.preventDefault(); setActive(i => Math.min(i + 1, shown.length - 1)); }
    else if (event.key === 'ArrowUp') { event.preventDefault(); setActive(i => Math.max(i - 1, 0)); }
    // Home and End move the caret in the search field; with Ctrl they move through the list.
    else if (event.key === 'Home' && event.ctrlKey) { event.preventDefault(); setActive(0); }
    else if (event.key === 'End' && event.ctrlKey) { event.preventDefault(); setActive(Math.max(shown.length - 1, 0)); }
    else if (event.key === 'Enter') { event.preventDefault(); run(shown[active]); }
  };

  let lastGroup = '';
  return createPortal(<div className="sheet-backdrop palette-backdrop" onMouseDown={e => { if (e.target === e.currentTarget) onClose(); }}>
    <div className="palette" role="dialog" aria-modal="true" aria-label={t('Command palette')} ref={dialog} tabIndex={-1} onKeyDown={onKey}>
      <div className="palette-search">
        <Search size={15} aria-hidden/>
        <input autoFocus className="palette-input" value={query} onChange={e => setQuery(e.target.value)}
          placeholder={t('Type a command or a screen')} aria-label={t('Command')} role="combobox" aria-expanded="true"
          aria-controls="palette-list" aria-activedescendant={shown[active] ? `palette-${shown[active].id}` : undefined}
          spellCheck={false} autoComplete="off"/>
      </div>
      <ul className="palette-list" id="palette-list" role="listbox" aria-label={t('Commands')} ref={list}>
        {shown.map((c, i) => {
          const heading = c.group !== lastGroup;
          lastGroup = c.group;
          return <li key={c.id} role="presentation">
            {heading && <div className="palette-group" role="presentation">{c.group}</div>}
            <div id={`palette-${c.id}`} role="option" aria-selected={i === active} data-index={i}
              className={`palette-item${i === active ? ' active' : ''}`}
              onMouseMove={() => setActive(i)} onClick={() => run(c)}>
              <span>{c.label}</span>{c.hint && <kbd>{c.hint}</kbd>}
            </div>
          </li>;
        })}
        {shown.length === 0 && <li className="palette-empty" role="presentation">{t('No command matches “{query}”', {query})}</li>}
      </ul>
    </div>
  </div>, document.body);
}
