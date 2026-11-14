#!/usr/bin/env node
// Generates the README's two animated images, each in a light and a dark version, into docs/images/:
//
// - usage-{light,dark}.svg: how Pygg is used. A terminal installs the CLI and routes Codex and Claude Code
//   through Pygg; the gateway comes up and connects the user's tools to the providers they connected.
// - request-flow-{light,dark}.svg: one request through the gateway, as `gateway::forward` handles it
//   (apps/desktop/src-tauri/src/gateway/mod.rs): the client token, the route (`auto` in auto.rs, a combo, or
//   <connection>/<model>), policy and the spend hold, the attempt journaled before it is sent, the connection's
//   key ring (keys.rs), a 429 and a 402 that cool their keys (policy.rs), the route's next target, the answer
//   streamed back, and the ledger entry that settles each attempt.
//
// Pure SVG and CSS keyframes, no script (GitHub strips script from images). Under prefers-reduced-motion every
// animation stops and the finished picture stays. Run: node scripts/readme-diagrams.mjs
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const OUT = join(dirname(fileURLToPath(import.meta.url)), '..', 'docs', 'images');

const THEMES = {
  light: {
    bg: '#ffffff', bg2: '#f3f6f4', panel: '#ffffff', panel2: '#f6f8fa', border: '#d5dbe1', text: '#1f2328',
    muted: '#57606a', faint: '#8c959f', accent: '#0b7a4b', accentSoft: '#e1f3e9', glow: '#0b7a4b',
    request: '#2563c9', requestSoft: '#e3ecfb', warn: '#b45309', warnSoft: '#fdecd8', line: '#9aa5b1',
    chip: '#eef1f4', term: '#0f1714', termText: '#e6edf3', termMuted: '#8b989f', termBorder: '#24302b', ok: '#3fbf7f',
    shadow: 'rgba(31,35,40,.10)',
  },
  dark: {
    bg: '#0d1117', bg2: '#101a15', panel: '#151b23', panel2: '#161b22', border: '#2a333d', text: '#e6edf3',
    muted: '#9198a1', faint: '#6e7681', accent: '#2fb170', accentSoft: '#11291c', glow: '#2fb170',
    request: '#6aa5ff', requestSoft: '#132540', warn: '#f2a34e', warnSoft: '#35230f', line: '#4d5864',
    chip: '#1f262e', term: '#0a0f0d', termText: '#e6edf3', termMuted: '#7d8a90', termBorder: '#22302a', ok: '#3fbf7f',
    shadow: 'rgba(0,0,0,.45)',
  },
};

const FONT = `-apple-system,BlinkMacSystemFont,"Segoe UI","Noto Sans",Helvetica,Arial,sans-serif`;
const MONO = `ui-monospace,SFMono-Regular,"SF Mono","Cascadia Mono",Menlo,Consolas,monospace`;

// ---------- helpers ----------
const kf = (name, frames) => `@keyframes ${name}{${frames.map(([p, css]) => `${p}%{${css}}`).join('')}}`;
const tr = (x, y) => `transform:translate(${+x.toFixed(1)}px,${+y.toFixed(1)}px)`;
/** Visible from a to b (percent), hidden otherwise. */
const show = (name, a, b) => kf(name, [[0, 'opacity:0'], [a, 'opacity:0'], [a + 0.6, 'opacity:1'], [b, 'opacity:1'],
  [Math.min(b + 0.6, 100), 'opacity:0'], [100, 'opacity:0']]);
const esc = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const tidy = (s) => s.replace(/\n\s*\n/g, '\n');

function defs(t) {
  return `<defs>
<linearGradient id="bgGrad" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${t.bg}"/><stop offset="1" stop-color="${t.bg2}"/></linearGradient>
<filter id="soft" x="-20%" y="-20%" width="140%" height="160%"><feDropShadow dx="0" dy="4" stdDeviation="8" flood-color="${t.shadow}"/></filter>
<filter id="glow" x="-60%" y="-60%" width="220%" height="220%"><feGaussianBlur stdDeviation="7"/></filter>
</defs>`;
}

