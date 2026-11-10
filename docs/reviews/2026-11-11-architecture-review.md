# Phản biện kiến trúc: SmartTaskRouter, 2026-11-11

> Vai trò: Principal Software Architect. Mục tiêu của bản này là **phản biện, không phải đồng tình**.
>
> Phạm vi là code trên `main` (gateway đã chạy) và bộ tài liệu mới: [UNIVERSAL_SHARED_MEMORY](../UNIVERSAL_SHARED_MEMORY.md), [ACCOUNT_API_ROTATION](../ACCOUNT_API_ROTATION.md), [CLI_IDE_NPM_PROVIDER_TIERS](../CLI_IDE_NPM_PROVIDER_TIERS.md), [OMNIROUTE_FEATURE_RESEARCH](../OMNIROUTE_FEATURE_RESEARCH.md), cùng ADR-017 đến ADR-024.
>
> Mức độ:
> - **Critical:** làm hỏng mục tiêu cốt lõi (giảm chi phí), hoặc có rủi ro vi phạm điều khoản hay mất tiền thật.
> - **High:** sai so với quyết định đã ghi, hoặc gây chi phí hay lỗi đáng kể.
> - **Medium:** cần sửa trước khi phát hành.
> - **Low:** nên sửa.
>
> Các số dòng đúng tại commit chứa file này.

## 0. Tóm tắt

1. **Giả định "giảm chi phí" chưa được chứng minh. Với code hiện tại, có đường đi còn làm chi phí tăng.**
   - Claude Code gọi tới Claude qua gateway thì mất prompt cache. Theo mô hình ở §7, một phiên dài đắt hơn khoảng **3,7 lần** so với gọi thẳng.
   - Nén context theo kiểu viết lại lịch sử sẽ phá cache, và có thể làm chi phí **tăng khoảng 3,3 lần** thay vì giảm.
2. **Feature creep là có thật.** Bốn tài liệu đánh dấu khoảng 20 hạng mục là P0, nên P0 không còn nghĩa là "làm trước".
3. **Universal Shared Memory không thể chạy qua gateway với các client tự giữ lịch sử** (Claude Code, OpenCode). Nó chỉ khả thi khi app sở hữu client chat. Hai hướng sản phẩm đang kéo kiến trúc về hai phía.
4. **Gateway hiện tại có các lỗi Critical/High** về fallback lỗi 401/403/429 (có rủi ro vi phạm điều khoản provider), về ngân sách không nguyên tử, và về việc chưa có session affinity.
5. **Không dự án nguồn mở nào trong tài liệu viết bằng Rust.** Thứ đáng tái dùng là *dữ liệu* (bảng giá, danh sách free tier) và *ý tưởng*. Code thì không dùng lại trực tiếp được.

## 1. Product Fit: có feature creep không?

### PF-1 · Critical · P0 bị lạm phát, không còn thứ tự ưu tiên

