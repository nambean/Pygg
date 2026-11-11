import { Cpu, Server } from 'lucide-react';
import {
  siAlibabacloud, siAnthropic, siDeepseek, siDocker, siGooglegemini, siHuggingface, siKimi, siLmstudio, siMinimax,
  siMistralai, siNvidia, siOllama, siOpenrouter, siVllm, siX,
} from 'simple-icons';

type Icon = {path: string; title: string};

/**
 * Provider marks from Simple Icons (CC0), drawn in the current text color so they read in light and dark.
 * The marks are their owners' trademarks, shown only to identify the provider. Presets without one get a letter.
 */
const ICONS: Record<string, Icon> = {
  gemini: siGooglegemini, anthropic: siAnthropic, openrouter: siOpenrouter,
  deepseek: siDeepseek, 'deepseek-anthropic': siDeepseek, qwen: siAlibabacloud, mistral: siMistralai, xai: siX,
  nvidia: siNvidia, huggingface: siHuggingface, moonshot: siKimi, 'moonshot-anthropic': siKimi,
  minimax: siMinimax, 'minimax-anthropic': siMinimax, ollama: siOllama, lmstudio: siLmstudio,
  'docker-model-runner': siDocker, vllm: siVllm,
};

export function ProviderLogo({kind, name, size = 16}: {kind: string; name: string; size?: number}) {
  const icon = ICONS[kind];
  if (icon) {
    return <span className="provider-logo" aria-hidden>
      <svg viewBox="0 0 24 24" width={size} height={size}><path d={icon.path} fill="currentColor"/></svg>
    </span>;
  }
  if (kind === 'custom' || kind === 'custom-anthropic') return <span className="provider-logo" aria-hidden><Server size={size - 1}/></span>;
  if (['llamacpp', 'jan'].includes(kind)) return <span className="provider-logo" aria-hidden><Cpu size={size - 1}/></span>;
  return <span className="provider-logo letter" aria-hidden>{name.trim().slice(0, 1).toUpperCase()}</span>;
}
