import { useMemo, useState } from 'react';
import { t } from '../lib/i18n';

type DiffFile = {path: string; added: number; removed: number; lines: string[]; binary: boolean};

/** Splits a unified diff (`git diff`) into files, with each file's added and removed line counts. */
export function parseDiff(diff: string): DiffFile[] {
  const files: DiffFile[] = [];
  let current: DiffFile | null = null;
  for (const line of diff.split('\n')) {
    if (line.startsWith('diff --git ')) {
      const path = line.match(/ b\/(.+)$/)?.[1] ?? line.slice(11);
      current = {path, added: 0, removed: 0, lines: [], binary: false};
      files.push(current);
      continue;
    }
    if (!current) continue;
    if (line.startsWith('Binary files')) current.binary = true;
    // File headers and index lines say nothing a reader needs beside the file's name.
    if (line.startsWith('index ') || line.startsWith('--- ') || line.startsWith('+++ ')) continue;
    if (line.startsWith('+')) current.added++;
    else if (line.startsWith('-')) current.removed++;
    current.lines.push(line);
  }
  for (const f of files) while (f.lines.length && f.lines[f.lines.length - 1] === '') f.lines.pop();
  return files;
}

const LIMIT = 400;

/**
 * A result's changes, file by file, with additions and removals coloured, as code review tools show them
 * (UX comparison, 2026-11-12). The raw diff stays available to copy.
 */
export function DiffView({diff}: {diff: string}) {
  const files = useMemo(() => parseDiff(diff), [diff]);
  if (files.length === 0) return <p className="secondary">{t('No changes.')}</p>;
  return <div className="diff-files">
    {files.map(f => <DiffFileView key={f.path} file={f} open={files.length <= 6}/>)}
  </div>;
}

function DiffFileView({file: f, open}: {file: DiffFile; open: boolean}) {
  const [all, setAll] = useState(false);
  const shown = all ? f.lines : f.lines.slice(0, LIMIT);
  return <details className="diff-file" open={open}>
    <summary>
      <span className="mono selectable diff-path">{f.path}</span>
      <span className="diff-counts"><span className="diff-added">+{f.added}</span> <span className="diff-removed">−{f.removed}</span></span>
    </summary>
    {f.binary ? <p className="secondary diff-note">{t('Binary file; not shown.')}</p>
      : <pre className="diff-lines">{shown.map((line, i) => <span key={i} className={lineClass(line)}>{line || ' '}</span>)}</pre>}
    {f.lines.length > LIMIT && !all && <button type="button" className="link-button table-more" onClick={() => setAll(true)}>
      {t('Show all {n} lines', {n: f.lines.length})}</button>}
  </details>;
}

function lineClass(line: string): string {
  if (line.startsWith('@@')) return 'diff-hunk';
  if (line.startsWith('+')) return 'diff-add';
  if (line.startsWith('-')) return 'diff-del';
  if (line.startsWith('new file') || line.startsWith('deleted file') || line.startsWith('rename ') || line.startsWith('similarity'))
    return 'diff-meta';
  return '';
}
