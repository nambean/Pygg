/*
 * Pygg setup tours: a 10-step first-setup tour, then short optional tours per screen. Each step dims the screen,
 * spotlights one setting and explains it; Next moves on and opens the right screen or sheet by itself.
 * Built on Driver.js (MIT, https://driverjs.com), loaded before this file by docker/Dockerfile.ui. The app's own
 * source is unchanged: this runs only in the browser preview image.
 */
(() => {
  'use strict';
  if (window.__pyggTour) return;
  window.__pyggTour = true;
  // The steps find settings by their Vietnamese labels: start in Vietnamese unless a language was chosen.
  const LANG = 'smarttaskrouter.language';
  if (!localStorage.getItem(LANG)) { localStorage.setItem(LANG, 'vi'); location.reload(); return; }

  // ---------- finding things ----------
  const $$ = (sel, root = document) => [...root.querySelectorAll(sel)];
  const shown = e => !!e && e.getClientRects().length > 0;
  const main = () => document.querySelector('main .content') || document.querySelector('main') || document.body;
  const sheet = () => $$('.sheet').find(shown) || null;
  const sheetTitle = () => { const s = sheet(); return s ? (s.innerText || '').trim().split('\n')[0].trim() : ''; };
  const title = () => (document.querySelector('header.toolbar .toolbar-title h1') || {}).textContent?.trim() || '';
  const sleep = ms => new Promise(r => setTimeout(r, ms));
  async function until(test, ms = 2500) {
    const end = Date.now() + ms;
    while (Date.now() < end) { try { if (test()) return true; } catch (e) { /* not yet */ } await sleep(60); }
    return false;
  }

  /** The deepest visible element whose text is, or starts with, `text`. */
  function find(text, scope) {
    let best = null;
    for (const e of (scope || main()).querySelectorAll('*')) {
      if (!shown(e)) continue;
      const t = (e.textContent || '').trim();
      if (t === text || t.startsWith(text)) { if (!best || best.contains(e)) best = e; }
    }
    return best;
  }
  /** A labelled setting: from its label up to the element that also holds its control. */
  function row(text, scope) {
    let e = find(text, scope);
    while (e && e.parentElement && !e.querySelector('input,select,textarea,button,[role=switch]')) e = e.parentElement;
    return e;
  }
  const btn = (text, scope) => $$('button', scope || main()).find(b => shown(b) && b.textContent.trim().startsWith(text)) || null;
  const exactBtn = (text, scope) => $$('button', scope || main()).find(b => shown(b) && b.textContent.trim() === text) || null;
  const group = text => { const h = find(text); return h ? h.closest('section.group') || h.closest('section') || h.parentElement : null; };
  const lastButtonOf = text => { let c = find(text); while (c && !c.querySelector('button')) c = c.parentElement; return c && $$('button', c).pop(); };
  const gatewayCard = () => document.querySelector('main section.gateway-card');
  const firstKeyRow = () => main().querySelector('table tbody tr');
  const firstModelRow = () => main().querySelector('table.models-table tbody tr');

  // ---------- getting to a step's screen ----------
  const SHEETS = {
    gateway: () => sheetTitle().startsWith('Cài đặt gateway'),
    limits: () => sheetTitle().startsWith('Giới hạn cho'),
    pick: () => sheetTitle().startsWith('Kết nối nhà cung cấp'),
    form: () => !!sheet() && !!btn('Change Provider', sheet()),
  };
  async function closeSheet() {
    const s = sheet(); if (!s) return;
    const cancel = exactBtn('Hủy', s) || exactBtn('Cancel', s);
    if (cancel) cancel.click(); else document.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));
    await until(() => !sheet());
  }
  async function openScreen(name) {
    if (title() === name) return;
    const b = $$('aside button').find(x => x.textContent.trim().startsWith(name));
    if (b) { b.click(); await until(() => title() === name); await sleep(150); }
  }
  async function openSheet(kind) {
    if (SHEETS[kind]()) return;
    if (kind === 'gateway') btn('Cấu hình', gatewayCard())?.click();
    if (kind === 'limits') btn('Giới hạn', firstKeyRow())?.click();
    if (kind === 'pick' || kind === 'form') {
      if (!sheet()) { btn('Kết nối nhà cung cấp', document.body)?.click(); await until(() => SHEETS.pick() || SHEETS.form()); }
      if (kind === 'pick' && SHEETS.form()) btn('Change Provider', sheet())?.click();
      if (kind === 'form' && !SHEETS.form()) $$('button', sheet()).find(b => b.textContent.includes('Groq'))?.click();
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
  function pick(st) { try { const e = st.el(); return (Array.isArray(e) ? e.find(shown) : e) || undefined; } catch (e) { return undefined; } }


  /** The smallest element holding both `a` and `b`: a label and its value, framed together and nothing more. */
  function around(a, b) { if (!a || !b) return a || b; let p = a; while (p && !p.contains(b)) p = p.parentElement; return p; }
  const keyRow = text => lastButtonOf(text);

  // ---------- the tours ----------
  // One short tour for the first setup (only what Pygg needs to work), then short optional tours per screen.
  const S = (screen, title, desc, el, extra = {}) => ({screen, title, desc, el, ...extra});
  const TOURS = [
    {id: 'quick', name: 'Cấu hình lần đầu', note: 'Những việc bắt buộc để dùng được Pygg', steps: [
      S('Home', 'Địa chỉ gateway', 'Pygg là một gateway trên máy bạn. Mọi công cụ code sẽ gọi vào http://127.0.0.1:8741 với model "auto". Tour này có 10 bước: nối nhà cung cấp → tạo key → trỏ công cụ vào → xem chi phí.', () => { const u = find('http://', gatewayCard()); return u && u.parentElement; }),
      S('Nhà cung cấp', 'Nối một nhà cung cấp', 'Pygg không có model riêng: bạn cần nối ít nhất một tài khoản. Bấm "Tiếp" để mở bảng kết nối.', () => btn('Kết nối nhà cung cấp', document.body)),
      S('Nhà cung cấp', 'Chọn nhà cung cấp', 'Người mới nên chọn bên có gói miễn phí: Groq, Google Gemini, OpenRouter — hoặc Ollama nếu chạy model trên máy (lọc "Cục bộ").', () => $$('button', sheet()).find(b => b.textContent.includes('Groq')), {sheet: 'pick'}),
      S('Nhà cung cấp', 'Dán API key', 'Dán key lấy từ trang của nhà cung cấp rồi bấm "Kết nối nhà cung cấp". Key lưu trong Keychain / Credential Manager, không bao giờ hiện lại. Pygg chỉ đọc danh sách model — không tốn tiền.', () => row('API key', sheet()), {sheet: 'form'}),
      S('Nhà cung cấp', 'Kiểm tra auto có model dùng được', 'Sau khi nối, xem dòng này: "models eligible" phải lớn hơn 0, nếu không công cụ gọi "auto" sẽ bị từ chối. Bấm vào số "missing routing facts" để bổ sung giá / tier cho model.', () => find('Auto, for a coding tool')?.parentElement),
      S('Endpoint', 'Tạo API key cho công cụ', 'Mỗi công cụ một key của Pygg (không phải key nhà cung cấp) để biết ai tiêu bao nhiêu. Token str-… chỉ hiện một lần — chép lại ngay.', () => btn('Create API Key', document.body)),
      S('Endpoint', 'Đặt hạn mức cho key', '"Giới hạn…" đặt hạn mức tháng / ngày / phiên cho key. Nên đặt ngay từ đầu; chạm hạn mức thì request bị từ chối, request đang chạy không bị cắt.', () => btn('Giới hạn', firstKeyRow())),
      S('Endpoint', 'Ba thứ công cụ cần', 'URL (OpenAI-compatible cho Codex/OpenCode/SDK, Anthropic cho Claude Code), API key vừa tạo, và model "auto".', () => group('Base URLs')),
      S('CLI Tools', 'Trỏ công cụ code vào Pygg', '"Cài đặt" làm hết cho Claude Code / OpenCode / Codex: tạo key, sao lưu cấu hình, rồi chạy `pygg run claude` (hoặc opencode, codex). Trên macOS chạy Docker: dùng "Set up by hand" với token trong docker/token.', () => lastButtonOf('Claude Code: models through Pygg')),
      S('Mức dùng', 'Xem chi phí', 'Mọi request được ghi lại cùng chi phí: thực tế, ước tính, hoặc "chưa rõ" (không bao giờ ghi $0). Xong! Các tour ngắn cho từng màn hình nằm trong nút "Hướng dẫn cấu hình".', () => { const h = $$('h2', main()).find(x => x.textContent.trim() === 'Chi phí API'); return h && (h.closest('section.group') || h.parentElement); }),
    ]},
    {id: 'gateway', name: 'Gateway', note: 'Cổng, khởi động', steps: [
      S('Home', 'Cài đặt gateway', 'Mở phần cài đặt của gateway. Bấm "Tiếp" để xem bên trong.', () => btn('Cấu hình', gatewayCard())),
      S('Home', 'Cổng', 'Mặc định 8741 (1024–65535). Chỉ đổi khi cổng đã bị chương trình khác dùng; đổi thì gateway tự khởi động lại và công cụ phải dùng URL mới.', () => row('Cổng', sheet()), {sheet: 'gateway'}),
      S('Home', 'Bật cùng ứng dụng', 'Nên để bật: gateway chạy ngay khi mở Pygg.', () => row('Bật cùng ứng dụng', sheet()), {sheet: 'gateway'}),
      S('Home', 'Địa chỉ', 'Luôn 127.0.0.1, không đổi được: chỉ máy này dùng được gateway.', () => around(find('Địa chỉ', sheet()), find('Gateway chỉ lắng nghe', sheet())), {sheet: 'gateway'}),
    ]},
    {id: 'providers', name: 'Model, giá và auto', note: 'Vì sao auto chọn / bỏ qua một model', steps: [
      S('Nhà cung cấp', 'Một kết nối', 'Dải chấm là sức khoẻ 24 giờ, số là điểm 0–100, "n for auto" là số model auto dùng được. Công tắc bên phải tắt cả kết nối.', () => { const r = main().querySelector('li.select-row'); return r && (r.querySelector('button.select-row-main') || r); }),
      S('Nhà cung cấp', 'Kiểm tra & làm mới model', 'Đọc lại model, tự điền context / tools / giá từ models.dev (không đè giá trị bạn tự đặt).', () => btn('Kiểm tra & làm mới model')),
      S('Nhà cung cấp', 'Tính phí', 'Miễn phí / Cục bộ / Gói thuê bao / Trả phí / Chưa rõ. "Miễn phí" ghi mọi request $0 — chỉ chọn khi đúng là miễn phí.', () => row('Tính phí')),
      S('Nhà cung cấp', 'Vì sao auto bỏ qua', 'Lý do dưới mỗi model: chưa có tier, chưa có giá, tier thấp hơn mức tối thiểu (T2)…', () => find('Auto skips')),
      S('Nhà cung cấp', 'Giá và dữ kiện', 'More → "Set/Edit Price…" (USD / 1 triệu token) và "Auto Routing Facts…" (tier, context, tools). Auto chỉ chọn model có tier ≥ T2, hỗ trợ tool, biết context, có giá (hoặc miễn phí/cục bộ/thuê bao).', () => btn('More', firstModelRow())),
    ]},
    {id: 'limits', name: 'Hạn mức của một key', note: 'Tháng, ngày, phiên, nghiêm ngặt…', steps: [
      S('Endpoint', 'Hạn mức hằng tháng', 'Đặt lại ngày 1 (UTC); chạm thì 429. Trống = không giới hạn. Gõ "1,000" được hiểu là $1.', () => row('Hạn mức hằng tháng', sheet()), {sheet: 'limits'}),
      S('Endpoint', 'Hạn mức phiên (và ngày)', 'Session limit giới hạn một lần `pygg run`; ngay dưới là hạn mức ngày, đặt lại 00:00 UTC. Chạm thì 402. Request đang chạy không bị cắt.', () => row('Session limit', sheet()), {sheet: 'limits'}),
      S('Endpoint', 'Giới hạn nghiêm ngặt', 'Bật: giữ chỗ chi phí tối đa trước khi gửi nên không bao giờ vượt; model thiếu giá / output tối đa sẽ không được dùng.', () => row('Giới hạn nghiêm ngặt', sheet()), {sheet: 'limits'}),
      S('Endpoint', 'Giá chưa rõ', 'Chặn (mặc định): key có hạn mức sẽ bỏ qua model chưa có giá.', () => row('Giá chưa rõ', sheet()), {sheet: 'limits'}),
      S('Endpoint', 'Sau lỗi máy chủ', 'Mặc định "Dừng": lỗi 5xx/timeout thì trả lỗi luôn, vì nhà cung cấp có thể đã tính tiền. Thử lại model khác = có thể trả tiền hai lần.', () => row('Sau lỗi máy chủ', sheet()), {sheet: 'limits'}),
      S('Endpoint', 'Tier tối thiểu cho auto', 'T2 (mặc định) hợp agent viết code; thấp hơn rẻ hơn nhưng yếu hơn.', () => row('Tier tối thiểu cho auto', sheet()), {sheet: 'limits'}),
      S('Endpoint', 'Kết nối được dùng', 'Giới hạn key vào một số kết nối. "Chỉ kết nối đã chọn" mà không tick gì thì mọi request bị từ chối.', () => row('Kết nối', sheet()), {sheet: 'limits'}),
    ]},
    {id: 'policies', name: 'Mặc định cho key mới', note: 'Chính sách', steps: [
      S('Chính sách', 'Tier tối thiểu', 'Áp cho key tạo SAU khi lưu; key đã có sửa ở Endpoint → Giới hạn…', () => row('Minimum model tier')),
      S('Chính sách', 'Hạn mức tháng mặc định', 'Đặt để không key mới nào chạy không giới hạn.', () => row('Hạn mức hằng tháng')),
      S('Chính sách', 'Limit mode', 'Ước tính: đếm khi request xong, có thể vượt chút. Strict: giữ chỗ trước, không vượt.', () => row('Limit mode')),
      S('Chính sách', 'Lưu', 'Chỉ key tạo sau đó mới nhận các giá trị này.', () => btn('Save Defaults')),
    ]},
    {id: 'combos', name: 'Combo và auto', note: 'Tự đặt thứ tự model; mô phỏng auto', steps: [
      S('Combo', 'Combo (tuỳ chọn)', 'Dùng auto thì không cần combo. Tạo combo khi muốn tự chọn model và thứ tự dự phòng; công cụ gửi tên combo làm model.', () => btn('Combo mới', document.body), {prep: async () => { const b = exactBtn('Combo'); if (find('Định tuyến tự động') && b) { b.click(); await until(() => !find('Định tuyến tự động')); } }}),
      S('Combo', 'Auto sẽ chọn gì?', 'Mô phỏng auto cho một request mà không gửi gì. Bấm "Tiếp" để mở.', () => btn('How auto chooses')),
      S('Combo', 'Kết quả mô phỏng', 'Tối đa 5 model, tối đa 2 mỗi kết nối, theo thứ tự sẽ thử, kèm lý do. "… model bị loại" cho biết vì sao các model khác bị loại.', () => main().querySelector('table'), {prep: async () => { if (!find('Định tuyến tự động')) { btn('How auto chooses')?.click(); await until(() => !!find('Định tuyến tự động')); } }}),
    ]},
  ];

  // ---------- the tour ----------
  const css = `
  .driver-popover.pygg-tour{background:#1c1c1e;color:#eee;border:1.5px solid #ff3b30;border-radius:12px;max-width:380px;box-shadow:0 12px 40px rgba(0,0,0,.6)}
  .driver-popover.pygg-tour .driver-popover-title{font:700 15px/1.35 -apple-system,system-ui,sans-serif;color:#fff}
  .driver-popover.pygg-tour .driver-popover-title small{display:block;font:600 11px/1.4 -apple-system,system-ui,sans-serif;color:#ff6b61;text-transform:uppercase;letter-spacing:.04em;margin-bottom:3px}
  .driver-popover.pygg-tour .driver-popover-description{font:13.5px/1.55 -apple-system,system-ui,sans-serif;color:#d8d8d8}
  .driver-popover.pygg-tour .driver-popover-description code{background:#333;border-radius:4px;padding:0 4px}
  .driver-popover.pygg-tour .driver-popover-progress-text{color:#999}
  .driver-popover.pygg-tour .driver-popover-footer button{background:#2c2c2e;color:#eee;border:1px solid #48484a;text-shadow:none;border-radius:6px;padding:5px 11px;font:600 12.5px -apple-system,system-ui,sans-serif}
  .driver-popover.pygg-tour .driver-popover-footer button.driver-popover-next-btn{background:#ff3b30;border-color:#ff3b30;color:#fff}
  .driver-popover.pygg-tour .driver-popover-close-btn{color:#aaa}
  .driver-popover.pygg-tour .driver-popover-arrow-side-left{border-left-color:#1c1c1e}
  .driver-popover.pygg-tour .driver-popover-arrow-side-right{border-right-color:#1c1c1e}
  .driver-popover.pygg-tour .driver-popover-arrow-side-top{border-top-color:#1c1c1e}
  .driver-popover.pygg-tour .driver-popover-arrow-side-bottom{border-bottom-color:#1c1c1e}
  .driver-active-element{outline:3px solid #ff3b30!important;outline-offset:4px;border-radius:6px}
  #pgt-launch{position:fixed;right:16px;bottom:16px;z-index:2147483000;font:13px -apple-system,system-ui,sans-serif}
  #pgt-launch > button{background:#ff3b30;color:#fff;border:0;border-radius:18px;padding:9px 15px;font-weight:700;box-shadow:0 4px 16px rgba(0,0,0,.5);cursor:pointer}
  #pgt-menu{position:absolute;right:0;bottom:44px;width:300px;background:#1c1c1e;color:#eee;border:1.5px solid #ff3b30;border-radius:12px;padding:6px;box-shadow:0 10px 34px rgba(0,0,0,.6);display:none}
  #pgt-menu.open{display:block}
  #pgt-menu div{padding:6px 10px 4px;color:#999;font-size:11px;text-transform:uppercase;letter-spacing:.04em}
  #pgt-menu button{display:block;width:100%;text-align:left;background:none;border:0;color:#eee;padding:7px 10px;border-radius:7px;font:13px -apple-system,system-ui,sans-serif;cursor:pointer}
  #pgt-menu button:hover{background:#3a1f1d}
  #pgt-menu button span{color:#999;font-size:11.5px;margin-left:4px}
  #pgt-menu button em{display:block;color:#999;font-size:11.5px;font-style:normal}
  #pgt-menu button.main{background:#3a1f1d;margin-bottom:4px}
  #pgt-menu button.main span{color:#ff9f97}
  `;
  const style = document.createElement('style'); style.textContent = css; document.head.appendChild(style);

  const esc = s => s.replace(/[&<>]/g, c => ({'&': '&amp;', '<': '&lt;', '>': '&gt;'}[c])).replace(/`([^`]+)`/g, '<code>$1</code>');
  let drv = null, moving = false, tour = null;

  async function go(i) {
    if (moving || !drv) return;
    moving = true;
    try { await prepare(tour.steps[i]); drv.moveTo(i); } finally { moving = false; }
  }
  async function start(id = 'quick') {
    if (!window.driver?.js?.driver) { alert('Không tải được thư viện tour (Driver.js).'); return; }
    menu.classList.remove('open');
    if (drv && drv.isActive()) drv.destroy();
    tour = TOURS.find(t => t.id === id) || TOURS[0];
    const steps = tour.steps;
    drv = window.driver.js.driver({
      steps: steps.map(st => ({
        element: () => pick(st),
        popover: {title: `<small>${esc(tour.name)}</small>${esc(st.title)}`, description: esc(st.desc), side: st.side || 'bottom', align: 'start'},
      })),
      showProgress: true, progressText: 'Bước {{current}}/{{total}}',
      nextBtnText: 'Tiếp →', prevBtnText: '← Lùi', doneBtnText: 'Xong ✓',
      popoverClass: 'pygg-tour', overlayOpacity: 0.62, stagePadding: 6, stageRadius: 8, smoothScroll: true,
      disableActiveInteraction: true, allowKeyboardControl: true,
      onNextClick: () => { const i = drv.getActiveIndex(); if (i >= steps.length - 1) drv.destroy(); else go(i + 1); },
      onPrevClick: () => { const i = drv.getActiveIndex(); if (i > 0) go(i - 1); },
      onDestroyed: () => { launch.style.display = ''; closeSheet(); },
    });
    launch.style.display = 'none';
    await prepare(steps[0]);
    drv.drive(0);
  }
  window.pyggTour = {start, tours: TOURS.map(t => `${t.id}:${t.steps.length}`)};

  const launch = document.createElement('div'); launch.id = 'pgt-launch';
  launch.innerHTML = '<div id="pgt-menu"></div><button type="button">? Hướng dẫn cấu hình</button>';
  document.body.appendChild(launch);
  const menu = launch.querySelector('#pgt-menu');
  TOURS.forEach((t, k) => {
    if (k === 1) { const d = document.createElement('div'); d.textContent = 'Tìm hiểu thêm'; menu.appendChild(d); }
    const b = document.createElement('button'); b.type = 'button'; if (k === 0) b.className = 'main';
    b.innerHTML = `${esc(t.name)} <span>${t.steps.length} bước</span><em>${esc(t.note)}</em>`;
    b.onclick = () => start(t.id);
    menu.appendChild(b);
  });
  launch.querySelector(':scope > button').onclick = () => menu.classList.toggle('open');
  document.addEventListener('click', e => { if (!launch.contains(e.target)) menu.classList.remove('open'); });

  // First visit: the first-setup tour, once.
  if (!localStorage.getItem('pygg.tour.seen')) { localStorage.setItem('pygg.tour.seen', '1'); setTimeout(() => start('quick'), 1200); }
})();