**Dẫn chứng:** các mục sau đều được đánh dấu P0.
- OR-01 đến OR-07 ([OMNIROUTE:34-40](../OMNIROUTE_FEATURE_RESEARCH.md#L34)), OR-P0-1 đến 4 ([OMNIROUTE:230-233](../OMNIROUTE_FEATURE_RESEARCH.md#L230)).
- M1 đến M3 ([UNIVERSAL_SHARED_MEMORY:213-215](../UNIVERSAL_SHARED_MEMORY.md#L213)).
- G1 đến G3 ([CLI_IDE_NPM:210-212](../CLI_IDE_NPM_PROVIDER_TIERS.md#L210)).
- Account registry, entitlement, quota group, `fill_first`, `round_robin` và allowlist ([ACCOUNT_API_ROTATION:183](../ACCOUNT_API_ROTATION.md#L183)).

**Vấn đề:**
- Gộp lại, P0 gồm gateway, catalog, alias `auto`, xoay vòng tài khoản, nén, memory, affinity và ba tích hợp CLI. Đó là cả sản phẩm, không phải một MVP.
- Mỗi tài liệu dùng một hệ đánh số riêng (Phase, G, M, OR-P, P0–P2), nên không có thứ tự toàn cục nào.

**Sửa:**
- Một file `docs/ROADMAP.md` duy nhất làm nguồn sự thật về thứ tự.
- P0 tối đa 5 hạng mục, đề xuất ở §6.
- Bốn tài liệu chuyên đề chuyển thành RFC trong `docs/rfc/` và chỉ dẫn link về ROADMAP. ADR chỉ ghi những gì **đã quyết**.

### PF-2 · High · Người dùng mục tiêu mâu thuẫn với giá trị cốt lõi

**Dẫn chứng:**
- Định vị "everyday AI users **and** developers" ([PLAN:9](../SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md#L9)).
- Consumer chat là "new primary flow" ([UNIVERSAL_SHARED_MEMORY:170](../UNIVERSAL_SHARED_MEMORY.md#L170)).

**Vấn đề:**
- Người dùng phổ thông phần lớn trả phí thuê bao cố định (ChatGPT/Claude/Gemini), không trả theo token. Tiết kiệm token gần như không có giá trị với họ.
- Nhóm thực sự trả tiền theo token với khối lượng lớn là **dev chạy agent bằng API key**. Theo §7, một phiên agent dài tốn vài đô, trong khi một tin nhắn chat chỉ tốn vài phần trăm cent.

**Sửa:**
- Ghi một ADR "Wedge" chọn người dùng đầu tiên: dev dùng Claude Code hoặc OpenCode với nhiều key.
- Auto Chat cho người dùng phổ thông lùi sang giai đoạn sau, và chỉ làm nếu đã có số đo từ wedge.

### PF-3 · High · Ba định vị sản phẩm cùng tồn tại trong tài liệu "hiện hành"

**Dẫn chứng:**
- ADR-003 "Orchestrator first" ([DECISIONS:17-19](../DECISIONS.md#L17)) và Plan §0.1 cùng hướng ([PLAN:53](../SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md#L53)).
- ADR-016 "Gateway first".
- North star "AI Cost Optimizer cho mọi người" ([PLAN:9](../SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md#L9)).
- README vẫn liệt kê "Orchestrator first" là nguyên tắc ([README:11](../../README.md#L11)).

**Sửa:**
- Sửa ADR-003 thành "Superseded in part by ADR-016", chỉ giữ ràng buộc "không đổi model trong một phiên agent".
- Viết lại Plan §0.1 theo đúng wedge đã chọn.

## 2. Routing Engine: chọn model có thật sự giảm tổng chi phí?

### RE-1 · High · Chưa có routing engine; "smart" hiện chỉ là combo tĩnh

**Dẫn chứng:** [store.rs:428-429](../../apps/desktop/src-tauri/src/gateway/store.rs#L428). `resolve` chỉ tra combo theo tên, hoặc theo `<connection>/<model>`.

**Vấn đề:** người dùng tự xếp thứ tự model. Không có ước tính chất lượng, cũng không có ước tính chi phí. Gọi đây là "Cost Optimizer" là quá lời so với code.

**Sửa:**
- Bản 1 là alias `auto/*` theo luật tường minh: lọc cứng (khả năng, ngữ cảnh, quyền riêng tư, ngân sách), rồi chọn hạng theo Save/Balanced/Quality.
- Header `x-smarttaskrouter-reason` giải thích lựa chọn.
- Không dùng classifier bằng LLM ở bản 1.

### RE-2 · High · Chi phí của classifier không có trần

**Dẫn chứng:**
- "detect intent… at minimal classifier cost" ([OMNIROUTE:86](../OMNIROUTE_FEATURE_RESEARCH.md#L86)).
- "Detect task intent" ở mỗi tin nhắn ([UNIVERSAL_SHARED_MEMORY:87](../UNIVERSAL_SHARED_MEMORY.md#L87)).

**Vấn đề:**
- Nếu classifier đọc toàn bộ context của một phiên agent (khoảng 70k token), nó tốn **bằng cả lượt trả lời của model rẻ** (§7, dòng 3). Khoản "tiết kiệm" khi đó bị nhân đôi chi phí.
- Tài liệu không đặt giới hạn nào cho đầu vào của classifier.

**Sửa:**
- Đặt bất biến: đầu vào classifier ≤ 2k token, gồm tin nhắn cuối và metadata, không bao giờ đọc lịch sử.
- Chỉ phân loại khi **bắt đầu hội thoại** hoặc **sau một thất bại**, không làm ở mọi lượt.
- Ghi chi phí classifier thành một dòng riêng trong sổ.

### RE-3 · Critical · Chưa có tín hiệu chất lượng nên không chứng minh được "giữ chất lượng"

**Dẫn chứng:**
- Tài liệu tự thừa nhận: "Gateway-only routing cannot infer complete intent… must not claim task-level cost optimization" ([CLI_IDE_NPM:15](../CLI_IDE_NPM_PROVIDER_TIERS.md#L15)).
- Rủi ro #4 trong [UNIVERSAL_SHARED_MEMORY](../UNIVERSAL_SHARED_MEMORY.md#L228).

**Vấn đề:**
- Với code còn có test để kiểm tra, với chat thường thì không có gì đo được chất lượng.
- Chi phí lớn nhất khi model rẻ thất bại là **thời gian của người dùng**: đọc câu trả lời kém, hoàn tác thay đổi sai. Công thức `C_effective` ([OMNIROUTE:104](../OMNIROUTE_FEATURE_RESEARCH.md#L104)) không có hạng mục này.

**Sửa:**
- Thêm `C_human_failure` vào công thức.
- Lấy tín hiệu chất lượng từ hành vi: retry, nút "dùng model tốt hơn", client hủy giữa chừng. Ghi các tín hiệu này vào sổ.
- Benchmark A–E chỉ được công bố khi có bộ đánh giá cố định.

## 3. Universal Shared Memory: có giữ được liên tục và không lãng phí token?

### USM-1 · Critical · Context package dựng lại mỗi lượt sẽ phá prompt cache

**Dẫn chứng:**
- Bước 3 dựng một "minimum viable context package" cho mỗi tin nhắn mới ([UNIVERSAL_SHARED_MEMORY:88](../UNIVERSAL_SHARED_MEMORY.md#L88)).
- Nén và dedup lịch sử ([OMNIROUTE:110-118](../OMNIROUTE_FEATURE_RESEARCH.md#L110)).

**Vấn đề:**
- Prompt cache của provider chỉ trúng khi **tiền tố y hệt** lần gửi trước.
- Mỗi lượt chọn lại "recent turns", "relevant facts" và summary sẽ làm tiền tố đổi, nên cache trượt ở **mọi lượt**.
- Với giá cache đọc chỉ bằng khoảng 10% giá input, việc cắt 30% token mà mất cache khiến chi phí tăng khoảng 3,3 lần (§7, dòng 6).

**Sửa:** thêm bất biến **prefix-stable, append-only** vào cả hai tài liệu.
- Context package chỉ được dựng **một lần tại điểm chuyển model**, rồi chỉ nối thêm.
- Nén chỉ áp dụng cho **nội dung mới**, ví dụ tool output trước khi nó vào lịch sử lần đầu. Không bao giờ sửa phần đã gửi.
- Đo `cache_read_tokens` trước và sau, coi đó là cổng chấp nhận.

### USM-2 · Critical · Memory không thể chạy qua gateway cho client tự giữ lịch sử

**Dẫn chứng:**
- "Conversation Manager owns one portable thread ID" ([UNIVERSAL_SHARED_MEMORY:57](../UNIVERSAL_SHARED_MEMORY.md#L57)).
- Kiến trúc ở §3 đặt "Chat UI / Agent CLI / SDK" chung một luồng.

**Vấn đề:**
- Claude Code và OpenCode tự giữ lịch sử, gửi lại toàn bộ ở mỗi request, và tự compact.
- Gateway chèn hay thay context là âm thầm viết lại hội thoại của agent. Việc này phá tool ID, phá cache và phá giả định của client, trái ràng buộc trong [DECISIONS:19](../DECISIONS.md#L19).
- Gateway cũng không nhận được conversation ID từ OpenAI Chat API.

**Sửa:** tách hai chế độ ngay trong ADR-017.
- **(a) App sở hữu hội thoại** (Auto Chat): có memory đầy đủ.
- **(b) Gateway cho client bên ngoài:** chỉ có affinity, ledger và chuyển model theo phiên. **Không chèn context.**

### USM-3 · High · Tài liệu không định nghĩa cách nhận diện hội thoại qua gateway

**Dẫn chứng:** khoá affinity có dạng `client_id + conversation_id + …` ([ACCOUNT_API_ROTATION:28](../ACCOUNT_API_ROTATION.md#L28)), nhưng không nói `conversation_id` lấy từ đâu.

**Sửa:** đặc tả thứ tự ưu tiên:
1. ID tường minh mà client gửi. Cần kiểm chứng xem Claude Code có gửi trong `metadata` không.
2. Header `x-smarttaskrouter-session` do người dùng cấu hình.
3. Hash của (client, system prompt, tin nhắn user đầu tiên), coi là *best effort*.

Hash dễ vỡ: hai phiên có thể có cùng tin nhắn mở đầu, và client compact lại lịch sử thì hash đổi. UI phải ghi rõ khi hội thoại được nhận diện bằng hash.

### USM-4 · Medium · Trích xuất memory chưa có trần chi phí

**Dẫn chứng:** dòng "Update summary/facts **selectively**" ([UNIVERSAL_SHARED_MEMORY:92](../UNIVERSAL_SHARED_MEMORY.md#L92)) không định nghĩa "selectively".

**Sửa:**
- Gom thành lô, ví dụ mỗi N lượt hoặc khi đóng hội thoại.
- Chỉ đọc phần **mới** từ lần trích xuất trước, chạy bằng hạng model rẻ nhất.
- Ghi chi phí thành dòng `memory_extraction` riêng trong sổ.

## 4. Gateway & Account Rotation (code hiện tại)

### GW-1 · Critical · Combo có thể xoay key cùng provider để né quota hoặc chặn

**Dẫn chứng:** [mod.rs:238-239](../../apps/desktop/src-tauri/src/gateway/mod.rs#L238). Gateway fallback trên 401, 403 và 429 sang **bất kỳ** model kế tiếp nào trong combo, kể cả một kết nối khác của cùng provider.

**Vấn đề:** điều này trái [ACCOUNT_API_ROTATION:66-69](../ACCOUNT_API_ROTATION.md#L66). Người dùng có thể bị khoá tài khoản vì xoay key để vượt rate limit hoặc né lệnh chặn.

**Sửa:**
- Phân loại lỗi trong `forward`:
  - **401:** đánh dấu kết nối `invalid_credentials`, hiện trong Connections, tạm bỏ qua kết nối đó.
  - **403:** dừng toàn bộ các kết nối **cùng provider** trong chuỗi.
  - **429:** đặt cooldown theo `Retry-After`, không nhảy sang kết nối cùng provider, trừ khi người dùng đã đánh dấu hai kết nối là tổ chức độc lập.
- Thêm cột `organization` cho `providers`.
- Trình sửa combo cảnh báo khi có hai kết nối cùng provider.

### GW-2 · Critical · Đường Claude mất prompt cache qua gateway

**Dẫn chứng:**
- [anthropic.rs:93](../../apps/desktop/src-tauri/src/gateway/anthropic.rs#L93) gộp system thành chuỗi.
- [anthropic.rs:131](../../apps/desktop/src-tauri/src/gateway/anthropic.rs#L131) chỉ giữ `text`, bỏ `cache_control`.
- [mod.rs:298](../../apps/desktop/src-tauri/src/gateway/mod.rs#L298) luôn gọi `/chat/completions`.
- Không có preset Anthropic gốc ([presets.rs](../../apps/desktop/src-tauri/src/gateway/presets.rs)).

**Vấn đề:**
- Claude Code đi Anthropic → OpenAI → OpenRouter → Anthropic, bị dịch hai lần và mất cache. Một phiên dài đắt hơn khoảng 3,7 lần (§7, dòng 2).
- Thinking cũng bị bỏ.
- Đây là chỗ sản phẩm *làm tăng* chi phí. Kết luận này dựa trên quy tắc cache của Anthropic và OpenRouter, và cần một phép đo `cache_read` thật để xác nhận.

**Sửa:**
- Thêm upstream kind `anthropic` (Anthropic API, endpoint Anthropic của OpenRouter) và **chuyển tiếp nguyên định dạng** `/v1/messages`.
- Khi buộc phải dịch, giữ `cache_control` trên content parts cho các upstream hỗ trợ.
- Thêm test đo `cache_read_input_tokens` ở lượt thứ hai.

### GW-3 · High · Hạn mức tháng không nguyên tử và bỏ qua chi phí Unknown

**Dẫn chứng:**
- Chỉ kiểm tra trước khi gửi ([mod.rs:192](../../apps/desktop/src-tauri/src/gateway/mod.rs#L192)).
- Chỉ cộng chi phí đã biết ([store.rs:278](../../apps/desktop/src-tauri/src/gateway/store.rs#L278)).

**Vấn đề:**
- N request song song đều qua cửa khi chi tiêu còn dưới hạn mức.
- Model chưa có giá thì không bao giờ bị chặn.

**Sửa:**
- Giới hạn số request đồng thời theo client (semaphore, mặc định 4).
- Giữ chỗ bi quan bằng `max_tokens × giá output`, áp dụng khi model đã có giá.
- Chính sách cho client khi gặp giá Unknown: `Block` / `Allow`. Khi client có hạn mức thì mặc định là `Block`, đúng theo [SCREENS §5.5](../design/SCREENS.md).

### GW-4 · High · Không có session affinity, nên đổi model qua lại giữa các lượt

**Dẫn chứng:** combo luôn bắt đầu từ phần tử đầu ([store.rs:428](../../apps/desktop/src-tauri/src/gateway/store.rs#L428)).

**Vấn đề:**
- Sau một lần 429, lượt sau chạy Qwen. Gemini hồi phục thì lượt kế lại quay về Gemini.
- Một phiên agent bị đổi model, trái [DECISIONS:19](../DECISIONS.md#L19), và mất cache ở cả hai phía.

**Sửa:**
- Bảng `session_affinity(client_id, session_key, provider_id, model_id, expires_at)`.
- Ghim vào phần tử đã trả lời thành công; chỉ thử lại phần tử đầu ở phiên mới hoặc khi hết cooldown.
- Cách lấy `session_key` theo USM-3.

### GW-5 · High · Client ngắt kết nối thì sổ không ghi gì

**Dẫn chứng:** [mod.rs:302](../../apps/desktop/src-tauri/src/gateway/mod.rs#L302) và [mod.rs:426](../../apps/desktop/src-tauri/src/gateway/mod.rs#L426) chạy trong future của handler.

**Vấn đề:** client đóng kết nối thì axum hủy future. Không có dòng nào được ghi, kể cả dòng Unknown, trong khi provider có thể vẫn tính tiền.

**Sửa:**
- Chạy cuộc gọi upstream trong `tokio::spawn`; handler chỉ chờ một `oneshot`.
- Task luôn ghi sổ, với outcome `client_disconnected`.

### GW-6 · High · Timeout dẫn tới fallback, có thể bị tính tiền hai lần

**Dẫn chứng:** `read_timeout(300s)` ([mod.rs:64](../../apps/desktop/src-tauri/src/gateway/mod.rs#L64)) cộng với nhánh lỗi mạng chuyển sang model kế.

**Vấn đề:**
- Một request không stream tới model reasoning có thể chạy quá 300 giây trước khi có header.
- Request đầu vẫn chạy xong và bị tính tiền, trong khi model thứ hai cũng bị tính.

**Sửa:**
- Chỉ fallback khi lỗi **kết nối** (request chưa gửi đi), hoặc khi có HTTP 429/5xx rõ ràng.
- Gặp timeout đọc thì trả lỗi cho client, ghi Unknown, không chuyển model.
- Nâng `read_timeout` cho request không stream, hoặc khuyến nghị client dùng stream.

### GW-7 · Medium · Ranh giới fallback đặt ở header thay vì ở token đầu tiên

**Dẫn chứng:** [mod.rs:313](../../apps/desktop/src-tauri/src/gateway/mod.rs#L313).

**Vấn đề:** provider trả 200 rồi mới gửi sự kiện lỗi trong stream thì gateway không thử model kế, dù client chưa nhận được nội dung nào.

**Sửa:** đệm có giới hạn (≤ 2 giây hoặc ≤ 8 KB) cho tới delta nội dung đầu tiên. Nếu trong khoảng đó gặp sự kiện lỗi thì vẫn được phép fallback.

### GW-8 · Medium · Cache chữ ký Gemini chỉ nằm trong RAM

**Dẫn chứng:** [mod.rs:68](../../apps/desktop/src-tauri/src/gateway/mod.rs#L68), [signatures.rs:9-12](../../apps/desktop/src-tauri/src/gateway/signatures.rs#L9).

**Vấn đề:** sau khi khởi động lại, lịch sử tool phải dùng chữ ký giữ chỗ. Google cảnh báo việc này có thể làm giảm chất lượng.

**Sửa:** bảng `tool_call_signatures(call_id, signature, created_at)` có TTL 7 ngày.

### GW-9 · Medium · Gateway sống theo cửa sổ app

**Dẫn chứng:** [lib.rs:63](../../apps/desktop/src-tauri/src/lib.rs#L63).

**Vấn đề:** người dùng đóng app thì Claude Code mất kết nối giữa phiên.

**Sửa:**
- Đóng cửa sổ thì thu vào tray thay vì thoát.
- Khoá một instance gateway duy nhất cho cả app và CLI (file lock trong thư mục dữ liệu).

### GW-10 · Medium · Điều khoản dùng dữ liệu luôn hiện "Unknown"

**Dẫn chứng:** [Connections.tsx:149-150](../../apps/desktop/src/components/gateway/Connections.tsx#L149), [presets.rs](../../apps/desktop/src-tauri/src/gateway/presets.rs) không có trường điều khoản.

**Vấn đề:** gói Gemini không trả phí cho phép dùng nội dung để cải thiện sản phẩm, nhưng người dùng không được báo.

**Sửa:**
- Mỗi preset có `terms_url`, `terms_summary`, `checked_at`.
- Hiện cảnh báo khi Billing = Free với provider có điều khoản này.

### GW-11 · Low · Migration bị race giữa app và CLI ở lần chạy đầu

**Dẫn chứng:** [db.rs:41](../../apps/desktop/src-tauri/src/db.rs#L41) đọc version **ngoài** transaction được mở ở [db.rs:49](../../apps/desktop/src-tauri/src/db.rs#L49).

**Sửa:** dùng `BEGIN IMMEDIATE` và đọc lại `user_version` bên trong transaction.

### GW-12 · Low · `count_tokens` ước tính theo ký tự chia 4

**Dẫn chứng:** [anthropic.rs:454](../../apps/desktop/src-tauri/src/gateway/anthropic.rs#L454).

**Vấn đề:** Claude Code dùng con số này để quyết định khi nào compact. Ước tính lệch thì compact quá sớm (lãng phí) hoặc quá muộn (tràn context).

**Sửa:**
- Với upstream Anthropic gốc thì chuyển tiếp `count_tokens` thật.
- Với các upstream khác thì ghi rõ đây là ước tính trong header.

## 5. Tái dùng nguồn mở: có khả thi với Rust/Tauri?

Dữ kiện lấy từ GitHub API ngày 2026-11-11:

| Dự án | Ngôn ngữ | License | Kết luận |
| --- | --- | --- | --- |
| decolua/9router | JavaScript | MIT | Không nhúng được nếu không có sidecar Node (trái ADR-005). Tái dùng **dữ liệu**: preset, danh sách free tier. Tái dùng **ý tưởng**: fill-first, sticky. Ghi nguồn MIT. |
| cheaper-inference/OmniRoute | TypeScript | MIT | Như trên. Bảng free tier và các quy tắc nén là dữ liệu và thuật toán nhỏ, viết lại bằng Rust được. |
| mem0ai/mem0 | Python | Apache-2.0 | Không nhúng được. Viết lại trích xuất và FTS5 trên SQLite là khả thi; cần kiểm tra `rusqlite` bản bundled có bật FTS5 hay không. |
| anomalyco/opencode | TypeScript | MIT | Học được ranh giới compaction. Dữ liệu giá mà OpenCode dùng (catalog models.dev) có thể làm nguồn giá có provenance; cần kiểm tra license của dữ liệu. |
| letta-ai/letta | Python | Apache-2.0 | Chỉ học khái niệm. |
| tensorzero/tensorzero | **Rust** | Apache-2.0 | Gateway Rust có sẵn, nhưng commit cuối là 2026-06. Đáng đánh giá trước khi tự viết tiếp phần dịch giữa các provider. |
| jeremychone/rust-genai | **Rust** | Apache-2.0 | Thư viện client đa provider. Nó chuẩn hoá định dạng nên **mất trường** khi dùng làm passthrough; chỉ hợp cho các lệnh gọi nội bộ như classifier hay trích xuất. |
| Helicone/ai-gateway | Rust | **GPL-3.0** | Loại: license copyleft, và đã ít hoạt động từ 2025-11. |

### OSS-1 · High · "Viết lại bằng Rust" bị đánh giá thấp chi phí

**Dẫn chứng:** "prefer reimplementing small contracts in Rust" ([OMNIROUTE:18](../OMNIROUTE_FEATURE_RESEARCH.md#L18)).

**Vấn đề:**
- Phần tốn kém không nằm ở "contracts nhỏ" mà ở hàng trăm ngóc ngách riêng của từng provider.
- Bằng chứng ngay trong code: chỉ riêng Gemini đã cần [signatures.rs](../../apps/desktop/src-tauri/src/gateway/signatures.rs).

**Sửa:**
- Nguyên tắc: **chuyển tiếp nguyên định dạng trước, dịch sau.**
- Chỉ viết bộ dịch cho những cặp đã có contract test.
- Đánh giá TensorZero trong 1 ngày trước khi mở rộng bộ dịch.

### OSS-2 · Medium · License của dự án chưa chọn

**Dẫn chứng:** [DECISIONS: Open](../DECISIONS.md#L134).

**Vấn đề:** việc tái dùng dữ liệu hay code MIT/Apache cần ghi nguồn đúng cách và phụ thuộc vào license của chính dự án này.

**Sửa:** chủ dự án chọn license trước khi nhập bất kỳ dữ liệu nào từ bên ngoài.

### OSS-3 · Low · `demo.ts` sao chép luật của core

**Dẫn chứng:** [demo.ts:4](../../apps/desktop/src/lib/demo.ts#L4), [demo.ts:34](../../apps/desktop/src/lib/demo.ts#L34).

**Sửa:** thay luật mô phỏng bằng các snapshot JSON lấy từ app thật, demo chỉ phát lại.

## 6. Roadmap: ADR, Plan và Implementation Status có mâu thuẫn không?

### RM-1 · High · Tài liệu nói gateway chưa có, trong khi code đã chạy

**Dẫn chứng:**
- "Current project state: UI concept and implementation planning only" ([PLAN:596](../SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md#L596)).
- "does not imply gateway… implementation" ([DECISIONS:159](../DECISIONS.md#L159)).
- "six-destination… remains the current implemented design" ([SCREENS:726](../design/SCREENS.md#L726)).
- Plan còn giữ "six-destination navigation" ([PLAN:498](../SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md#L498), [PLAN:579](../SMARTROUTER_UI_UX_FUNCTIONAL_PLAN.md#L579)).
- Trong khi đó [IMPLEMENTATION-STATUS](../IMPLEMENTATION-STATUS.md) mô tả gateway đã được kiểm chứng.

**Sửa:**
- Cập nhật ba chỗ trên.
- Thêm vào CONTRIBUTING một quy tắc: dòng "trạng thái" chỉ được viết trong IMPLEMENTATION-STATUS; các tài liệu khác link tới đó.

### RM-2 · Medium · Cổng và tên CLI không thống nhất

**Dẫn chứng:**
- Tài liệu dùng `4318` ([SCREENS:85](../design/SCREENS.md#L85), [SCREENS:586-592](../design/SCREENS.md#L586), [CLI_IDE_NPM:90](../CLI_IDE_NPM_PROVIDER_TIERS.md#L90)).
- Code dùng `8741` ([mod.rs:36](../../apps/desktop/src-tauri/src/gateway/mod.rs#L36)).
- Tên CLI: `str` (ADR-021) và binary `gateway` hiện có.

**Sửa:**
- Chốt cổng `8741`.
- Đổi tên binary thành `str` cho khớp ADR-021, đổi `serve` thành `start`.

### RM-3 · Medium · Cấu trúc DECISIONS.md

**Dẫn chứng:**
- Mục "Open" ([DECISIONS:134-137](../DECISIONS.md#L134)) nằm *giữa* ADR-016 và ADR-017.
- Mục đó vẫn đề xuất OpenCode làm agent cho benchmark, một hướng đã bị hoãn.
- 6 trong 8 ADR mới mang trạng thái "proposed".

**Sửa:**
- Dời "Open" xuống cuối file và bỏ mục OpenCode.
- ADR-017, 018, 020–023 chuyển thành RFC. Chỉ giữ trong DECISIONS những gì đã quyết (ADR-019, ADR-024).

### RM-4 · Medium · Lược đồ đề xuất trùng lặp

**Dẫn chứng:**
- `model_entitlements` có ở hai nơi với cột khác nhau ([OMNIROUTE §8](../OMNIROUTE_FEATURE_RESEARCH.md#L160), [CLI_IDE_NPM §5](../CLI_IDE_NPM_PROVIDER_TIERS.md#L148)).
- `quota_groups` và `account_quota_groups` trùng ý.
- `client_tokens` trùng với bảng `gateway_clients` đang có.

**Sửa:**
- Một RFC lược đồ duy nhất.
- **Mở rộng** `providers` (= tài khoản) và `models` (= entitlement của tài khoản–model) đã có, thay vì tạo bảng song song.

### Thứ tự đề xuất (thay cho 5 hệ đánh số)

| # | Hạng mục | Lý do đứng ở vị trí này |
| --- | --- | --- |
| 1 | GW-2 (đi thẳng định dạng Anthropic, giữ cache) | Đây là chỗ đang làm chi phí tăng. Phải sửa trước mọi tính năng "tối ưu". |
| 2 | GW-1, GW-6 (quy tắc lỗi, cooldown, không fallback sai) | Rủi ro điều khoản và rủi ro tính tiền hai lần. |
| 3 | GW-3, GW-5 (ngân sách, giới hạn đồng thời, sổ đầy đủ) | Không thể nói "tối ưu chi phí" khi chưa đo đủ chi phí. |
| 4 | GW-4 + USM-3 (session affinity) | Điều kiện tiên quyết cho mọi định tuyến thông minh. |
| 5 | RE-1 (`auto/*` theo luật) + catalog giá có provenance | Mốc "smart" đầu tiên, có giải thích được. |
| 6 | Đo: benchmark chi phí trên mỗi phiên thành công (agentic coding) | Quyết định có làm tiếp 7–8 hay không. |
| 7 | Nén chỉ áp dụng cho tool output mới (append-only), chạy shadow trước | Chỉ làm khi đo được `cache_read` không giảm. |
| 8 | Memory và Auto Chat (chế độ app sở hữu hội thoại) | Chỉ làm khi bước 6 cho thấy wedge có giá trị. |

## 7. Kiểm tra giả định: SmartTaskRouter có thật sự giảm chi phí?

**Giả định của mô hình.** Đây là giá *minh họa* theo tỉ lệ phổ biến, không phải báo giá: chỉ để thấy chiều của tác động.
- **Model mạnh P:** $3/1M input, $15/1M output. Ghi cache = 1,25× giá input, đọc cache = 0,1× giá input.
- **Model rẻ C:** $0,10/1M input, $0,40/1M output, giả định không có cache (thận trọng).
- **Phiên agent:** 40 lượt, trung bình 70k token input mỗi lượt, trong đó 7k token mới; 1k token output mỗi lượt.

| # | Kịch bản | Chi phí/lượt | Chi phí/phiên | So với P có cache |
| --- | --- | --- | --- | --- |
| 1 | P, cache hoạt động (63k × $0,30 + 7k × $3,75 + 1k × $15, tính theo /1M) | $0,060 | **$2,40** | 1× |
| 2 | P, **mất cache** (đường Claude qua gateway hiện tại, GW-2) | $0,225 | **$9,00** | **3,7×** |
| 3 | C toàn phiên | $0,0074 | $0,30 | 0,12× |
| 4 | C + classifier đọc toàn bộ context mỗi lượt (bằng C) | $0,0144 | $0,58 | Gần gấp đôi kịch bản 3 |
| 5 | C + classifier chỉ đọc tin nhắn cuối (2k token) | $0,0076 | $0,31 | Không đáng kể |
| 6 | P + nén lịch sử 30% nhưng tiền tố đổi mỗi lượt (49k × $3,75) | $0,199 | **$7,95** | **3,3×**, đắt hơn không nén |
| 7 | C thất bại ở lượt 20, chuyển sang P (lượt đầu không cache ~100k × $3,75, rồi 20 lượt có cache, mỗi lượt ~$0,07) | | ≈ $1,90 + công hoàn tác | Tiết kiệm ít, rủi ro cao |
| 8 | Timeout rồi fallback (GW-6): thêm 1 lượt P không cache | +$0,375 | | |
| 9 | Trích xuất memory mỗi lượt bằng C trên toàn context | +$0,007 | +$0,28 | Gần gấp đôi kịch bản 3 |

**Kết luận:**

1. **Chọn model *có thể* tiết kiệm nhiều.** C rẻ hơn khoảng 8 lần so với P đã có cache. Điểm hoà vốn khi tính theo tiền là C phải thất bại ít hơn khoảng 80% số phiên.
   - Nhưng chi phí thật của một lần thất bại là **công sức con người** (kịch bản 7), và công thức `C_effective` chưa tính khoản này.
   - Chưa có dữ liệu nào cho thấy tỉ lệ thất bại của C. Bước 6 trong roadmap là để đo nó.
2. **Các tính năng "tối ưu" có thể đảo ngược khoản tiết kiệm:**
   - Nén mà phá cache khiến chi phí ×3,3.
   - Classifier hoặc trích xuất memory trên toàn context khiến chi phí của model rẻ ×2.
   - Đổi model giữa phiên mất khoảng 1 lượt không cache, và có thể kèm thời gian hoàn tác.
   - Fallback khi timeout bị tính tiền hai lần.
3. **Với code hiện tại:**
   - Cơ chế tiết kiệm duy nhất là combo đặt model rẻ trước, do người dùng tự cấu hình.
   - Đường Claude Code tới Claude **đang đắt hơn** gọi thẳng (kịch bản 2).
   - Vì vậy, hiện chưa được phép công bố "tiết kiệm".
4. **Điều kiện cần để tuyên bố giảm chi phí.** Mỗi điều kiện có một cổng đo:
   1. Tiền tố prompt ổn định, `cache_read` không giảm so với gọi thẳng (USM-1, GW-2).
   2. Classifier ≤ 2k token và chỉ chạy khi bắt đầu phiên hoặc sau thất bại (RE-2).
   3. Trích xuất memory theo lô, có dòng riêng trong sổ (USM-4).
   4. Chỉ đổi model tại ranh giới phiên hoặc sau thất bại, có ghi phí chuyển (GW-4).
   5. Retry có giới hạn, không fallback khi timeout (GW-6).
   6. Benchmark chi phí trên mỗi phiên thành công so với baseline "P có cache", có ghi tỉ lệ chi phí Unknown.

**Chưa kiểm chứng trong bản này:**
- Giá thật hiện hành của từng provider.
- Hành vi cache của OpenRouter khi thiếu `cache_control`.
- Việc Claude Code có gửi session ID hay không.

Mỗi mục trên cần một phép đo trước khi chốt GW-2 và USM-3.
