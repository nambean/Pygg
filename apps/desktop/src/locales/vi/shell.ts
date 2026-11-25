/** Window, sidebar, toolbar, settings and placeholder screens. */
export const shell: Record<string, string> = {
  // Sidebar and destinations
  'Sidebar': 'Thanh bên',
  'Tasks': 'Tác vụ',
  'All Tasks': 'Tất cả tác vụ',
  'Running': 'Đang chạy',
  'Needs Approval': 'Chờ duyệt',
  'Completed': 'Đã xong',
  'Failed': 'Thất bại',
  'Stopped': 'Đã dừng',
  'Workspaces': 'Workspace',
  'Add Workspace…': 'Thêm workspace…',
  'Add Workspace': 'Thêm workspace',
  'Manage': 'Quản lý',
  'Connections': 'Kết nối',
  'Policies': 'Chính sách',
  'Usage': 'Mức dùng',
  'Endpoints': 'Endpoint',
  'App': 'Ứng dụng',
  'Settings': 'Cài đặt',
  'Workspace Settings': 'Cài đặt workspace',
  'Workspace Settings…': 'Cài đặt workspace…',

  'Loading…': 'Đang tải…',

  // Toolbar and window
  'Hide sidebar': 'Ẩn thanh bên',
  'Show sidebar': 'Hiện thanh bên',
  'New Task': 'Tác vụ mới',
  'Task': 'Tác vụ',
  'Window': 'Cửa sổ',
  'Close': 'Đóng',
  'Minimize': 'Thu nhỏ',
  'Zoom': 'Phóng to',
  'Sample tasks for design review. The task engine isn’t built yet, so nothing here ran.':
    'Tác vụ mẫu để duyệt thiết kế. Bộ máy chạy tác vụ chưa được xây, nên chưa có gì ở đây thực sự chạy.',

  // Gateway status
  'Core isn’t responding': 'Lõi ứng dụng không phản hồi',
  'Browser preview': 'Xem trước trên trình duyệt',
  'Connecting…': 'Đang kết nối…',
  'Gateway Running': 'Gateway đang chạy',
  'Gateway Stopped': 'Gateway đã dừng',
  'Gateway not built yet': 'Gateway chưa được xây',

  // Budget subtitle and popover
  '{n} running': '{n} đang chạy',
  '{n} waiting': '{n} chờ duyệt',
  'Budget needs review': 'Ngân sách cần xem lại',
  '{recorded} recorded + {reserved} reserved / {limit} today': '{recorded} đã ghi + {reserved} giữ chỗ / {limit} hôm nay',
  'Today’s API budget': 'Ngân sách API hôm nay',
  'Recorded': 'Đã ghi nhận',
  'Reserved for running work': 'Giữ chỗ cho việc đang chạy',
  'Daily limit': 'Hạn mức ngày',
  'Calls with unknown cost': 'Lượt gọi chưa rõ chi phí',
  'Reserved money is held for running attempts; it is not spent. Subscription and local use are not included.':
    'Tiền giữ chỗ được dành cho các lần thử đang chạy, chưa phải đã tiêu. Không tính gói subscription và model local.',

  // Placeholder destinations
  '{name} isn’t built yet': '{name} chưa được xây',
  'Folder, Git and isolation readiness, privacy class, approved checks and policy overrides for one workspace.':
    'Thư mục, trạng thái Git và khả năng cô lập, mức riêng tư, các check đã duyệt và chính sách riêng của một workspace.',
  'Choose a folder, check that it can be isolated with Git, detect its checks and set its privacy class.':
    'Chọn thư mục, kiểm tra có cô lập được bằng Git không, phát hiện các check và đặt mức riêng tư.',
  'Providers and accounts, the model catalog with where each fact comes from, and the agent that runs tasks.':
    'Provider và tài khoản, danh mục model kèm nguồn của từng thông tin, và agent chạy tác vụ.',
  'One place for strategies, tiers, attempt limits, budgets and when paid steps need your approval. Until then, these are set where they apply:':
    'Một nơi chung cho chiến lược, tier, giới hạn số lần thử, ngân sách và khi nào bước trả phí cần bạn duyệt. Trong lúc chờ, các mục này được đặt ngay nơi chúng áp dụng:',
  'Checks and Privacy in Workspaces': 'Check và riêng tư trong Workspace',
  'Daily Budget in Settings': 'Ngân sách ngày trong Cài đặt',
  'Steps and Strategy in New Task': 'Các bước và chiến lược trong Tác vụ mới',
  'Strategies, tiers, attempt limits, budgets and when paid steps need your approval.':
    'Chiến lược, tier, giới hạn số lần thử, ngân sách và khi nào bước trả phí cần bạn duyệt.',
  'The usage ledger with actual, estimated and unknown costs kept apart, plus subscription and local use.':
    'Sổ ghi mức dùng, tách riêng chi phí thực, ước tính và chưa rõ, cùng mức dùng subscription và local.',
  'The local gateway address, client tokens and setup for SDKs and agents.':
    'Địa chỉ gateway local, token cho client và hướng dẫn cài cho SDK và agent.',

  // Settings
  'General': 'Chung',
  'Language': 'Ngôn ngữ',
  'System': 'Theo hệ thống',
  'Appearance': 'Giao diện',
  'Follows the system’s light, dark and contrast settings.': 'Theo chế độ sáng, tối và độ tương phản của hệ thống.',
  // Tray and notifications (src-tauri/src/shell.rs)
  'Open Pygg': 'Mở Pygg',
  'Quit Pygg': 'Thoát Pygg',
  'Pygg is still running': 'Pygg vẫn đang chạy',
  'The gateway and your tasks keep running. Open Pygg or quit it from the tray icon.':
    'Gateway và các task của bạn vẫn chạy. Mở hoặc thoát Pygg từ biểu tượng ở khay hệ thống.',
  'A task is waiting for your approval': 'Có task đang chờ bạn duyệt',
  'Open Pygg to approve or decline its paid step.': 'Mở Pygg để duyệt hoặc từ chối bước trả phí.',
  'A task is ready': 'Có task đã xong',
  'Its checks passed. Open Pygg to keep or discard the result.': 'Check đã qua. Mở Pygg để giữ hoặc bỏ kết quả.',
  'A task needs your review': 'Có task cần bạn xem lại',
  'It finished, but no check confirmed it. Open Pygg to review the changes.':
    'Task đã xong nhưng chưa có check nào xác nhận. Mở Pygg để xem các thay đổi.',
  'A task did not finish': 'Có task không hoàn thành',
  'Open Pygg to see why each step stopped.': 'Mở Pygg để xem vì sao từng bước dừng lại.',
  'Background': 'Chạy nền',
  'Keep running in the tray': 'Tiếp tục chạy ở khay hệ thống',
  'Closing the window keeps the gateway and your tasks running. Quit from the tray icon.':
    'Đóng cửa sổ thì gateway và các task vẫn chạy. Thoát từ biểu tượng ở khay hệ thống.',
  'Notify me about tasks': 'Thông báo về task',
  'When a paid step waits for approval, or a task is ready, needs review or did not finish. Notifications never show a task’s name or files.':
    'Khi bước trả phí chờ duyệt, hoặc task đã xong, cần xem lại hay không hoàn thành. Thông báo không bao giờ hiện tên task hay file.',

  // Added 2026-11-25: the rest of the window, settings and guide, so the whole interface reads in Vietnamese.
  'Go to': 'Đi tới',
  'Agents & defaults': 'Agent & mặc định',
  'Skills & Prompts': 'Skill & Prompt',
  'Guide': 'Hướng dẫn',
  'Simulate auto routing': 'Mô phỏng định tuyến auto',
  'Open Usage to export CSV': 'Mở Mức dùng để xuất CSV',
  'Set up a coding tool': 'Thiết lập công cụ lập trình',
  'New Skill…': 'Skill mới…',
  'Let an IDE agent delegate tasks (MCP)': 'Cho agent trong IDE giao tác vụ (MCP)',
  'New task': 'Tác vụ mới',
  'Ask about a project': 'Hỏi về một dự án',
  'Plan a change (Brain Storming)': 'Lên kế hoạch thay đổi (Brain Storming)',
  'Open Settings to save diagnostics': 'Mở Cài đặt để lưu chẩn đoán',
  'Back': 'Quay lại',
  'Plan: {title}': 'Kế hoạch: {title}',
  'Skip to content': 'Chuyển tới nội dung',
  'Smart Tasks · Experimental': 'Tác vụ thông minh · Thử nghiệm',
  'Command palette': 'Bảng lệnh',
  'Plans': 'Kế hoạch',
  'Loading work…': 'Đang tải công việc…',
  'That project is no longer here.': 'Dự án đó không còn ở đây.',
  '{what} could not be loaded: {error}': 'Không tải được {what}: {error}',
  'Maximize': 'Phóng to',
  'Type a command or a screen': 'Gõ lệnh hoặc tên màn hình',
  'Command': 'Lệnh',
  'Commands': 'Lệnh',
  'No command matches “{query}”': 'Không có lệnh nào khớp “{query}”',
  'Diagnostics': 'Chẩn đoán',
  'Diagnostics bundle': 'Gói chẩn đoán',
  'A zip to send when something goes wrong. It never holds keys, tokens, prompts, agent output, diffs or file contents.':
    'Một file zip để gửi khi có sự cố. Không bao giờ chứa key, token, prompt, đầu ra của agent, diff hay nội dung file.',
  'Save Diagnostics…': 'Lưu chẩn đoán…',
  'What the bundle holds': 'Gói này chứa gì',
  'App, schema and Windows versions; the gateway’s state and port.':
    'Phiên bản ứng dụng, schema và Windows; trạng thái và cổng của gateway.',
  'Each connection’s kind, endpoint without its query, enabled state, billing, terms, health, and whether a key is set. Not its label.':
    'Loại của từng kết nối, endpoint không kèm query, trạng thái bật, cách tính phí, điều khoản, tình trạng, và đã đặt key hay chưa. Không gồm nhãn.',
  'Each client’s name and limits. Not its token.': 'Tên và giới hạn của từng client. Không gồm token.',
  'How many rows each table holds, and the latest 200 requests: models, outcome, tokens and cost. Not the provider’s error text.':
    'Số dòng của mỗi bảng, và 200 request gần nhất: model, kết quả, token và chi phí. Không gồm nội dung lỗi của nhà cung cấp.',
  'Each task’s state and its steps’ states. Not its title, prompt or reasons.':
    'Trạng thái của từng tác vụ và của các bước. Không gồm tiêu đề, prompt hay lý do.',
  'The last 200 lines of Pygg’s log (logs\\pygg.log), already redacted when written.':
    '200 dòng cuối trong log của Pygg (logs\\pygg.log), đã được ẩn thông tin nhạy cảm khi ghi.',
  'Every text is cut to 200 characters and passed through the same redaction as task logs. The terminal can save the same zip with pygg-cli diagnostics --out PATH.':
    'Mọi đoạn văn bản được cắt còn 200 ký tự và được ẩn thông tin nhạy cảm giống log tác vụ. Từ terminal có thể lưu cùng file zip này bằng pygg-cli diagnostics --out PATH.',
  'Set in Endpoint, next to the gateway’s address.': 'Đặt trong Endpoint, cạnh địa chỉ của gateway.',
  'About': 'Giới thiệu',
  'Pygg {version}': 'Pygg {version}',
  'Developer preview for Windows x64. MIT licence.': 'Bản xem trước cho nhà phát triển, Windows x64. Giấy phép MIT.',
  'Source Code': 'Mã nguồn',
  'Report an Issue': 'Báo lỗi',
  'Third-party licences': 'Giấy phép bên thứ ba',
  'The open-source packages built into Pygg and their licences. A copy, THIRD-PARTY-LICENSES.md, is installed next to Pygg.exe.':
    'Các gói mã nguồn mở tích hợp trong Pygg và giấy phép của chúng. Một bản sao, THIRD-PARTY-LICENSES.md, được cài cạnh Pygg.exe.',
  'View Licences': 'Xem giấy phép',
  'Log and backups': 'Nhật ký và bản sao lưu',
  'In Pygg’s data folder (%APPDATA%\\app.smarttaskrouter.desktop): logs\\pygg.log, redacted, which Save Diagnostics includes; and backups\\, a copy of the store made each day, the newest three kept. To restore one, quit Pygg and copy it over store.sqlite3.':
    'Trong thư mục dữ liệu của Pygg (%APPDATA%\\app.smarttaskrouter.desktop): logs\\pygg.log, đã ẩn thông tin nhạy cảm, có trong gói Lưu chẩn đoán; và backups\\, bản sao của kho dữ liệu tạo mỗi ngày, giữ ba bản mới nhất. Để khôi phục, thoát Pygg rồi chép bản sao đè lên store.sqlite3.',
  'Open Pygg when you sign in': 'Mở Pygg khi đăng nhập',
  'Pygg starts in the tray when you sign in to Windows, so the gateway is ready for your tools. Open it from the tray icon.':
    'Pygg khởi động ở khay hệ thống khi bạn đăng nhập Windows, để gateway sẵn sàng cho các công cụ của bạn. Mở Pygg từ biểu tượng ở khay hệ thống.',
  'Notify me about spending': 'Thông báo về chi tiêu',
  'When a client nears or reaches its monthly limit, spending is three times the week’s average, or many requests fail. Once a day each, checked every ten minutes.':
    'Khi một client sắp chạm hoặc đã chạm hạn mức tháng, chi tiêu gấp ba mức trung bình của tuần, hoặc nhiều request thất bại. Mỗi loại tối đa một lần mỗi ngày, kiểm tra mỗi mười phút.',
  'Enter an amount above 0, or leave it empty for no limit.':
    'Nhập số tiền lớn hơn 0, hoặc để trống nếu không giới hạn.',
  'Connect a provider.': 'Kết nối một nhà cung cấp.',
  'Providers → Connect Provider. A cloud provider takes its API key (Gemini, OpenRouter and Groq have free tiers); a local runtime such as Ollama takes its local address and no key. Pygg reads the model list; nothing is billed.':
    'Nhà cung cấp → Kết nối nhà cung cấp. Nhà cung cấp đám mây cần API key (Gemini, OpenRouter và Groq có gói miễn phí); runtime cục bộ như Ollama cần địa chỉ cục bộ, không cần key. Pygg đọc danh sách model; không có gì bị tính phí.',
  'Set up your coding tool.': 'Thiết lập công cụ lập trình.',
  'If CLI Tools says auto has no eligible model, open the connection’s Models and review its routing facts (Auto…).':
    'Nếu Công cụ CLI báo auto không có model đủ điều kiện, hãy mở Model của kết nối và xem lại thông tin định tuyến (Auto…).',
  'CLI Tools: Codex and OpenCode in one click, Claude Code with a launcher, editors and SDKs with a snippet. CLI Tools makes a separate Pygg API key for each tool it sets up; for a tool set up by hand, make one in Endpoint → API Keys.':
    'Công cụ CLI: Codex và OpenCode chỉ một cú nhấp, Claude Code qua launcher, trình soạn thảo và SDK qua một đoạn mã. Công cụ CLI tạo một API key Pygg riêng cho mỗi công cụ nó thiết lập; với công cụ thiết lập thủ công, hãy tạo key trong Endpoint → API key.',
  'Work as usual, with the model auto.': 'Làm việc như thường lệ, với model auto.',
  'Pygg picks an eligible model for each request, within your limits, and records what it cost. CLI Tools says “Working” after that tool receives an answer; Usage shows the cost and an estimate of what you saved.':
    'Pygg chọn một model đủ điều kiện cho từng request, trong giới hạn của bạn, và ghi lại chi phí. Công cụ CLI báo “Hoạt động” sau khi công cụ đó nhận được câu trả lời; Mức dùng hiển thị chi phí và ước tính khoản bạn tiết kiệm được.',
  'The gateway listens on this computer only. A tool set up by hand needs these three fields and a Pygg API key:':
    'Gateway chỉ lắng nghe trên máy này. Công cụ thiết lập thủ công cần ba trường sau và một API key Pygg:',
  'Addresses': 'Địa chỉ',
  'Pointing Claude Code’s own settings at another endpoint would stop it using your Claude plan, so Pygg uses a launcher: set it up once in CLI Tools (or with the command below), then start Claude Code through Pygg when you want it. Plain claude keeps your Claude sign-in.':
    'Trỏ cài đặt của chính Claude Code sang endpoint khác sẽ khiến nó không dùng gói Claude của bạn nữa, nên Pygg dùng launcher: thiết lập một lần trong Công cụ CLI (hoặc bằng lệnh bên dưới), rồi khởi chạy Claude Code qua Pygg khi bạn muốn. Lệnh claude thông thường vẫn giữ đăng nhập Claude của bạn.',
  'Claude Code through Pygg': 'Claude Code qua Pygg',
  'Arguments after claude are Claude Code’s own; --model before claude sets the model for every slot. Claude Code may say it does not recognize the model “auto” and assume a 200k context: that is expected.':
    'Các tham số sau claude là của chính Claude Code; --model đặt trước claude sẽ đặt model cho mọi vị trí. Claude Code có thể báo không nhận ra model “auto” và giả định context 200k: điều đó là bình thường.',
  'To let Claude Code hand tasks to Pygg instead (smart tasks, experimental):':
    'Còn để Claude Code giao tác vụ cho Pygg (tác vụ thông minh, thử nghiệm):',
  'MCP server for Claude Code': 'MCP server cho Claude Code',
  'Pygg adds a pygg provider and a profile to Codex’s configuration, after a backup. Codex’s default model is not changed: choose Pygg per run with the profile.':
    'Pygg thêm nhà cung cấp pygg và một profile vào cấu hình của Codex, sau khi sao lưu. Model mặc định của Codex không bị đổi: chọn Pygg cho từng lần chạy bằng profile.',
  'Codex through Pygg': 'Codex qua Pygg',
  'Undo': 'Hoàn tác',
  'Pygg adds a provider named Pygg to opencode.json, after a backup, listing auto, auto/cheap and your enabled models; it keeps the list current when models change.':
    'Pygg thêm một nhà cung cấp tên Pygg vào opencode.json, sau khi sao lưu, liệt kê auto, auto/cheap và các model bạn đã bật; danh sách được cập nhật khi model thay đổi.',
  'OpenCode through Pygg': 'OpenCode qua Pygg',
  'Cline, Continue, Roo Code and other tools that call the API from this computer: choose an OpenAI-compatible provider and fill in the address, a Pygg API key from Endpoint, and the model. Tools that send requests from their own servers cannot reach 127.0.0.1.':
    'Cline, Continue, Roo Code và các công cụ khác gọi API từ máy này: chọn nhà cung cấp tương thích OpenAI rồi điền địa chỉ, một API key Pygg từ Endpoint, và model. Công cụ gửi request từ máy chủ của chính họ không truy cập được 127.0.0.1.',
  'Fields': 'Các trường',
  'These send a real request, recorded in Usage. The gateway serves three formats:':
    'Các lệnh này gửi request thật, được ghi trong Mức dùng. Gateway phục vụ ba định dạng:',
  'Formats': 'Định dạng',
  'IDE agents that take MCP servers (Claude Code, Codex, Cursor…) can also start Pygg’s MCP server over stdio, to hand tasks to Pygg (experimental). It is a program they start, not an address:':
    'Các agent trong IDE hỗ trợ MCP server (Claude Code, Codex, Cursor…) cũng có thể khởi chạy MCP server của Pygg qua stdio, để giao tác vụ cho Pygg (thử nghiệm). Đây là chương trình chúng khởi chạy, không phải một địa chỉ:',
  'The pygg command does from a terminal what CLI Tools does with a click. npm installs the command only: the Pygg app still runs the gateway, and your coding tool is installed as before. Install @pygg/cli; run pygg. @dev is the Windows x64 developer preview, not code-signed, so SmartScreen or an antivirus may warn about it.':
    'Lệnh pygg làm từ terminal những gì Công cụ CLI làm bằng một cú nhấp. npm chỉ cài lệnh: ứng dụng Pygg vẫn chạy gateway, và công cụ lập trình của bạn vẫn được cài như trước. Cài @pygg/cli; chạy pygg. @dev là bản xem trước cho nhà phát triển trên Windows x64, chưa ký mã, nên SmartScreen hoặc phần mềm diệt virus có thể cảnh báo.',
  'Install, check, set up Claude Code, start it through Pygg': 'Cài, kiểm tra, thiết lập Claude Code, chạy qua Pygg',
  'Try it without installing globally': 'Dùng thử không cần cài toàn cục',
  'npx downloads the package into npm’s cache and runs it; setup still makes the same changes to your tool’s configuration. Without Node.js, use the program the installer put next to the app:':
    'npx tải package vào cache của npm rồi chạy; setup vẫn thực hiện đúng các thay đổi đó với cấu hình công cụ của bạn. Không có Node.js thì dùng chương trình mà bộ cài đặt đặt cạnh ứng dụng:',
  'Without npm': 'Không dùng npm',
  'If pygg is not found.': 'Nếu không tìm thấy pygg.',
  'Open a new terminal first. Then check where npm put it, and add that folder to your user PATH if it is missing:':
    'Trước hết hãy mở terminal mới. Sau đó kiểm tra npm đặt nó ở đâu, và thêm thư mục đó vào PATH của người dùng nếu còn thiếu:',
  'Where npm put the command': 'npm đã đặt lệnh ở đâu',
  'If PowerShell refuses to run scripts, call pygg.cmd instead of pygg. To use another program, set PYGG_CLI to its full path; pygg --launcher-info shows which program the command runs.':
    'Nếu PowerShell từ chối chạy script, hãy gọi pygg.cmd thay cho pygg. Để dùng chương trình khác, đặt PYGG_CLI thành đường dẫn đầy đủ của nó; pygg --launcher-info cho biết lệnh đang chạy chương trình nào.',
  'What it does': 'Chức năng',
  'Where you are: the gateway, providers, tools set up, and what to type next':
    'Tình trạng hiện tại: gateway, nhà cung cấp, công cụ đã thiết lập, và cần gõ gì tiếp theo',
  'The three tools, whether each is installed and set up, and what to type next':
    'Ba công cụ, mỗi cái đã cài và thiết lập chưa, và cần gõ gì tiếp theo',
  'Route the tool’s requests through Pygg (--dry-run previews, --remove undoes)':
    'Định tuyến request của công cụ qua Pygg (--dry-run để xem trước, --remove để hoàn tác)',
  'Start the tool through Pygg, a routing session per launch (--model before claude picks a model)':
    'Khởi chạy công cụ qua Pygg, mỗi lần chạy là một phiên định tuyến (--model trước claude để chọn model)',
  'Is the gateway answering, and on which store (exit code 3 if not)':
    'Gateway có đang trả lời không, và trên store nào (mã thoát 3 nếu không)',
  'Status, plus your tools and the providers that need attention':
    'Trạng thái, kèm công cụ của bạn và nhà cung cấp cần chú ý',
  'Saved connections: on, last check, health, models': 'Kết nối đã lưu: bật, lần kiểm tra cuối, tình trạng, model',
  'The models to ask for, with billing and price': 'Các model có thể gọi, kèm cách tính phí và giá',
  'Recent request attempts, newest first': 'Các lần thử request gần đây, mới nhất trước',
  'Requests, known and unknown cost, token mix, by client and model':
    'Request, chi phí đã biết và chưa rõ, cơ cấu token, theo client và model',
  'On status, doctor, providers, models, logs and usage: the same as JSON for scripts':
    'Với status, doctor, providers, models, logs và usage: cùng nội dung dạng JSON cho script',
  'The experimental MCP task integration, not the gateway: the tool can hand tasks to Pygg':
    'Tích hợp tác vụ MCP thử nghiệm, không phải gateway: công cụ có thể giao tác vụ cho Pygg',
  'One short request to a model (a paid model costs a little)':
    'Một request ngắn tới model (model trả phí sẽ tốn một ít)',
  'Fill model facts (prices, context, tools) from the models.dev catalog':
    'Điền thông tin model (giá, context, tool) từ danh mục models.dev',
  'Which model auto would choose, and why': 'auto sẽ chọn model nào, và vì sao',
  'The gateway without the window, on another store (--db PATH)':
    'Gateway không có cửa sổ, trên store khác (--db PATH)',
  'The agent CLIs installed and how they sign in': 'Các agent CLI đã cài và cách chúng đăng nhập',
  'A redacted bundle for a bug report: no keys, tokens or prompts':
    'Gói đã ẩn thông tin nhạy cảm để báo lỗi: không có key, token hay prompt',
  'The MCP server that IDE agents start': 'MCP server mà agent trong IDE khởi chạy',
  'pygg <command> --help lists every option. Keys are never taken on the command line: add-connection reads one from an environment variable (--key-env).':
    'pygg <command> --help liệt kê mọi tùy chọn. Key không bao giờ được nhận qua dòng lệnh: add-connection đọc key từ một biến môi trường (--key-env).',
  'an eligible model at or above the key’s minimum tier (T2 unless changed), local, free and subscription before paid, then the lowest estimated cost. It chooses within your rules; it does not judge how hard a prompt is.':
    'một model đủ điều kiện có tier bằng hoặc cao hơn tier tối thiểu của key (T2 nếu không đổi), ưu tiên cục bộ, miễn phí và thuê bao trước trả phí, rồi đến chi phí ước tính thấp nhất. Nó chọn trong phạm vi quy tắc của bạn; không đánh giá độ khó của prompt.',
  'the lowest estimated cost among the models that can take the request.':
    'chi phí ước tính thấp nhất trong số các model nhận được request.',
  'the best public benchmark fact for that role first (the response names the benchmark and its date), then the rest by tier.':
    'trước hết là kết quả benchmark công khai tốt nhất cho vai trò đó (phản hồi ghi tên benchmark và ngày của nó), sau đó các model còn lại theo tier.',
  'a combo’s name: your own ordered list; the next model answers when one is out of quota or unavailable.':
    'tên một combo: danh sách có thứ tự của riêng bạn; model kế tiếp trả lời khi một model hết quota hoặc không khả dụng.',
  'Open Combos': 'Mở Combo',
  'one model of one connection, by name.': 'một model của một kết nối, theo tên.',
  'From the command line (CONNECTION and MODEL are placeholders). The auto command only shows the choice; test-model sends one short request and may cost a little.':
    'Từ dòng lệnh (CONNECTION và MODEL là giá trị cần thay). Lệnh auto chỉ hiển thị lựa chọn; test-model gửi một request ngắn và có thể tốn một ít phí.',
  'Combos and auto': 'Combo và auto',
  'Each API key can have a monthly limit, a strict limit, and a minimum tier for auto (Endpoint → API Keys → Limits).':
    'Mỗi API key có thể có hạn mức hằng tháng, giới hạn nghiêm ngặt và tier tối thiểu cho auto (Endpoint → API key → Giới hạn).',
  'A cost Pygg cannot know is shown as unknown, never as $0, and is left out of totals. Give a model a price in Providers to estimate it.':
    'Chi phí Pygg không biết được hiển thị là chưa rõ, không bao giờ là $0, và không được cộng vào tổng. Đặt giá cho model trong Nhà cung cấp để ước tính.',
  'Usage → Estimated Savings prices the same tokens at a model you choose, and says what it left out. It is an estimate, at list prices.':
    'Mức dùng → Tiết kiệm ước tính tính giá cùng lượng token đó theo một model bạn chọn, và cho biết phần nào bị bỏ qua. Đây là ước tính, theo giá niêm yết.',
  'Subscriptions count against their plan, not as money.':
    'Thuê bao được tính vào hạn mức của gói, không tính bằng tiền.',
  'You see': 'Bạn thấy',
  'What to do': 'Cách xử lý',
  'Connection refused, or “nothing is listening”': 'Kết nối bị từ chối, hoặc “nothing is listening”',
  'Start the Pygg app (it serves the gateway), or check with pygg status. The port is set in Endpoint → Configure.':
    'Mở ứng dụng Pygg (nó chạy gateway), hoặc kiểm tra bằng pygg status. Cổng được đặt trong Endpoint → Cấu hình.',
  'The API key is wrong or revoked. Endpoint → API Keys → Show reveals it again (keys made before this version cannot be shown: create a new one).':
    'API key sai hoặc đã bị thu hồi. Endpoint → API key → Hiển thị sẽ hiện lại key (key tạo trước phiên bản này không hiển thị được: hãy tạo key mới).',
  'HTTP 422 “No connected model is eligible for auto”': 'HTTP 422 “No connected model is eligible for auto”',
  'No model has every fact auto needs (tier, price, tools, context). Run pygg gateway lookup, set them in Providers → Auto…, lower the key’s minimum tier, or name a model.':
    'Không model nào có đủ thông tin auto cần (tier, giá, tool, context). Chạy pygg gateway lookup, đặt chúng trong Nhà cung cấp → Auto…, hạ tier tối thiểu của key, hoặc chỉ định một model.',
  'HTTP 429 or “cooling down”': 'HTTP 429 hoặc “đang tạm nghỉ”',
  'A provider’s rate limit or quota. auto and combos move on to the next model; Providers shows when it is tried again.':
    'Nhà cung cấp giới hạn tốc độ hoặc hết quota. auto và combo chuyển sang model kế tiếp; Nhà cung cấp cho biết khi nào nó được thử lại.',
  'A cost shows as unknown': 'Chi phí hiển thị là chưa rõ',
  'The model has no price. Set one in Providers, or run pygg gateway lookup.':
    'Model chưa có giá. Đặt giá trong Nhà cung cấp, hoặc chạy pygg gateway lookup.',
  'Gemini CLI: “client no longer supported”': 'Gemini CLI: “client no longer supported”',
  'Google no longer accepts its free Google-account login from Gemini CLI. Use a Gemini API key, or connect Gemini in Providers and use it through Pygg.':
    'Google không còn nhận đăng nhập miễn phí bằng tài khoản Google từ Gemini CLI. Hãy dùng API key Gemini, hoặc kết nối Gemini trong Nhà cung cấp và dùng qua Pygg.',
  'Before asking for help': 'Trước khi nhờ hỗ trợ',
  'Does Pygg provide models?': 'Pygg có cung cấp model không?',
  'No. It routes your requests to the providers you connect, with your own keys or local runtimes.':
    'Không. Pygg chuyển request của bạn tới các nhà cung cấp bạn kết nối, bằng key của bạn hoặc runtime cục bộ.',
  'Is free unlimited?': 'Miễn phí có phải là không giới hạn?',
  'No. Free tiers have their own limits, set by each provider; Pygg moves on to the next model when one is used up.':
    'Không. Gói miễn phí có hạn mức riêng do từng nhà cung cấp đặt; Pygg chuyển sang model kế tiếp khi một gói đã dùng hết.',
  'Why is a cost unknown?': 'Vì sao chi phí chưa rõ?',
  'The model has no price, the provider reported no usage, or a failed request may have been billed. Unknown is never counted as $0.':
    'Model chưa có giá, nhà cung cấp không báo mức dùng, hoặc một request lỗi có thể đã bị tính phí. Chưa rõ không bao giờ được tính là $0.',
  'Why did it stop after a server error?': 'Vì sao dừng lại sau lỗi máy chủ?',
  'The provider may already have run and billed the request. Pygg tries another model only if the API key allows paid retries.':
    'Nhà cung cấp có thể đã chạy và tính phí request. Pygg chỉ thử model khác nếu API key cho phép thử lại có trả phí.',
  'Does fallback retry a poor answer?': 'Fallback có thử lại khi câu trả lời kém không?',
  'No. Fallback happens on errors before the answer starts; Pygg does not judge answers.':
    'Không. Fallback chỉ xảy ra khi có lỗi trước lúc câu trả lời bắt đầu; Pygg không đánh giá câu trả lời.',
  'Can another computer use my Pygg?': 'Máy khác có dùng được Pygg của tôi không?',
  'No. The gateway listens on 127.0.0.1 only.': 'Không. Gateway chỉ lắng nghe trên 127.0.0.1.',
  'Does using Claude Code through Pygg use my Claude plan?':
    'Dùng Claude Code qua Pygg có tính vào gói Claude của tôi không?',
  'No: through Pygg, Claude Code uses your connected providers. Plain claude, started without pygg run, keeps your plan.':
    'Không: qua Pygg, Claude Code dùng các nhà cung cấp bạn đã kết nối. Lệnh claude thường, chạy không qua pygg run, vẫn dùng gói của bạn.',
  'Guide sections': 'Các mục hướng dẫn',
  'Forever': 'Vĩnh viễn',
  '1 year': '1 năm',
  '{n} days': '{n} ngày',
  'Usage History': 'Lịch sử mức dùng',
  'Keep usage history for': 'Thời gian giữ lịch sử mức dùng',
  'The requests in Usage and why each route was chosen. Older ones are deleted when Pygg starts and once a day. Requests a task’s cost is counted from, and this month’s requests that client limits count, are always kept.':
    'Các request trong Mức dùng và lý do chọn từng tuyến. Request cũ hơn bị xóa khi Pygg khởi động và mỗi ngày một lần. Các request dùng để tính chi phí của tác vụ, và các request tháng này được tính vào hạn mức của client, luôn được giữ lại.',
  'Counting the requests older than that…': 'Đang đếm các request cũ hơn mốc đó…',
  'Saving; deleting the older history can take a moment…': 'Đang lưu; xóa lịch sử cũ có thể mất một lúc…',
  'Last clean-up {when}: nothing was old enough to delete.': 'Lần dọn gần nhất {when}: không có gì đủ cũ để xóa.',
  'Keep usage history for {period}?': 'Giữ lịch sử mức dùng: {period}?',
  'Delete Older History': 'Xóa lịch sử cũ',
  'Last clean-up {when}: {n} request deleted.': 'Lần dọn gần nhất {when}: đã xóa {n} request.',
  'Last clean-up {when}: {n} requests deleted.': 'Lần dọn gần nhất {when}: đã xóa {n} request.',
  '{n} request older than that is deleted now, and older ones every day after. This cannot be undone. Export CSV in Usage first to keep a copy.':
    '{n} request cũ hơn mốc đó sẽ bị xóa ngay, sau đó request quá hạn bị xóa mỗi ngày. Không thể hoàn tác. Hãy xuất CSV trong Mức dùng trước để giữ một bản.',
  '{n} requests older than that are deleted now, and older ones every day after. This cannot be undone. Export CSV in Usage first to keep a copy.':
    '{n} request cũ hơn mốc đó sẽ bị xóa ngay, sau đó request quá hạn bị xóa mỗi ngày. Không thể hoàn tác. Hãy xuất CSV trong Mức dùng trước để giữ một bản.',
  'More': 'Tuỳ chọn',
  'requests': 'request',
  'tokens': 'token',
  'input tokens': 'token đầu vào',
  'output tokens': 'token đầu ra',
  'free requests today': 'request miễn phí hôm nay',
  'asked to wait': 'được yêu cầu chờ',
  '{left} of {limit} left': 'còn {left}/{limit}',
  '{left} left': 'còn {left}',
  'resets {time}': 'đặt lại lúc {time}',
  'A client has reached its monthly limit': 'Một client đã chạm hạn mức tháng',
  'Its requests are refused until next month or a higher limit. Open Pygg to see Usage.':
    'Request của client bị từ chối cho tới tháng sau hoặc khi hạn mức được nâng. Mở Pygg để xem Mức dùng.',
  'A client is near its monthly limit': 'Một client sắp chạm hạn mức tháng',
  'It has used 80% or more of it. Open Pygg to see Usage.':
    'Client đã dùng từ 80% hạn mức trở lên. Mở Pygg để xem Mức dùng.',
  'Spending is up': 'Chi tiêu đang tăng',
  'The last 24 hours cost three times the week’s daily average or more. Open Pygg to see Usage.':
    'Chi phí 24 giờ qua gấp ba mức trung bình ngày của tuần trở lên. Mở Pygg để xem Mức dùng.',
  'Many requests are failing': 'Nhiều request đang thất bại',
  'Half or more of the last hour’s requests failed. Open Pygg to see Usage.':
    'Từ một nửa số request trong giờ qua trở lên đã thất bại. Mở Pygg để xem Mức dùng.',
  'Quick start': 'Bắt đầu nhanh',
  'Editors and SDKs': 'Trình soạn thảo và SDK',
  'Command line and npm': 'Dòng lệnh và npm',
  'auto, combos and model names': 'auto, combo và tên model',
  'Costs and limits': 'Chi phí và hạn mức',
  'Troubleshooting': 'Khắc phục sự cố',
  'FAQ': 'Câu hỏi thường gặp',
  'Home': 'Trang chủ',
  'CLI Tools': 'Công cụ CLI',
};