// =====================================================================================================
// 1. Usage: terminal on the left, the gateway hub on the right. 14 s loop.
// =====================================================================================================
function usage(t) {
  const W = 940, H = 340, DUR = 16;
  const TX = 16, TY = 16, TW = 446, TH = 308; // terminal card
  const CH = 7.9; // monospace advance at 13px, rounded up so the cover never lags behind the text
  const lx = TX + 22;
  // Typed lines: [y, command, typing start %, typing end %]. The desktop app (or `pygg gateway serve` on Linux)
  // runs the gateway; `pygg setup claude` makes Claude Code's own Pygg key; `pygg run claude` starts it through Pygg.
  const cmds = [
    [TY + 66, 'npm install -g @pygg/cli@dev', 2, 10],
    [TY + 122, 'pygg status', 18, 22],
    [TY + 178, 'pygg setup claude', 32, 38],
    [TY + 254, 'pygg run claude', 52, 57],
  ];
  const outputs = [
    // [y, text, class, appear %]
    [TY + 88, '# 0.1.0-dev.1 (tag dev): Windows and Linux', 'tm', 12],
    [TY + 144, 'Gateway   http://127.0.0.1:8741 answers', 'tok', 24],
    [TY + 200, '✓ Its own Pygg key; no file changes', 'tok', 40],
    [TY + 220, 'Next: pygg run claude', 'tm', 42],
    [TY + 276, '→ Claude Code through Pygg, model auto', 'tok', 59],
  ];
  const END = 95, UP = 24, FLOW = 59;
  let css = `
svg{font-family:${FONT}}
.mono{font-family:${MONO}}
.tt{fill:${t.termText};font-size:13px}
.tm{fill:${t.termMuted};font-size:12.5px}
.tok{fill:${t.ok};font-size:12.5px}
.pr{fill:${t.ok};font-size:13px;font-weight:700}
.lbl{fill:${t.text};font-size:13.5px;font-weight:600}
.sub{fill:${t.muted};font-size:12px}
.cap{fill:${t.muted};font-size:11px;font-weight:600;letter-spacing:.1em}
.node{fill:${t.panel};stroke:${t.border};stroke-width:1.2}
.wire{stroke:${t.line};stroke-width:1.4;fill:none}
.pulse{stroke:${t.accent};stroke-width:2.6;fill:none;stroke-linecap:round;stroke-dasharray:14 400;stroke-dashoffset:414}
.anim{opacity:0}
`;
  let body = '';
  cmds.forEach(([y, cmd, a, b], i) => {
    const w = cmd.length * CH + 4;
    const steps = cmd.length;
    css += show(`l${i}`, a - 1.5, END);
    css += kf(`c${i}`, [[0, tr(0, 0)], [a, `${tr(0, 0)};animation-timing-function:steps(${steps},end)`], [b, tr(w, 0)], [100, tr(w, 0)]]);
    // the cursor is shown while this is the line being typed or the last one typed
    const next = cmds[i + 1] ? cmds[i + 1][2] - 1.5 : END;
    css += show(`k${i}`, a - 1.5, Math.min(next, b + 1));
    css += `.l${i}{animation:l${i} ${DUR}s infinite}.c${i}{animation:c${i} ${DUR}s infinite}.k${i}{animation:k${i} ${DUR}s infinite}`;
    body += `<g class="anim l${i}"><text class="pr mono" x="${lx}" y="${y}">$</text><text class="tt mono" x="${lx + 16}" y="${y}">${esc(cmd)}</text>
<g class="c${i}"><rect x="${lx + 15}" y="${y - 14}" width="${TW - 40}" height="19" fill="${t.term}"/><g class="anim k${i}"><rect class="blink" x="${lx + 16}" y="${y - 12}" width="7.5" height="15" fill="${t.ok}"/></g></g></g>`;
  });
  outputs.forEach(([y, text, cls, a], i) => {
    css += show(`o${i}`, a, END) + `.o${i}{animation:o${i} ${DUR}s infinite}`;
    body += `<text class="anim o${i} ${cls} mono" x="${lx + 16}" y="${y}">${esc(text)}</text>`;
  });
  // The hub lights up once the gateway answers; requests flow once Claude Code runs through it.
  css += kf('dim', [[0, 'opacity:.45'], [UP, 'opacity:.45'], [UP + 2, 'opacity:1'], [END, 'opacity:1'], [END + 2, 'opacity:.45'], [100, 'opacity:.45']]);
  css += `.hub{animation:dim ${DUR}s infinite}`;
  css += show('flowing', FLOW, END) + `.flowing{animation:flowing ${DUR}s infinite}`;
  css += kf('blink', [[0, 'opacity:1'], [50, 'opacity:0'], [100, 'opacity:1']]) + `.blink{animation:blink 1s steps(1,end) infinite}`;
  // One target at a time: auto picks a model for each request, it does not broadcast.
  css += kf('sel', [[0, 'stroke-dashoffset:414;opacity:1'], [25, 'stroke-dashoffset:0;opacity:1'], [25.5, 'opacity:0'], [100, 'opacity:0;stroke-dashoffset:0']]);
  css += kf('halo', [[0, 'opacity:.14'], [50, 'opacity:.3'], [100, 'opacity:.14']]);

  // ---- hub ----
  const HX = 480, HW = W - HX - 16;
  const tool = { x: HX + 50, y: 168 };
  const gw = { x: HX + 168, y: 168 };
  const targets = [
    ['Free and cheap models', 'Gemini, OpenRouter, Groq …', 76],
    ['Your plans', 'subscription keys', 136],
    ['Local models', 'Ollama, on this PC', 196],
    ['Web sessions', 'opt-in, text only, guarded', 256],
  ];
  const TXN = HX + 248, TWN = W - 16 - 14 - TXN;
  const curve = (y) => `M${gw.x + 34} ${gw.y}C${gw.x + 70} ${gw.y} ${TXN - 40} ${y} ${TXN} ${y}`;
  const hub = `
<g class="hub">
<path class="wire" d="M${tool.x + 24} ${tool.y}H${gw.x - 34}"/>
<g class="anim flowing"><path class="pulse" d="M${tool.x + 24} ${tool.y}H${gw.x - 34}" style="stroke-dasharray:14 140;animation:flowS 1.5s linear infinite"/></g>
${targets.map(([, , y]) => `<path class="wire" d="${curve(y)}"/>`).join('')}
<g class="anim flowing">${targets.map(([, , y], i) => `<path class="pulse" d="${curve(y)}" style="opacity:0;animation:sel 6s ${(i * 1.5).toFixed(1)}s linear infinite"/>`).join('')}</g>
<rect x="${tool.x - 24}" y="${tool.y - 24}" width="48" height="48" rx="12" class="node"/>
<path d="M${tool.x - 9} ${tool.y - 7}l7 7-7 7M${tool.x + 1} ${tool.y + 8}h9" stroke="${t.text}" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round"/>
<text class="sub" x="${tool.x}" y="${tool.y + 44}" text-anchor="middle">Your tools</text>
<text class="sub" x="${tool.x}" y="${tool.y + 60}" text-anchor="middle" style="font-size:10.5px">Claude Code · Codex</text>
<circle cx="${gw.x}" cy="${gw.y}" r="46" fill="${t.glow}" filter="url(#glow)" class="halo" style="opacity:.2;animation:halo 3.2s ease-in-out infinite"/>
<rect x="${gw.x - 34}" y="${gw.y - 34}" width="68" height="68" rx="18" fill="${t.panel}" stroke="${t.accent}" stroke-width="1.6"/>
<rect x="${gw.x - 26}" y="${gw.y - 26}" width="52" height="52" rx="13" fill="${t.accentSoft}"/>
<path d="M${gw.x - 13} ${gw.y - 9}h14a8 8 0 0 1 0 16h-8v9" stroke="${t.accent}" stroke-width="3.2" fill="none" stroke-linecap="round" stroke-linejoin="round"/><circle cx="${gw.x + 6}" cy="${gw.y + 15}" r="3" fill="${t.accent}"/>
<text class="lbl" x="${gw.x}" y="${gw.y + 58}" text-anchor="middle">Pygg gateway</text>
<text class="sub mono" x="${gw.x}" y="${gw.y + 74}" text-anchor="middle" style="font-size:11px">model: auto</text>
${targets.map(([title, sub, y]) => `<g><rect x="${TXN}" y="${y - 22}" width="${TWN}" height="44" rx="10" class="node"/><circle cx="${TXN + TWN - 12}" cy="${y}" r="3" fill="${t.accent}"/>
<text class="lbl" x="${TXN + 12}" y="${y - 3}" style="font-size:12.5px">${title}</text><text class="sub" x="${TXN + 12}" y="${y + 13}" style="font-size:11px">${sub}</text></g>`).join('')}
</g>`;
  css += kf('flowS', [[0, 'stroke-dashoffset:154'], [100, 'stroke-dashoffset:0']]);
  css += `@media (prefers-reduced-motion:reduce){*{animation:none!important}.anim{opacity:1}.hub{opacity:1}.pulse{opacity:0!important}.c0>rect,.c1>rect,.c2>rect,.c3>rect{opacity:0}.k0,.k1,.k2,.k3{opacity:0}.halo{opacity:.2}}`;

  return tidy(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-labelledby="ut ud">
<title id="ut">Using Pygg</title>
<desc id="ud">A terminal installs the Pygg command line with npm install -g @pygg/cli@dev, checks with pygg status that Pygg's gateway answers on http://127.0.0.1:8741, gives Claude Code its own Pygg key with pygg setup claude, and starts Claude Code through Pygg with pygg run claude and the model auto. The gateway sends each request to one of the providers you connected: free and cheap models, your plans, local models such as Ollama, or opt-in web sessions behind a guard.</desc>
${defs(t)}
<style>${css}</style>
<rect width="${W}" height="${H}" rx="16" fill="url(#bgGrad)"/>
<g filter="url(#soft)"><rect x="${TX}" y="${TY}" width="${TW}" height="${TH}" rx="14" fill="${t.term}" stroke="${t.termBorder}"/></g>
<circle cx="${TX + 20}" cy="${TY + 20}" r="4.5" fill="#e5534b"/><circle cx="${TX + 35}" cy="${TY + 20}" r="4.5" fill="#d29922"/><circle cx="${TX + 50}" cy="${TY + 20}" r="4.5" fill="#3fbf7f"/>
<text class="tm mono" x="${TX + TW / 2}" y="${TY + 24}" text-anchor="middle" style="font-size:11.5px">~/my-project</text>
<circle cx="${TX + TW - 58}" cy="${TY + 20}" r="3" fill="${t.ok}"/><text class="tm" x="${TX + TW - 50}" y="${TY + 24}" style="font-size:10px;letter-spacing:.12em;font-weight:600">LOCAL</text>
<path d="M${TX} ${TY + 38}H${TX + TW}" stroke="${t.termBorder}"/>
<clipPath id="termClip"><rect x="${TX + 1}" y="${TY + 39}" width="${TW - 2}" height="${TH - 40}" rx="12"/></clipPath>
<g clip-path="url(#termClip)">${body}</g>
<rect x="${HX}" y="${TY}" width="${HW}" height="${TH}" rx="14" fill="${t.panel2}" stroke="${t.border}" opacity=".7"/>
<text class="cap" x="${HX + 18}" y="${TY + 28}">ROUTES YOUR TOOLS TO YOUR PROVIDERS</text>
${hub}
</svg>
`);
}

// =====================================================================================================
// 2. Request flow. 12 s loop.
// =====================================================================================================
function flow(t) {
  const W = 940, H = 494, DUR = 15;
  // Event times below are on a 0-100 scale for the story; M() compresses the story into the first ~74% of the loop so
  // the finished ledger holds for about 3.5 s before the loop restarts (times above 92 map to the closing 97-100%).
  const M = (p) => (p <= 92 ? +(p * 0.8).toFixed(2) : +(97 + ((p - 92) * 3) / 8).toFixed(2));
  const kfm = (name, frames) => kf(name, frames.map(([p, c]) => [M(p), c]));
  const showm = (name, a, b) => kfm(name, [[0, 'opacity:0'], [a, 'opacity:0'], [a + 0.6, 'opacity:1'], [b, 'opacity:1'],
    [Math.min(b + 0.6, 100), 'opacity:0'], [100, 'opacity:0']]);
  const R1 = { y: 54, h: 96 };
  const cy1 = R1.y + R1.h / 2;
  const BW = 164, GAP = 22;
  const COLS = [0, 1, 2, 3, 4].map((i) => 16 + i * (BW + GAP));
  const cx = (i) => COLS[i] + BW / 2;
  const LANE = 178, ROW2 = 204, BUS = cx(4);
  const CARD = { x: 420, w: 380 };
  const CARDS = [
    { id: 'c1', y: ROW2, h: 80 },
    { id: 'c2', y: ROW2 + 88, h: 46, title: 'Plans', sub: 'subscription keys, counted as quota' },
    { id: 'c3', y: ROW2 + 142, h: 46, title: 'Ollama and local runtimes', sub: 'on this PC, no API charge' },
    { id: 'c4', y: ROW2 + 196, h: 46, title: 'Web sessions · opt-in', sub: 'text only · the guard: one at a time, rests, a daily cap' },
  ];
  const mid = (i) => CARDS[i].y + CARDS[i].h / 2;
  const LEDGER = { x: 16, y: ROW2, w: 360, h: 242 };
  const RIGHT = CARD.x + CARD.w;
  const k1y = mid(0); // the packet meets card 1 here

  const packet = kfm('pk', [
    [0, `${tr(cx(0), cy1)};opacity:0`], [2, `${tr(cx(0), cy1)};opacity:1`],
    [8, tr(cx(1), cy1)], [14, tr(cx(2), cy1)], [20, tr(cx(3), cy1)], [26, tr(BUS, cy1)],
    [30, tr(BUS, k1y)], [34, tr(RIGHT, k1y)], [41, tr(RIGHT, k1y)],
    [44, tr(BUS, k1y)], [47, tr(RIGHT, k1y)], [55, tr(RIGHT, k1y)],
    [58, tr(BUS, k1y)], [61, tr(BUS, mid(2))], [63, tr(RIGHT, mid(2))], [67, tr(RIGHT, mid(2))],
    [70, tr(CARD.x, mid(2))], [72, tr(398, mid(2))], [77, tr(398, LANE)], [88, tr(cx(0), LANE)],
    [91.5, `${tr(cx(0), R1.y + R1.h)};opacity:1`], [92, `${tr(cx(0), R1.y + R1.h)};opacity:0`],
    [100, `${tr(cx(0), R1.y + R1.h)};opacity:0`],
  ]);
  const packetColor = kfm('pkc', [
    [0, `fill:${t.request}`], [40, `fill:${t.request}`], [40.5, `fill:${t.warn}`], [46, `fill:${t.warn}`],
    [46.5, `fill:${t.request}`], [54, `fill:${t.request}`], [54.5, `fill:${t.warn}`], [62, `fill:${t.warn}`],
    [62.5, `fill:${t.request}`], [66, `fill:${t.request}`], [66.5, `fill:${t.accent}`], [100, `fill:${t.accent}`],
  ]);
  const glowK = (name, spans) => {
    const f = [[0, `stroke:${t.border}`]];
    for (const [a, b] of spans) f.push([a, `stroke:${t.border}`], [a + 0.5, `stroke:${t.request}`], [b, `stroke:${t.request}`], [b + 0.5, `stroke:${t.border}`]);
    f.push([100, `stroke:${t.border}`]);
    return kfm(name, f);
  };
  const flash = (name, colour, soft, a, b) => kfm(name, [
    [0, `stroke:${t.border};fill:${t.panel}`], [a, `stroke:${t.border};fill:${t.panel}`],
    [a + 0.5, `stroke:${colour};fill:${soft}`], [b, `stroke:${colour};fill:${soft}`],
    [b + 0.5, `stroke:${t.border};fill:${t.panel}`], [100, `stroke:${t.border};fill:${t.panel}`]]);
  const keyK = (name, a, b) => kfm(name, [[0, `fill:${t.chip};stroke:${t.border}`], [a, `fill:${t.chip};stroke:${t.border}`],
    [a + 0.5, `fill:${t.requestSoft};stroke:${t.request}`], [b, `fill:${t.requestSoft};stroke:${t.request}`],
    [b + 0.5, `fill:${t.warnSoft};stroke:${t.warn}`], [95, `fill:${t.warnSoft};stroke:${t.warn}`],
    [95.5, `fill:${t.chip};stroke:${t.border}`], [100, `fill:${t.chip};stroke:${t.border}`]]);

  const anims = {
    r1a: [25, 40.5], r1b: [40.5, 95], r2a: [44, 54.5], r2b: [54.5, 95], r3a: [58, 66.5], r3b: [66.5, 91], r3c: [91, 95],
    b429: [40.5, 95], b402: [54.5, 95], b200: [66.5, 95], nt: [55, 95], cool1: [41, 95], cool2: [55, 95],
  };
  let css = `
svg{font-family:${FONT}}
.m{font-family:${MONO}}
.box{fill:${t.panel};stroke:${t.border};stroke-width:1.4}
.t{fill:${t.text};font-size:15px;font-weight:600}
.s{fill:${t.muted};font-size:12.5px}
.h{fill:${t.muted};font-size:10.5px;font-weight:600;letter-spacing:.1em}
.chip{fill:${t.chip};stroke:${t.border}}
.ct{fill:${t.text};font-size:12px}
.ln{stroke:${t.line};stroke-width:1.4;fill:none}
.lane{stroke:${t.accent};stroke-width:1.6;fill:none;stroke-dasharray:5 4}
.ah{fill:${t.line}}
.ok{fill:${t.accent};font-weight:700}
.no{fill:${t.warn};font-weight:700}
.row{font-size:12.5px;fill:${t.text}}
.dim{fill:${t.muted}}
.anim,.pkt{opacity:0}
${packet}${packetColor}
${glowK('g1', [[5, 10]])}${glowK('g2', [[11, 16]])}${glowK('g3', [[17, 22]])}${glowK('g4', [[23, 28], [44, 46], [58, 60]])}
${flash('f1', t.warn, t.warnSoft, 34, 58)}${flash('f3', t.accent, t.accentSoft, 63, 94)}
${keyK('k1', 29, 40.5)}${keyK('k2', 43.5, 54.5)}
${Object.entries(anims).map(([n, [a, b]]) => showm(n, a, b) + `.${n}{animation:${n} ${DUR}s infinite}`).join('')}
${kfm('halo', [[0, 'opacity:.12'], [50, 'opacity:.35'], [100, 'opacity:.12']])}
.pkt{animation:pk ${DUR}s linear infinite}
.pkt .pk{animation:pkc ${DUR}s step-end infinite}
.pkt .halo{opacity:.25}
#gw{animation:g1 ${DUR}s infinite}#route{animation:g2 ${DUR}s infinite}#policy{animation:g3 ${DUR}s infinite}
#journal{animation:g4 ${DUR}s infinite}#c1{animation:f1 ${DUR}s infinite}#c3{animation:f3 ${DUR}s infinite}
#k1{animation:k1 ${DUR}s infinite}#k2{animation:k2 ${DUR}s infinite}
@media (prefers-reduced-motion:reduce){
*{animation:none!important}
.pkt,.r1a,.r2a,.r3a,.r3b,.b200{opacity:0}
.r1b,.r2b,.r3c,.b429,.b402,.nt,.cool1,.cool2{opacity:1}
#c1{stroke:${t.warn}}#c3{stroke:${t.accent}}#k1,#k2{stroke:${t.warn};fill:${t.warnSoft}}
}`;

  const box = (id, i, title, lines, extra = '') => `
<g filter="url(#soft)"><rect id="${id}" class="box" x="${COLS[i]}" y="${R1.y}" width="${BW}" height="${R1.h}" rx="12"/></g>
<text class="h" x="${COLS[i] + 14}" y="${R1.y + 20}">${i + 1}</text>
<text class="t" x="${COLS[i] + 14}" y="${R1.y + 40}">${title}</text>
${lines.map((l, k) => `<text class="s${l.mono ? ' m' : ''}" x="${COLS[i] + 14}" y="${R1.y + 61 + k * 17}"${l.mono ? ' style="font-size:12px"' : ''}>${l.text}</text>`).join('')}${extra}`;
  const arrowR = (x1, x2, y) => `<path class="ln" d="M${x1} ${y}H${x2 - 7}"/><path class="ah" d="M${x2} ${y}l-7 -4v8z"/>`;
  const arrowL = (x1, x2, y) => `<path class="ln" d="M${x1} ${y}H${x2 + 7}"/><path class="ah" d="M${x2} ${y}l7 -4v8z"/>`;
  const chip = (x, y, w, label) =>
    `<rect class="chip" x="${x}" y="${y}" width="${w}" height="20" rx="10"/><text class="ct m" x="${x + w / 2}" y="${y + 14}" text-anchor="middle">${label}</text>`;
  const badge = (cls, x, y, label, colour) =>
    `<g class="anim ${cls}"><rect x="${x}" y="${y}" width="48" height="22" rx="11" fill="${colour}"/><text x="${x + 24}" y="${y + 15.5}" text-anchor="middle" font-size="12.5" font-weight="700" fill="${t.bg}" class="m">${label}</text></g>`;
  const ledgerRow = (y, n, target, cls, status, statusCls, note) => `
<g class="anim ${cls}"><text class="row m" x="${LEDGER.x + 16}" y="${y}">${n}</text><text class="row" x="${LEDGER.x + 42}" y="${y}">${target}</text><text class="row m ${statusCls}" x="${LEDGER.x + 156}" y="${y}">${status}</text><text class="row dim" x="${LEDGER.x + 200}" y="${y}">${note}</text></g>`;

  const c1 = CARDS[0];
  return tidy(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" role="img" aria-labelledby="ft fd">
<title id="ft">One request through Pygg's gateway</title>
<desc id="fd">A coding agent sends a request to Pygg on 127.0.0.1:8741. Pygg checks the client token, resolves the model (auto, a combo or a named connection/model), applies the key's policy and holds the estimated spend, and journals the attempt before sending it. In this example route, the connection's key ring sends it with key 1, which gets a 429 and cools down; key 2 gets a 402 and cools down; the request then goes to the route's next target, a local Ollama model, which answers. The answer streams back to the agent, and each attempt is settled in the ledger with its cost and that cost's source.</desc>
${defs(t)}
<style>${css}</style>
<rect width="${W}" height="${H}" rx="16" fill="url(#bgGrad)"/>
<text class="h" x="16" y="32" style="font-size:11px">ONE REQUEST THROUGH PYGG · AN EXAMPLE ROUTE</text>
<circle cx="${W - 356}" cy="28" r="5" fill="${t.request}"/><text class="s" x="${W - 346}" y="32">request</text>
<circle cx="${W - 276}" cy="28" r="5" fill="${t.warn}"/><text class="s" x="${W - 266}" y="32">refused: next key or target</text>
<circle cx="${W - 92}" cy="28" r="5" fill="${t.accent}"/><text class="s" x="${W - 82}" y="32">answer</text>
${box('agent', 0, 'Coding agent', [{ text: 'Claude Code, Codex,' }, { text: 'OpenCode, SDKs' }])}
${box('gw', 1, 'Pygg gateway', [{ text: '127.0.0.1:8741', mono: true }, { text: 'token, loopback only' }])}
${box('route', 2, 'Route', [], `${chip(COLS[2] + 14, R1.y + 50, 48, 'auto')}${chip(COLS[2] + 68, R1.y + 50, 62, 'combo')}${chip(COLS[2] + 14, R1.y + 74, 136, 'connection/model')}`)}
${box('policy', 3, 'Policy · budget', [{ text: 'limits, cooldowns' }, { text: 'spend held if capped' }])}
${box('journal', 4, 'Journal', [{ text: 'written to disk' }, { text: 'before it is sent' }])}
${arrowR(COLS[0] + BW, COLS[1], cy1)}${arrowR(COLS[1] + BW, COLS[2], cy1)}${arrowR(COLS[2] + BW, COLS[3], cy1)}${arrowR(COLS[3] + BW, COLS[4], cy1)}
<path class="ln" d="M${BUS} ${R1.y + R1.h}V${mid(3)}"/>
${CARDS.map((c, i) => arrowL(BUS, RIGHT, mid(i))).join('')}
<g class="anim nt"><text class="s" x="${BUS + 10}" y="${mid(1) - 4}" style="fill:${t.warn};font-weight:600">next</text><text class="s" x="${BUS + 10}" y="${mid(1) + 12}" style="fill:${t.warn};font-weight:600">target</text></g>

<g filter="url(#soft)"><rect id="c1" class="box" x="${CARD.x}" y="${c1.y}" width="${CARD.w}" height="${c1.h}" rx="11"/></g>
<text class="t" x="${CARD.x + 14}" y="${c1.y + 23}" style="font-size:14px">API keys · key ring</text>
<text class="s" x="${CARD.x + 14}" y="${c1.y + 41}">OpenAI, Anthropic, Gemini, OpenRouter …</text>
<rect id="k1" class="chip" x="${CARD.x + 14}" y="${c1.y + 51}" width="58" height="21" rx="10.5"/><text class="ct m" x="${CARD.x + 43}" y="${c1.y + 65.5}" text-anchor="middle">key 1</text>
<rect id="k2" class="chip" x="${CARD.x + 78}" y="${c1.y + 51}" width="58" height="21" rx="10.5"/><text class="ct m" x="${CARD.x + 107}" y="${c1.y + 65.5}" text-anchor="middle">key 2</text>
<text class="s" x="${CARD.x + 146}" y="${c1.y + 66}" style="font-size:11.5px">other accounts, used in turn</text>
${badge('b429', RIGHT - 112, c1.y + 10, '429', t.warn)}${badge('b402', RIGHT - 58, c1.y + 10, '402', t.warn)}
${CARDS.slice(1).map((c) => `<g filter="url(#soft)"><rect id="${c.id}" class="box" x="${CARD.x}" y="${c.y}" width="${CARD.w}" height="${c.h}" rx="11"/></g>
<text class="t" x="${CARD.x + 14}" y="${c.y + 20}" style="font-size:14px">${c.title}</text><text class="s" x="${CARD.x + 14}" y="${c.y + 37}">${c.sub}</text>`).join('')}
${badge('b200', RIGHT - 58, CARDS[2].y + 12, '200', t.accent)}

<path class="lane" d="M${CARD.x} ${mid(2)}H398V${LANE}H${cx(0)}V${R1.y + R1.h + 7}"/><path d="M${cx(0)} ${R1.y + R1.h}l-4.5 8h9z" fill="${t.accent}"/>
<text class="s" x="${cx(0) + 20}" y="${LANE - 8}">streamed back · a fallback happens only before the agent gets its first byte</text>

<g filter="url(#soft)"><rect class="box" x="${LEDGER.x}" y="${LEDGER.y}" width="${LEDGER.w}" height="${LEDGER.h}" rx="12"/></g>
<text class="t" x="${LEDGER.x + 16}" y="${LEDGER.y + 27}">Ledger</text>
<text class="s" x="${LEDGER.x + 16}" y="${LEDGER.y + 46}">each attempt settled with its journal entry</text>
<text class="h" x="${LEDGER.x + 16}" y="${LEDGER.y + 74}">#</text><text class="h" x="${LEDGER.x + 42}" y="${LEDGER.y + 74}">TARGET</text><text class="h" x="${LEDGER.x + 156}" y="${LEDGER.y + 74}">HTTP</text><text class="h" x="${LEDGER.x + 200}" y="${LEDGER.y + 74}">OUTCOME</text>
<path class="ln" d="M${LEDGER.x + 16} ${LEDGER.y + 82}H${LEDGER.x + LEDGER.w - 16}" style="stroke-width:1"/>
${ledgerRow(LEDGER.y + 104, 1, 'API key 1', 'r1a', '…', 'dim', 'journaled, sent')}
${ledgerRow(LEDGER.y + 104, 1, 'API key 1', 'r1b', '429', 'no', 'key cools down')}
${ledgerRow(LEDGER.y + 128, 2, 'API key 2', 'r2a', '…', 'dim', 'journaled, sent')}
${ledgerRow(LEDGER.y + 128, 2, 'API key 2', 'r2b', '402', 'no', 'key cools down')}
${ledgerRow(LEDGER.y + 152, 3, 'Ollama', 'r3a', '…', 'dim', 'journaled, sent')}
${ledgerRow(LEDGER.y + 152, 3, 'Ollama', 'r3b', '200', 'ok', 'streaming')}
${ledgerRow(LEDGER.y + 152, 3, 'Ollama', 'r3c', '200', 'ok', 'tokens, cost, source')}
<path class="ln" d="M${LEDGER.x + 16} ${LEDGER.y + 172}H${LEDGER.x + LEDGER.w - 16}" style="stroke-width:1"/>
<text class="s" x="${LEDGER.x + 16}" y="${LEDGER.y + 197}">cost source: actual · estimated · unknown</text>
<text class="s" x="${LEDGER.x + 16}" y="${LEDGER.y + 216}">unknown is never shown as $0</text>

<text class="s" x="16" y="${H - 14}" style="font-size:12px">A timeout stops the request, and so does a 5xx unless the client allows retries: the provider may already have billed it.</text>
<g class="pkt"><circle class="pk halo" r="12"/><circle class="pk" r="6.5"/></g>
</svg>
`);
}

for (const [name, theme] of Object.entries(THEMES)) {
  for (const [base, make] of [['usage', usage], ['request-flow', flow]]) {
    const file = join(OUT, `${base}-${name}.svg`);
    writeFileSync(file, make(theme));
    console.log(file);
  }
}
