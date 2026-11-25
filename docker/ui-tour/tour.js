/*
 * Pygg setup tours, in Vietnamese and English: a first-setup tour in four stages (welcome card, 10 spotlighted steps,
 * a "what next" card) and short optional tours per screen, opened from a "?" launcher with a setup checklist. Each
 * step dims the screen, spotlights one setting and explains it; Next opens the right screen or sheet by itself.
 * The tour speaks the app's own language (the VI/EN switch in the toolbar, or Settings → Language) and finds
 * settings by either language's label. Built on Driver.js (MIT, https://driverjs.com), loaded before this file by
 * docker/Dockerfile.ui; UI patterns from Mobbin. The app's own source is unchanged.
 */
(() => {
  'use strict';
  if (window.__pyggTour) return;
  window.__pyggTour = true;

  // ---------- language: the app's own choice (lib/native.ts), "system" resolved like lib/i18n.ts ----------
  const LANG = 'smarttaskrouter.language';
  function lang() {
    const v = localStorage.getItem(LANG);
    if (v === 'vi' || v === 'en') return v;
    const tags = navigator.languages?.length ? navigator.languages : [navigator.language || ''];
    return tags.some(t => /^vi\b/i.test(t)) ? 'vi' : 'en';
  }
  /** A text in both languages: {vi, en} or a plain string shared by both. */
  const tr = o => (o && typeof o === 'object' && !Array.isArray(o) ? o[lang()] ?? o.en : o);
  /** A label as the app shows it: [English, Vietnamese]; matching accepts either. */
  const L = (en, vi) => [en, vi ?? en];
  const opts = t => (Array.isArray(t) ? t : [t]);
  const U = {
    vi: {later: 'Để sau', start: 'Bắt đầu', next: 'Tiếp', prev: '← Lùi', done: 'Xong', more: 'Chi tiết', keys: 'để chuyển bước',
      guide: 'Hướng dẫn cấu hình', stage: 'Chặng', setup: 'Thiết lập Pygg', stages: 'chặng', allDone: 'Đã xong ✓', begin: 'Bắt đầu hướng dẫn',
      cont: 'Tiếp tục hướng dẫn', again: 'Xem lại hướng dẫn', learn: 'Tìm hiểu thêm', steps: 'bước', seen: '✓ Đã xem', resume: 'Tiếp tục',
      copy: 'Chép', copied: 'Đã chép ✓', hide: 'Ẩn', nextUp: 'Tiếp theo', showCard: 'Hiện lại thẻ thiết lập ở thanh bên', noLib: 'Không tải được thư viện tour (Driver.js).', language: 'Ngôn ngữ'},
    en: {later: 'Later', start: 'Start', next: 'Next', prev: '← Back', done: 'Done', more: 'Details', keys: 'to move between steps',
      guide: 'Setup guide', stage: 'Stage', setup: 'Set up Pygg', stages: 'stages', allDone: 'Done ✓', begin: 'Start the guide',
      cont: 'Continue the guide', again: 'Replay the guide', learn: 'Learn more', steps: 'steps', seen: '✓ Seen', resume: 'Continue',
      copy: 'Copy', copied: 'Copied ✓', hide: 'Hide', nextUp: 'Next up', showCard: 'Show the setup card in the sidebar again', noLib: 'The tour library (Driver.js) did not load.', language: 'Language'},
  };
  const ui = k => U[lang()][k];

  // ---------- finding things ----------
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  const shown = e => !!e && e.getClientRects().length > 0;
  const main = () => document.querySelector('main .content') || document.querySelector('main') || document.body;
  const sheet = () => $$('.sheet').find(shown) || null;
  const sheetTitle = () => { const s = sheet(); return s ? (s.innerText || '').trim().split('\n')[0].trim() : ''; };
  const title = () => (document.querySelector('header.toolbar .toolbar-title h1') || {}).textContent?.trim() || '';
  const starts = (text, label) => opts(label).some(o => text === o || text.startsWith(o));
  const isScreen = name => opts(name).includes(title());
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  async function until(test, ms = 2500) {
    const end = Date.now() + ms;
    while (Date.now() < end) { try { if (test()) return true; } catch (e) { /* not yet */ } await sleep(60); }
    return false;
  }

  /** The deepest visible element whose text is, or starts with, the label (in either language). */
  function find(label, scope) {
    let best = null;
    for (const e of (scope || main()).querySelectorAll('*')) {
      if (!shown(e)) continue;
      if (starts((e.textContent || '').trim(), label)) { if (!best || best.contains(e)) best = e; }
    }
    return best;
  }
  /** A labelled setting: from its label up to the element that also holds its control. */
  function row(label, scope) {
    let e = find(label, scope);
    while (e && e.parentElement && !e.querySelector('input,select,textarea,button,[role=switch]')) e = e.parentElement;
    return e;
  }
  const btn = (label, scope) => $$('button', scope || main()).find(b => shown(b) && starts(b.textContent.trim(), label)) || null;
  const exactBtn = (label, scope) => $$('button', scope || main()).find(b => shown(b) && opts(label).includes(b.textContent.trim())) || null;
  const group = label => { const h = find(label); return h ? h.closest('section.group') || h.closest('section') || h.parentElement : null; };
  const lastButtonOf = label => { let c = find(label); while (c && !c.querySelector('button')) c = c.parentElement; return c && $$('button', c).pop(); };
  /** The smallest element holding both a label and its value, framed together and nothing more. */
  function around(a, b) { if (!a || !b) return a || b; let p = a; while (p && !p.contains(b)) p = p.parentElement; return p; }
  const gatewayCard = () => document.querySelector('main section.gateway-card');
  const firstKeyRow = () => main().querySelector('table tbody tr');
  const firstModelRow = () => main().querySelector('table.models-table tbody tr');

  // ---------- the app's labels the tour looks for ----------
  const N = {
    home: L('Home', 'Trang chủ'), providers: L('Providers', 'Nhà cung cấp'), endpoint: L('Endpoint'), cli: L('CLI Tools', 'Công cụ CLI'),
    combos: L('Combos', 'Combo'), usage: L('Usage', 'Mức dùng'), policies: L('Policies', 'Chính sách'),
    configure: L('Configure', 'Cấu hình'), gatewaySettings: L('Gateway Settings', 'Cài đặt gateway'), limitsFor: L('Limits for', 'Giới hạn cho'),
    connect: L('Connect Provider', 'Kết nối nhà cung cấp'), cancel: L('Cancel', 'Hủy'), limits: L('Limits', 'Giới hạn'),
    port: L('Port', 'Cổng'), startWithApp: L('Start with the app', 'Bật cùng ứng dụng'), address: L('Address', 'Địa chỉ'),
    listensOnly: L('The gateway listens on 127.0.0.1 only', 'Gateway chỉ lắng nghe'),
    monthly: L('Monthly limit', 'Hạn mức hằng tháng'), session: L('Session limit', 'Hạn mức mỗi phiên'), strict: L('Strict limit', 'Giới hạn nghiêm ngặt'),
    unknownPrice: L('Unknown price', 'Giá chưa rõ'), afterError: L('After a server error', 'Sau lỗi máy chủ'),
    autoTier: L('Auto minimum tier', 'Tier tối thiểu cho auto'), connections: L('Connections', 'Kết nối'),
    billing: L('Billing', 'Tính phí'), refresh: L('Check & Refresh Models', 'Kiểm tra & làm mới model'),
    apiCosts: L('API Costs', 'Chi phí API'), autoRouting: L('Automatic Routing', 'Định tuyến tự động'), newCombo: L('New Combo', 'Combo mới'),
    apiKey: L('API Key', 'API key'), changeProvider: L('Change Provider', 'Đổi nhà cung cấp'),
    createKey: L('Create API Key', 'Tạo API key'), baseUrls: L('Base URLs', 'Base URL'),
    claudeRow: L('Claude Code: models through Pygg', 'Claude Code: model qua Pygg'),
    autoLine: L('Auto, for a coding tool', 'Auto, cho request của công cụ lập trình'), autoSkips: L('Auto skips', 'Auto bỏ qua'),
    more: L('More', 'Tuỳ chọn'), minTier: L('Minimum model tier', 'Tier model tối thiểu'), limitMode: L('Limit mode', 'Chế độ hạn mức'),
    saveDefaults: L('Save Defaults', 'Lưu mặc định'), howAuto: L('How auto chooses', 'Cách auto chọn model'),
  };

  // ---------- getting to a step's screen ----------
  const SHEETS = {
    gateway: () => starts(sheetTitle(), N.gatewaySettings),
    limits: () => starts(sheetTitle(), N.limitsFor),
    pick: () => starts(sheetTitle(), N.connect),
    form: () => !!sheet() && !!btn(N.changeProvider, sheet()),
  };
  async function closeSheet() {
    const s = sheet(); if (!s) return;
    const cancel = exactBtn(N.cancel, s);
    if (cancel) cancel.click(); else document.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));
    await until(() => !sheet());
  }
  async function openScreen(name) {
    if (isScreen(name)) return;
    const b = $$('aside button').find(x => starts(x.textContent.trim(), name));
    if (b) { b.click(); await until(() => isScreen(name)); await sleep(150); }
  }
  async function openSheet(kind) {
    if (SHEETS[kind]()) return;
    if (kind === 'gateway') btn(N.configure, gatewayCard())?.click();
    if (kind === 'limits') btn(N.limits, firstKeyRow())?.click();
    if (kind === 'pick' || kind === 'form') {
      if (!sheet()) { btn(N.connect, document.body)?.click(); await until(() => SHEETS.pick() || SHEETS.form()); }
      if (kind === 'pick' && SHEETS.form()) btn(N.changeProvider, sheet())?.click();
      const groq = () => sheet() && $$('button', sheet()).find(b => shown(b) && b.textContent.includes('Groq'));
      if (kind === 'form' && !SHEETS.form()) { await until(groq); groq()?.click(); }
    }
    await until(() => SHEETS[kind]());
    await sleep(150);
  }
  /** The two halves of the connect sheet are one sheet: moving between them never closes it. */
  const sameSheet = kind => kind && (SHEETS[kind]() || ((kind === 'pick' || kind === 'form') && (SHEETS.pick() || SHEETS.form())));
  async function prepare(st) {
    if (sheet() && !sameSheet(st.sheet)) await closeSheet();
    if (!sheet()) await openScreen(st.screen);
    if (st.sheet) await openSheet(st.sheet);
    if (st.prep) { await st.prep(); await sleep(150); }
    await until(() => shown(pick(st)));
  }
  function pick(st) { if (!st.el) return undefined; try { const e = st.el(); return (Array.isArray(e) ? e.find(shown) : e) || undefined; } catch (e) { return undefined; } }

  // ---------- the tours ----------
  const S = (screen, title, desc, el, extra = {}) => ({screen, title, desc, el, ...extra});
  const STAGES = [{vi: 'Gateway', en: 'Gateway'}, {vi: 'Nhà cung cấp', en: 'Provider'}, {vi: 'API key', en: 'API key'}, {vi: 'Công cụ & chi phí', en: 'Tools & costs'}];
  const usageCard = () => { const h = $$('h2', main()).find(x => opts(N.apiCosts).includes(x.textContent.trim())); return h && (h.closest('section.group') || h.parentElement); };
  const QUICK = [
    S(N.home, {vi: 'Thiết lập Pygg trong khoảng 3 phút', en: 'Set up Pygg in about 3 minutes'},
      {vi: 'Pygg chạy trên máy bạn và chuyển mỗi request của công cụ code tới một model bạn đã nối. Tour đi qua 4 chặng:', en: 'Pygg runs on your computer and sends each request from your coding tools to a model you connected. The guide has 4 stages:'}, null, {kind: 'welcome'}),
    S(N.home, {vi: 'Địa chỉ gateway', en: 'The gateway address'},
      {vi: 'Mọi công cụ code sẽ gọi vào <code>http://127.0.0.1:8741</code> với model <code>auto</code>. Chỉ máy này truy cập được.', en: 'Every coding tool calls <code>http://127.0.0.1:8741</code> with the model <code>auto</code>. Only this computer can reach it.'},
      () => find('http://', gatewayCard())?.parentElement, {stage: 0}),
    S(N.providers, {vi: 'Nối một nhà cung cấp', en: 'Connect a provider'},
      {vi: 'Pygg không có model riêng — cần ít nhất một tài khoản nhà cung cấp.', en: 'Pygg has no models of its own: connect at least one provider account.'},
      () => btn(N.connect, document.body), {stage: 1}),
    S(N.providers, {vi: 'Chọn nhà cung cấp', en: 'Pick a provider'},
      {vi: 'Nên bắt đầu với bên có gói miễn phí: Groq, Gemini, OpenRouter — hoặc Ollama chạy trên máy.', en: 'Start with one that has a free tier: Groq, Gemini, OpenRouter — or Ollama on this computer.'},
      () => $$('button', sheet()).find(b => b.textContent.includes('Groq')), {stage: 1, sheet: 'pick'}),
    S(N.providers, {vi: 'Dán API key', en: 'Paste the API key'},
      {vi: 'Dán key rồi bấm "Kết nối". Pygg chỉ đọc danh sách model, không tốn tiền.', en: 'Paste the key, then Connect. Pygg only reads the model list, which costs nothing.'},
      () => row(N.apiKey, sheet()), {stage: 1, sheet: 'form', more: {vi: 'Key lưu trong Keychain / Credential Manager và không bao giờ hiện lại.', en: 'The key goes to the Keychain / Credential Manager and is never shown again.'}}),
    S(N.providers, {vi: 'Kiểm tra auto có model dùng được', en: 'Check that auto has a model'},
      {vi: 'Số "model đủ điều kiện" phải lớn hơn 0, nếu không công cụ gọi auto sẽ bị từ chối.', en: '"models eligible" must be above 0, or tools asking for auto are refused.'},
      () => find(N.autoLine)?.parentElement, {stage: 1, more: {vi: 'Bấm vào số "thiếu thông tin định tuyến" để bổ sung giá và tier cho các model đang thiếu.', en: 'Click "missing routing facts" to add the price and tier the models lack.'}}),
    S(N.endpoint, {vi: 'Tạo API key cho mỗi công cụ', en: 'One API key per tool'},
      {vi: 'Key của Pygg (không phải key nhà cung cấp) cho biết công cụ nào tiêu bao nhiêu. Token chỉ hiện một lần.', en: 'A Pygg key (not a provider key) shows what each tool spends. The token is shown once.'},
      () => btn(N.createKey, document.body), {stage: 2}),
    S(N.endpoint, {vi: 'Đặt hạn mức cho key', en: 'Set the key’s limits'},
      {vi: 'Đặt hạn mức tháng / ngày / phiên ngay từ đầu. Chạm hạn mức thì request bị từ chối.', en: 'Set a monthly, daily or session limit from the start. At the limit, requests are refused.'},
      () => btn(N.limits, firstKeyRow()), {stage: 2, more: {vi: 'Request đang chạy không bị cắt giữa chừng. Tour "Hạn mức của một key" giải thích từng ô.', en: 'A request already running is never cut. The "A key’s limits" tour explains each field.'}}),
    S(N.endpoint, {vi: 'Ba thứ công cụ cần', en: 'The three things a tool needs'},
      {vi: 'URL, API key vừa tạo, và model <code>auto</code>.', en: 'The URL, the API key you made, and the model <code>auto</code>.'},
      () => group(N.baseUrls), {stage: 2}),
    S(N.cli, {vi: 'Trỏ công cụ code vào Pygg', en: 'Point your coding tool at Pygg'},
      {vi: '"Cài đặt" lo hết cho Claude Code, OpenCode, Codex; sau đó chạy <code>pygg run claude</code>.', en: '"Set Up" does it for Claude Code, OpenCode and Codex; then run <code>pygg run claude</code>.'},
      () => lastButtonOf(N.claudeRow), {stage: 3, more: {vi: 'Trên macOS chạy bằng Docker: dùng "Cài thủ công" với token trong <code>docker/token</code>.', en: 'On macOS with Docker: use "Set up by hand" with the token in <code>docker/token</code>.'}}),
    S(N.usage, {vi: 'Xem chi phí', en: 'See what it cost'},
      {vi: 'Mỗi request ghi chi phí thực tế, ước tính, hoặc "chưa rõ" — không bao giờ ghi $0.', en: 'Each request records an actual, estimated or "unknown" cost — never a made-up $0.'},
      usageCard, {stage: 3}),
    S(N.home, {vi: 'Pygg đã sẵn sàng', en: 'Pygg is ready'}, {vi: 'Việc tiếp theo:', en: 'What next:'}, null, {kind: 'finish'}),
  ];
  const toCombos = async () => { const b = exactBtn(N.combos); if (find(N.autoRouting) && b) { b.click(); await until(() => !find(N.autoRouting)); } };
  const toSimulator = async () => { if (!find(N.autoRouting)) { btn(N.howAuto)?.click(); await until(() => !!find(N.autoRouting)); } };
  const TOURS = [
    {id: 'quick', name: {vi: 'Cấu hình lần đầu', en: 'First setup'}, note: {vi: '4 chặng, khoảng 3 phút', en: '4 stages, about 3 minutes'}, steps: QUICK},
    {id: 'providers', name: {vi: 'Model, giá và auto', en: 'Models, prices and auto'}, note: {vi: 'Vì sao auto chọn hay bỏ qua một model', en: 'Why auto picks or skips a model'}, steps: [
      S(N.providers, {vi: 'Một kết nối', en: 'A connection'}, {vi: 'Dải chấm là sức khoẻ 24 giờ; "n for auto" là số model auto dùng được.', en: 'The dots are its last 24 hours; "n for auto" is how many models auto may use.'}, () => { const r = main().querySelector('li.select-row'); return r && (r.querySelector('button.select-row-main') || r); }),
      S(N.providers, {vi: 'Kiểm tra & làm mới model', en: 'Check & Refresh Models'}, {vi: 'Đọc lại model và tự điền context, tools, giá từ models.dev.', en: 'Reads the models again and fills context, tools and prices from models.dev.'}, () => btn(N.refresh), {more: {vi: 'Không đè giá trị bạn tự đặt.', en: 'Values you set yourself are kept.'}}),
      S(N.providers, {vi: 'Tính phí', en: 'Billing'}, {vi: '"Miễn phí" ghi mọi request $0 — chỉ chọn khi đúng là miễn phí.', en: '"Free" records every request at $0 — choose it only when it really is free.'}, () => row(N.billing)),
      S(N.providers, {vi: 'Vì sao auto bỏ qua', en: 'Why auto skips a model'}, {vi: 'Lý do nằm dưới mỗi model: thiếu tier, thiếu giá, hoặc tier thấp hơn T2.', en: 'The reason sits under each model: no tier, no price, or a tier below T2.'}, () => find(N.autoSkips)),
      S(N.providers, {vi: 'Giá và dữ kiện', en: 'Prices and facts'}, {vi: 'Tuỳ chọn → "Đặt giá…" và "Thông tin định tuyến auto…" để auto dùng được model.', en: 'More → "Set/Edit Price…" and "Auto Routing Facts…" make a model usable by auto.'}, () => btn(N.more, firstModelRow()), {more: {vi: 'Auto chỉ chọn model có tier ≥ T2, hỗ trợ tool, biết context, và có giá (hoặc miễn phí / cục bộ / thuê bao).', en: 'Auto only picks a model with tier T2 or above, tool support, a known context size, and a price (or free / local / subscription billing).'}}),
    ]},
    {id: 'limits', name: {vi: 'Hạn mức của một key', en: 'A key’s limits'}, note: {vi: 'Tháng, phiên, nghiêm ngặt…', en: 'Monthly, session, strict…'}, steps: [
      S(N.endpoint, {vi: 'Hạn mức hằng tháng', en: 'Monthly limit'}, {vi: 'Đặt lại ngày 1 (UTC); chạm thì request bị từ chối.', en: 'Resets on the 1st (UTC); at the limit, requests are refused.'}, () => row(N.monthly, sheet()), {sheet: 'limits', more: {vi: 'Để trống = không giới hạn. Gõ "1,000" được hiểu là $1.', en: 'Empty means no limit. "1,000" is read as $1.'}}),
      S(N.endpoint, {vi: 'Hạn mức phiên (và ngày)', en: 'Session (and daily) limit'}, {vi: 'Giới hạn một lần <code>pygg run</code>; ngay dưới là hạn mức ngày (đặt lại 00:00 UTC).', en: 'Caps one <code>pygg run</code>; the daily limit below resets at 00:00 UTC.'}, () => row(N.session, sheet()), {sheet: 'limits'}),
      S(N.endpoint, {vi: 'Giới hạn nghiêm ngặt', en: 'Strict limit'}, {vi: 'Giữ chỗ chi phí tối đa trước khi gửi, nên không bao giờ vượt.', en: 'Holds the most a request could cost before sending, so the limit is never passed.'}, () => row(N.strict, sheet()), {sheet: 'limits', more: {vi: 'Đổi lại, model thiếu giá hoặc thiếu "output tối đa" sẽ không được dùng.', en: 'In exchange, models without a price or a maximum output are not used.'}}),
      S(N.endpoint, {vi: 'Giá chưa rõ', en: 'Unknown price'}, {vi: '"Chặn" bỏ qua model chưa có giá khi key có hạn mức.', en: '"Block" skips models without a price while the key has a limit.'}, () => row(N.unknownPrice, sheet()), {sheet: 'limits'}),
      S(N.endpoint, {vi: 'Sau lỗi máy chủ', en: 'After a server error'}, {vi: '"Dừng" (mặc định) tránh trả tiền hai lần khi nhà cung cấp lỗi giữa chừng.', en: '"Stop" (the default) avoids paying twice when a provider fails midway.'}, () => row(N.afterError, sheet()), {sheet: 'limits'}),
      S(N.endpoint, {vi: 'Tier tối thiểu cho auto', en: 'Auto minimum tier'}, {vi: 'T2 hợp agent viết code; thấp hơn rẻ hơn nhưng yếu hơn.', en: 'T2 suits coding agents; lower is cheaper but weaker.'}, () => row(N.autoTier, sheet()), {sheet: 'limits'}),
      S(N.endpoint, {vi: 'Kết nối được dùng', en: 'Connections it may use'}, {vi: 'Giới hạn key vào một số kết nối, bất kể combo hay fallback.', en: 'Keeps the key to some connections, whatever a combo or fallback says.'}, () => row(N.connections, sheet()), {sheet: 'limits', more: {vi: '"Chỉ kết nối đã chọn" mà không tick gì thì mọi request bị từ chối.', en: '"Only selected" with nothing ticked refuses every request.'}}),
    ]},
    {id: 'gateway', name: {vi: 'Gateway', en: 'Gateway'}, note: {vi: 'Cổng, khởi động', en: 'Port, start-up'}, steps: [
      S(N.home, {vi: 'Cài đặt gateway', en: 'Gateway settings'}, {vi: 'Mở phần cài đặt của gateway.', en: 'Opens the gateway’s settings.'}, () => btn(N.configure, gatewayCard())),
      S(N.home, {vi: 'Cổng', en: 'Port'}, {vi: 'Mặc định 8741; chỉ đổi khi cổng đã bị chương trình khác dùng.', en: '8741 by default; change it only if another program uses it.'}, () => row(N.port, sheet()), {sheet: 'gateway', more: {vi: 'Đổi cổng thì gateway tự khởi động lại và công cụ phải dùng URL mới.', en: 'A new port restarts the gateway, and tools need the new URL.'}}),
      S(N.home, {vi: 'Bật cùng ứng dụng', en: 'Start with the app'}, {vi: 'Nên để bật: gateway chạy ngay khi mở Pygg.', en: 'Keep it on: the gateway runs as soon as Pygg opens.'}, () => row(N.startWithApp, sheet()), {sheet: 'gateway'}),
      S(N.home, {vi: 'Địa chỉ', en: 'Address'}, {vi: 'Luôn 127.0.0.1: chỉ máy này dùng được gateway.', en: 'Always 127.0.0.1: only this computer can use the gateway.'}, () => around(find(N.address, sheet()), find(N.listensOnly, sheet())), {sheet: 'gateway'}),
    ]},
    {id: 'policies', name: {vi: 'Mặc định cho key mới', en: 'Defaults for new keys'}, note: {vi: 'Chính sách', en: 'Policies'}, steps: [
      S(N.policies, {vi: 'Tier tối thiểu', en: 'Minimum tier'}, {vi: 'Áp cho key tạo sau khi lưu; key đã có sửa ở Endpoint → Giới hạn….', en: 'Applies to keys made after saving; change existing keys in Endpoint → Limits….'}, () => row(N.minTier)),
      S(N.policies, {vi: 'Hạn mức tháng mặc định', en: 'Default monthly limit'}, {vi: 'Để không key mới nào chạy không giới hạn.', en: 'So no new key runs without a limit.'}, () => row(N.monthly)),
      S(N.policies, {vi: 'Chế độ hạn mức', en: 'Limit mode'}, {vi: '"Ước tính" có thể vượt chút; "Nghiêm ngặt" giữ chỗ trước nên không vượt.', en: '"Estimated" can end a little over; "Strict" holds first, so it never does.'}, () => row(N.limitMode)),
      S(N.policies, {vi: 'Lưu', en: 'Save'}, {vi: 'Chỉ key tạo sau đó mới nhận các giá trị này.', en: 'Only keys made afterwards get these values.'}, () => btn(N.saveDefaults)),
    ]},
    {id: 'combos', name: {vi: 'Combo và auto', en: 'Combos and auto'}, note: {vi: 'Tự đặt thứ tự model; mô phỏng auto', en: 'Your own model order; simulate auto'}, steps: [
      S(N.combos, {vi: 'Combo (tuỳ chọn)', en: 'Combos (optional)'}, {vi: 'Tự chọn model và thứ tự dự phòng; công cụ gửi tên combo làm model.', en: 'Choose models and their fallback order; tools send the combo’s name as the model.'}, () => btn(N.newCombo, document.body), {prep: toCombos}),
      S(N.combos, {vi: 'Auto sẽ chọn gì?', en: 'What would auto pick?'}, {vi: 'Mô phỏng auto cho một request mà không gửi gì.', en: 'Simulates auto for a request without sending anything.'}, () => btn(N.howAuto), {prep: toCombos}),
      S(N.combos, {vi: 'Kết quả mô phỏng', en: 'The simulated route'}, {vi: 'Tối đa 5 model, theo thứ tự sẽ thử, kèm lý do.', en: 'Up to 5 models, in the order they would be tried, with the reason.'}, () => main().querySelector('table'), {prep: toSimulator}),
    ]},
  ];
  const tourById = id => TOURS.find(t => t.id === id) || TOURS[0];

  // ---------- what the user has done (kept in this browser) ----------
  const store = {
    get: (k, d) => { try { return JSON.parse(localStorage.getItem('pygg.tour.' + k)) ?? d; } catch (e) { return d; } },
    set: (k, v) => localStorage.setItem('pygg.tour.' + k, JSON.stringify(v)),
  };
  const done = () => store.get('done', []);
  const markDone = id => { const d = done(); if (!d.includes(id)) store.set('done', [...d, id]); };
  const resumeAt = () => store.get('resume', null);
  /** Stages of the first-setup tour reached: by finishing it, or by how far a paused run got. */
  function quickStagesDone() {
    if (done().includes('quick')) return STAGES.length;
    const r = resumeAt(); const top = Math.max(store.get('quickMax', 0), r && r.tour === 'quick' ? r.index : 0);
    return STAGES.filter((_, s) => QUICK.some(st => st.stage === s) && QUICK.every((st, i) => st.stage !== s || i < top)).length;
  }

  // ---------- look ----------
  const css = `
  .driver-popover.pygg-tour{--pa:var(--accent,#0D8551);--pt:var(--accent-text,#3DDC97);background:var(--surface,#2C2C2E);color:var(--label,#e8e8ea);border:1px solid var(--separator,rgba(255,255,255,.09));border-radius:12px;max-width:340px;padding:16px 16px 12px;box-shadow:var(--popover-shadow,0 16px 44px rgba(0,0,0,.55))}
  .driver-popover.pygg-tour .driver-popover-title{font:600 15px/1.35 -apple-system,system-ui,sans-serif;color:var(--label,#fff)}
  .driver-popover.pygg-tour .pgt-eyebrow{display:block;font:600 11px/1.4 -apple-system,system-ui,sans-serif;color:var(--pt);margin-bottom:4px}
  .driver-popover.pygg-tour .driver-popover-description{font:13.5px/1.5 -apple-system,system-ui,sans-serif;color:var(--label,#cfcfd3)}
  .driver-popover.pygg-tour code{font:12px ui-monospace,SFMono-Regular,Menlo,monospace;background:var(--separator,rgba(255,255,255,.08));border-radius:4px;padding:1px 5px;color:var(--label,#fff)}
  .driver-popover.pygg-tour details{margin-top:8px;color:var(--secondary,#a1a1a6);font-size:12.5px}
  .driver-popover.pygg-tour details summary{cursor:pointer;color:var(--pt);list-style:none}
  .driver-popover.pygg-tour details summary::before{content:'›';display:inline-block;margin-right:5px;transition:transform .15s}
  .driver-popover.pygg-tour details[open] summary::before{transform:rotate(90deg)}
  .driver-popover.pygg-tour .driver-popover-footer{margin-top:14px;align-items:center}
  .driver-popover.pygg-tour .driver-popover-progress-text{display:none}
  .driver-popover.pygg-tour .pgt-progress{flex:1 1 auto;min-width:130px;display:flex;flex-direction:column;align-items:flex-start;gap:5px;color:var(--secondary,#8e8e93);font:11.5px -apple-system,system-ui,sans-serif;text-align:left}
  .driver-popover.pygg-tour .pgt-progress span{white-space:nowrap}
  .driver-popover.pygg-tour .pgt-bar{width:100%}
  .driver-popover.pygg-tour .driver-popover-navigation-btns{flex:none}
  .driver-popover.pygg-tour .pgt-bar{display:flex;gap:3px}
  .driver-popover.pygg-tour .pgt-bar i{flex:1;height:3px;border-radius:2px;background:var(--track,rgba(255,255,255,.12))}
  .driver-popover.pygg-tour .pgt-bar i.on{background:var(--pt)}
  .driver-popover.pygg-tour .pgt-dots{display:flex;gap:5px}
  .driver-popover.pygg-tour .pgt-dots i{width:6px;height:6px;border-radius:3px;background:var(--track,rgba(255,255,255,.18))}
  .driver-popover.pygg-tour .pgt-dots i.on{background:var(--pt);width:14px}
  .driver-popover.pygg-tour kbd{font:10.5px ui-monospace,Menlo,monospace;border:1px solid var(--track,rgba(255,255,255,.18));border-radius:3px;padding:0 3px;color:var(--secondary,#a1a1a6)}
  .driver-popover.pygg-tour .driver-popover-footer button{text-shadow:none;font:600 12.5px -apple-system,system-ui,sans-serif;border-radius:7px;padding:6px 12px}
  .driver-popover.pygg-tour .driver-popover-prev-btn{background:none;border:0;color:var(--secondary,#a1a1a6)}
  .driver-popover.pygg-tour .driver-popover-prev-btn:hover{color:var(--label,#fff)}
  .driver-popover.pygg-tour .driver-popover-next-btn{background:var(--pa);border:1px solid var(--pa);color:#fff}
  .driver-popover.pygg-tour .driver-popover-next-btn:hover{filter:brightness(1.12)}
  .driver-popover.pygg-tour .driver-popover-close-btn{color:var(--secondary,#8e8e93)}
  .driver-popover.pygg-tour .driver-popover-arrow-side-left{border-left-color:var(--surface,#2C2C2E)}
  .driver-popover.pygg-tour .driver-popover-arrow-side-right{border-right-color:var(--surface,#2C2C2E)}
  .driver-popover.pygg-tour .driver-popover-arrow-side-top{border-top-color:var(--surface,#2C2C2E)}
  .driver-popover.pygg-tour .driver-popover-arrow-side-bottom{border-bottom-color:var(--surface,#2C2C2E)}
  .driver-popover.pygg-tour .pgt-skip{background:none!important;border:0!important;color:var(--secondary,#8e8e93)!important;padding:6px 4px!important;margin-right:auto;font-weight:500!important}
  .driver-popover.pygg-tour.pgt-center{max-width:420px;padding:22px 22px 14px}
  .driver-popover.pygg-tour.pgt-center .driver-popover-title{font-size:18px}
  .driver-popover.pygg-tour ol.pgt-stages{margin:12px 0 0;padding:0;list-style:none;counter-reset:s}
  .driver-popover.pygg-tour ol.pgt-stages li{counter-increment:s;display:flex;gap:10px;align-items:center;padding:6px 0;color:var(--label,#e8e8ea)}
  .driver-popover.pygg-tour ol.pgt-stages li::before{content:counter(s);width:22px;height:22px;border-radius:11px;background:rgba(61,220,151,.14);color:var(--pt);font:600 12px/22px -apple-system,system-ui,sans-serif;text-align:center}
  .driver-popover.pygg-tour .pgt-next{display:grid;gap:8px;margin-top:12px}
  .driver-popover.pygg-tour .pgt-next button,.driver-popover.pygg-tour .pgt-next div{display:block;text-align:left;width:100%;background:var(--field,rgba(255,255,255,.05));border:1px solid var(--separator,rgba(255,255,255,.08));border-radius:9px;padding:9px 11px;color:var(--label,#e8e8ea);font:13px -apple-system,system-ui,sans-serif;cursor:pointer}
  .driver-popover.pygg-tour .pgt-next button:hover{border-color:var(--pt)}
  .driver-popover.pygg-tour .pgt-next em{display:block;font-style:normal;color:var(--secondary,#8e8e93);font-size:12px}
  .driver-popover.pygg-tour .pgt-next .copy{float:right;color:var(--pt);font-size:12px}
  .driver-active-element{outline:2px solid var(--accent-text,#3DDC97)!important;outline-offset:3px;border-radius:6px}
  @media (prefers-reduced-motion:reduce){.driver-popover{animation:none!important}}
  #pgt-panel{position:fixed;z-index:2147483000;width:320px;background:var(--surface,#2C2C2E);color:var(--label,#e8e8ea);border:1px solid var(--separator,var(--separator,rgba(255,255,255,.09)));border-radius:12px;padding:14px;box-shadow:var(--popover-shadow,var(--popover-shadow,0 16px 44px rgba(0,0,0,.55)));display:none;font:13px var(--font,-apple-system,system-ui,sans-serif)}
  .pgt-help{position:relative}
  .pgt-help .pgt-dot{position:absolute;top:4px;right:4px;width:7px;height:7px;border-radius:4px;background:var(--accent-text,#3DDC97);box-shadow:0 0 0 2px var(--window,#1c1c1e)}
  #pgt-card{margin:0 10px 8px;padding:10px 11px;border-radius:9px;background:var(--field,var(--hover,rgba(255,255,255,.06)));font:12px var(--font,-apple-system,system-ui,sans-serif);color:var(--label,#e8e8ea)}
  #pgt-card .top{display:flex;align-items:center;justify-content:space-between;margin-bottom:7px}
  #pgt-card .open{background:none;border:0;padding:0;font:600 12.5px var(--font,-apple-system,system-ui,sans-serif);color:var(--label,#e8e8ea);cursor:pointer}
  #pgt-card .x{background:none;border:0;padding:0 2px;font-size:15px;line-height:1;color:var(--secondary,#8e8e93);cursor:pointer}
  #pgt-card .x:hover{color:var(--label,#e8e8ea)}
  #pgt-card .bar{height:4px;border-radius:2px;background:var(--track,var(--track,rgba(255,255,255,.12)));overflow:hidden}
  #pgt-card .bar i{display:block;height:100%;background:var(--accent-text,#3DDC97)}
  #pgt-card .meta b{font-weight:500;color:var(--label,#e8e8ea)}
  #pgt-card .meta{margin:6px 0 8px;color:var(--secondary,#8e8e93);font-size:11.5px;line-height:1.35}
  #pgt-card .go{width:100%;height:24px;border:0;border-radius:6px;background:var(--accent,#0D8551);color:#fff;font:600 12px var(--font,-apple-system,system-ui,sans-serif);cursor:pointer}
  #pgt-card .go:focus-visible,#pgt-card .open:focus-visible,#pgt-card .x:focus-visible{outline:none;box-shadow:var(--focus,0 0 0 3px rgba(13,133,81,.5));border-radius:5px}
  #pgt-panel.open{display:block}
  #pgt-panel h3{margin:0 0 2px;font:600 14px -apple-system,system-ui,sans-serif;color:var(--label,#fff)}
  #pgt-panel .sub{color:var(--secondary,#8e8e93);font-size:12px;margin-bottom:10px}
  #pgt-panel .bar{height:4px;border-radius:2px;background:var(--track,rgba(255,255,255,.12));overflow:hidden;margin-bottom:10px}
  #pgt-panel .bar i{display:block;height:100%;background:var(--accent-text,#3DDC97)}
  #pgt-panel ul{list-style:none;margin:0 0 10px;padding:0}
  #pgt-panel li{display:flex;gap:9px;align-items:center;padding:5px 0;font-size:13px}
  #pgt-panel li b{width:18px;height:18px;border-radius:9px;border:1.5px solid var(--secondary,rgba(255,255,255,.25));font:700 11px/16px -apple-system,system-ui,sans-serif;text-align:center;color:transparent;flex:none}
  #pgt-panel li.ok b{background:var(--accent-text,#3DDC97);border-color:var(--accent-text,#3DDC97);color:#0b0b0b}
  #pgt-panel li.ok span{color:var(--secondary,#8e8e93);text-decoration:line-through}
  #pgt-panel .primary{width:100%;background:var(--accent,#0D8551);color:#fff;border:0;border-radius:8px;padding:8px;font:600 13px -apple-system,system-ui,sans-serif;cursor:pointer;margin-bottom:12px}
  #pgt-panel .more{border-top:1px solid var(--separator,rgba(255,255,255,.08));padding-top:8px;color:var(--secondary,#8e8e93);font-size:11px;text-transform:uppercase;letter-spacing:.04em}
  #pgt-panel .t{display:flex;justify-content:space-between;align-items:center;width:100%;background:none;border:0;color:var(--label,#e8e8ea);text-align:left;padding:7px 6px;border-radius:7px;font:13px -apple-system,system-ui,sans-serif;cursor:pointer}
  #pgt-panel .t:hover{background:var(--hover,rgba(255,255,255,.06))}
  #pgt-panel .t em{font-style:normal;color:var(--secondary,#8e8e93);font-size:11.5px}
  #pgt-panel .t em.seen{color:var(--accent-text,#3DDC97)}
  `;
  const style = document.createElement('style'); style.textContent = css; document.head.appendChild(style);


  // ---------- the tour engine ----------
  let drv = null, moving = false, tour = null, finished = false;

  function describe(st) {
    let html = tr(st.desc);
    if (st.kind === 'welcome') html += '<ol class="pgt-stages">' + STAGES.map(s => `<li>${tr(s)}</li>`).join('') + '</ol>';
    if (st.kind === 'finish') html += '<div class="pgt-next">'
      + `<button type="button" data-copy="pygg run claude"><span class="copy">${ui('copy')}</span>${lang() === 'vi' ? 'Chạy' : 'Run'} <code>pygg run claude</code><em>${tr({vi: 'Claude Code đi qua Pygg với model auto', en: 'Claude Code through Pygg on the model auto'})}</em></button>`
      + ['limits', 'providers'].map(id => { const t = tourById(id); return `<button type="button" data-tour="${id}">${tr(t.name)}<em>${t.steps.length} ${ui('steps')} · ${tr(t.note)}</em></button>`; }).join('')
      + '</div>';
    if (st.more) html += `<details><summary>${ui('more')}</summary>${tr(st.more)}</details>`;
    return html;
  }
  /** Progress under each step: the first-setup tour by stage, short tours by dots. */
  function progressHtml(t, i) {
    const st = t.steps[i];
    if (st.kind) return `<span><kbd>←</kbd> <kbd>→</kbd> ${ui('keys')}</span>`;
    if (t.id === 'quick') {
      const inStage = QUICK.filter(s => s.stage === st.stage); const pos = inStage.indexOf(st) + 1;
      return `<div class="pgt-bar">${STAGES.map((_, s) => `<i class="${s <= st.stage ? 'on' : ''}"></i>`).join('')}</div><span>${tr(STAGES[st.stage])} · ${pos}/${inStage.length}</span>`;
    }
    return `<div class="pgt-dots">${t.steps.map((_, k) => `<i class="${k === i ? 'on' : ''}"></i>`).join('')}</div><span>${i + 1}/${t.steps.length}</span>`;
  }

  async function prepareStep(st) { if (st.el) await prepare(st); else { if (sheet()) await closeSheet(); await openScreen(st.screen); } }
  async function go(i) {
    if (moving || !drv) return;
    moving = true;
    try { await prepareStep(tour.steps[i]); drv.moveTo(i); } finally { moving = false; }
  }
  function remember(i) {
    store.set('resume', {tour: tour.id, index: i});
    if (tour.id === 'quick') store.set('quickMax', Math.max(store.get('quickMax', 0), i));
  }

  async function start(id = 'quick', from = 0) {
    if (!window.driver?.js?.driver) { alert(ui('noLib')); return; }
    closePanel();
    if (drv && drv.isActive()) drv.destroy();
    tour = tourById(id); finished = false;
    const steps = tour.steps;
    const reduce = matchMedia('(prefers-reduced-motion: reduce)').matches;
    drv = window.driver.js.driver({
      steps: steps.map((st, i) => {
        const first = i === 0 || (i === 1 && steps[0].kind === 'welcome');
        const buttons = first || st.kind === 'welcome' ? ['next', 'close'] : ['next', 'previous', 'close'];
        const eyebrow = st.kind ? (tour.id === 'quick' ? ui('guide') : tr(tour.name)) : (tour.id === 'quick' ? `${ui('stage')} ${st.stage + 1} · ${tr(STAGES[st.stage])}` : tr(tour.name));
        return {
          element: st.el ? () => pick(st) : undefined,
          popover: {
            title: `<span class="pgt-eyebrow">${eyebrow}</span>${tr(st.title)}`, description: describe(st),
            showButtons: buttons, side: st.side || 'bottom', align: 'start',
            popoverClass: 'pygg-tour' + (st.el ? '' : ' pgt-center'),
            nextBtnText: st.kind === 'welcome' ? ui('start') : st.kind === 'finish' || i === steps.length - 1 ? ui('done') : ui('next'),
            prevBtnText: ui('prev'),
          },
        };
      }),
      animate: !reduce, smoothScroll: !reduce, overlayOpacity: 0.6, stagePadding: 6, stageRadius: 8,
      disableActiveInteraction: true, allowKeyboardControl: true,
      onPopoverRender: (pop, {state}) => {
        const i = state.activeIndex ?? 0, st = steps[i];
        pop.wrapper.setAttribute('aria-labelledby', 'pgt-title'); pop.wrapper.setAttribute('lang', lang()); pop.title.id = 'pgt-title';
        pop.description.setAttribute('aria-live', 'polite');
        const prog = document.createElement('div'); prog.className = 'pgt-progress'; prog.innerHTML = progressHtml(tour, i);
        pop.footer.insertBefore(prog, pop.footer.firstChild);
        if (st.kind === 'welcome') {
          const later = document.createElement('button'); later.type = 'button'; later.className = 'pgt-skip'; later.textContent = ui('later');
          later.onclick = () => { store.set('seen', true); store.set('resume', null); finished = true; drv.destroy(); };
          pop.footerButtons.insertBefore(later, pop.footerButtons.firstChild);
          prog.remove();
        }
        $$('[data-tour]', pop.description).forEach(b => b.onclick = () => { markDone(tour.id); finished = true; drv.destroy(); setTimeout(() => start(b.dataset.tour), 50); });
        $$('[data-copy]', pop.description).forEach(b => b.onclick = () => { navigator.clipboard?.writeText(b.dataset.copy); b.querySelector('.copy').textContent = ui('copied'); });
      },
      onHighlighted: () => remember(drv.getActiveIndex() ?? 0),
      onNextClick: () => {
        const i = drv.getActiveIndex();
        if (i >= steps.length - 1) { markDone(tour.id); store.set('resume', null); if (tour.id === 'quick') store.set('seen', true); finished = true; drv.destroy(); }
        else go(i + 1);
      },
      onPrevClick: () => { const i = drv.getActiveIndex(); if (i > 0) go(i - 1); },
      // Closing midway keeps the place: the launcher offers to continue.
      onDestroyStarted: () => { if (!finished && tour.id === 'quick') store.set('seen', true); drv.destroy(); },
      onDestroyed: () => { if (finished) store.set('resume', null); closeSheet(); renderLauncher(); help.focus(); },
    });
    await prepareStep(steps[from]);
    drv.drive(from);
  }
  window.pyggTour = {start, lang, tours: TOURS.map(t => `${t.id}:${t.steps.length}`)};

  // ---------- the launcher: a setup card in the sidebar and a "?" in the toolbar, both opening the checklist ----------
  // Inside the app's own frame (Searchable's and HoneyBook's sidebar widget): nothing floats over the content.
  const HELP_SVG = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><path d="M12 17h.01"/></svg>';
  const panel = document.createElement('div'); panel.id = 'pgt-panel'; panel.setAttribute('role', 'dialog'); document.body.appendChild(panel);
  const help = document.createElement('button'); help.type = 'button'; help.className = 'toolbar-button pgt-help'; help.setAttribute('aria-expanded', 'false');
  help.innerHTML = HELP_SVG + '<i class="pgt-dot" hidden></i>';
  const card = document.createElement('div'); card.id = 'pgt-card';
  let anchor = null;
  function placePanel() {
    if (!anchor || !panel.classList.contains('open')) return;
    const r = anchor.getBoundingClientRect(), w = panel.offsetWidth || 320;
    if (anchor === help) { panel.style.left = Math.max(8, Math.min(innerWidth - w - 8, r.right - w)) + 'px'; panel.style.top = (r.bottom + 8) + 'px'; panel.style.bottom = ''; }
    else { panel.style.left = (r.right + 10) + 'px'; panel.style.top = ''; panel.style.bottom = Math.max(8, innerHeight - r.bottom) + 'px'; }
  }
  const closePanel = () => { panel.classList.remove('open'); help.setAttribute('aria-expanded', 'false'); anchor = null; };
  const openPanel = a => { renderLauncher(); anchor = a; panel.classList.add('open'); help.setAttribute('aria-expanded', String(a === help)); placePanel(); };
  const togglePanel = a => (panel.classList.contains('open') && anchor === a ? closePanel() : openPanel(a));
  help.onclick = () => togglePanel(help);
  const showCard = () => !done().includes('quick') && !store.get('cardHidden', false);
  /** React owns the toolbar and the sidebar: put the two triggers back whenever a render drops or moves them. */
  function mount() {
    const bar = document.querySelector('header.toolbar');
    if (bar) {
      const sw = bar.querySelector('.language-switch');
      const want = sw ? sw.nextSibling : bar.querySelector('.toolbar-button[aria-label]:not(.pgt-help)');
      if (help.parentElement !== bar || (want && help !== want && help.nextSibling !== want)) bar.insertBefore(help, sw ? sw.nextSibling : want);
    }
    const foot = document.querySelector('aside .sidebar-footer');
    if (foot && showCard()) { if (card.nextSibling !== foot) foot.parentElement.insertBefore(card, foot); }
    else if (card.parentElement) card.remove();
  }

  function renderLauncher() {
    const stagesDone = quickStagesDone(), r = resumeAt(), all = done().includes('quick');
    const live = r && r.index > 0, quickLive = live && r.tour === 'quick';
    help.setAttribute('aria-label', ui('guide')); help.title = ui('guide');
    panel.setAttribute('aria-label', ui('guide')); panel.setAttribute('lang', lang());
    help.querySelector('.pgt-dot').hidden = all;
    const pct = stagesDone / STAGES.length * 100;
    const next = stagesDone < STAGES.length ? tr(STAGES[stagesDone]) : '';
    card.setAttribute('lang', lang());
    card.innerHTML = `<div class="top"><button type="button" class="open" aria-haspopup="dialog">${ui('setup')}</button>`
      + `<button type="button" class="x" aria-label="${ui('hide')}" title="${ui('hide')}">×</button></div>`
      + `<div class="bar"><i style="width:${pct}%"></i></div>`
      + `<div class="meta">${stagesDone}/${STAGES.length} ${ui('stages')}${next ? `<br>${ui('nextUp')}: <b>${next}</b>` : ''}</div>`
      + `<button type="button" class="go">${quickLive ? ui('resume') : ui('start')}</button>`;
    card.querySelector('.open').onclick = () => togglePanel(card);
    card.querySelector('.go').onclick = () => (quickLive ? start('quick', r.index) : start('quick', 1));
    card.querySelector('.x').onclick = () => { store.set('cardHidden', true); closePanel(); mount(); help.focus(); };
    panel.innerHTML = `<h3>${ui('setup')}</h3><div class="sub">${all ? ui('allDone') : `${stagesDone}/${STAGES.length} ${ui('stages')}`}</div>`
      + `<div class="bar"><i style="width:${pct}%"></i></div>`
      + '<ul>' + STAGES.map((s, k) => `<li class="${k < stagesDone ? 'ok' : ''}"><b>✓</b><span>${tr(s)}</span></li>`).join('') + '</ul>'
      + `<button type="button" class="primary" data-act="quick">${all ? ui('again') : quickLive ? ui('cont') : ui('begin')}</button>`
      + (live && !quickLive ? `<button type="button" class="t" data-act="resume"><span>${ui('resume')} · ${tr(tourById(r.tour).name)}</span><em>${r.index + 1}/${tourById(r.tour).steps.length}</em></button>` : '')
      + `<div class="more">${ui('learn')}</div>`
      + TOURS.slice(1).map(t => `<button type="button" class="t" data-tour="${t.id}"><span>${tr(t.name)}</span><em class="${done().includes(t.id) ? 'seen' : ''}">${done().includes(t.id) ? ui('seen') : `${t.steps.length} ${ui('steps')}`}</em></button>`).join('')
      + (store.get('cardHidden', false) && !all ? `<button type="button" class="t restore" data-act="card"><span>${ui('showCard')}</span></button>` : '');
    panel.querySelector('[data-act=quick]').onclick = () => (quickLive && !all ? start('quick', r.index) : start('quick', all ? 1 : 0));
    panel.querySelector('[data-act=resume]')?.addEventListener('click', () => start(r.tour, r.index));
    panel.querySelector('[data-act=card]')?.addEventListener('click', () => { store.set('cardHidden', false); closePanel(); renderLauncher(); mount(); });
    $$('[data-tour]', panel).forEach(b => b.onclick = () => start(b.dataset.tour));
    mount();
  }
  // The path at dispatch time: a click that re-renders the card would otherwise look like a click outside it.
  document.addEventListener('click', e => { const path = e.composedPath(); if (![panel, help, card].some(x => path.includes(x))) closePanel(); });
  document.addEventListener('keydown', e => { if (e.key === 'Escape' && panel.classList.contains('open')) { const back = anchor === card ? card.querySelector('.open') : help; closePanel(); back?.focus(); } });
  addEventListener('resize', placePanel);
  // The toolbar's VI/EN switch and Settings → Language change the app without a reload: follow it, mid-tour too.
  let shownLang = lang();
  setInterval(() => {
    mount(); placePanel();
    if (lang() === shownLang) return;
    shownLang = lang(); renderLauncher();
    // A running tour redraws its current step in the new language.
    if (drv && drv.isActive()) { const at = drv.getActiveIndex() ?? 0, id = tour.id; finished = true; drv.destroy(); setTimeout(() => start(id, at), 50); }
  }, 500);
  renderLauncher();

  // First visit: a welcome card that asks, never a tour that starts by itself.
  if (!store.get('seen', false) && !resumeAt()) setTimeout(() => start('quick', 0), 900);
})();
