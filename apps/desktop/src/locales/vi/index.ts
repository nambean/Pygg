/** Vietnamese, keyed by the English text passed to t() (see src/lib/i18n.ts). */
import { gateway } from './gateway';
import { shell } from './shell';
import { tasks } from './tasks';

export const vi: Record<string, string> = {...shell, ...tasks, ...gateway};
