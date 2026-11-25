/** Task list, task detail, new task and the attempt sequence. */
export const tasks: Record<string, string> = {
  // List
  'Search Tasks': 'Tìm tác vụ',
  'Needs Attention': 'Cần xử lý',
  'Review': 'Xem xét',
  'Open': 'Mở',
  'No Tasks Match “{query}”': 'Không có tác vụ nào khớp “{query}”',
  'Clear Search': 'Xoá tìm kiếm',
  'Ready for Your First Task': 'Sẵn sàng cho tác vụ đầu tiên',
  'Describe the work, review the route the app proposes, then run it.':
    'Mô tả việc cần làm, xem lộ trình ứng dụng đề xuất, rồi mới chạy.',
  'No Tasks': 'Chưa có tác vụ',
  'Nothing Is Running': 'Không có gì đang chạy',
  'Nothing Is Waiting for Approval': 'Không có gì chờ duyệt',
  'No Completed Tasks': 'Chưa có tác vụ nào xong',
  'No Failed Tasks': 'Không có tác vụ thất bại',
  'No Stopped Tasks': 'Không có tác vụ đã dừng',

  // Status lines
  'Preparing': 'Đang chuẩn bị',
  'Checking': 'Đang chạy check',
  'Generating': 'Đang sinh code',
  '{phase} · attempt {n} · {time}': '{phase} · lần thử {n} · {time}',
  'Waiting for approval · attempt {n}': 'Chờ duyệt · lần thử {n}',
  'Plan needs revalidation': 'Kế hoạch cần kiểm tra lại',
  'Checks passed · {time}': 'Check đã qua · {time}',
  'Manually accepted · {time}': 'Đã chấp nhận thủ công · {time}',
  'Check failed · {time}': 'Check thất bại · {time}',
  'Check failed after {n} attempts · {time}': 'Check thất bại sau {n} lần thử · {time}',
  'Blocked by policy · {time}': 'Bị chính sách chặn · {time}',
  'Canceled · {time}': 'Đã huỷ · {time}',
  'Interrupted · needs review': 'Bị gián đoạn · cần xem lại',

  // Attempts and billing
  'Attempts': 'Các lần thử',
  'Free': 'Miễn phí',
  // 'Local' and 'Subscription' are in gateway.ts, one wording for both screens.
  'Paid': 'Trả phí',
  'Unknown billing': 'Chưa rõ cách tính phí',
  'Passed': 'Đã qua',
  'Skipped: not answering': 'Bỏ qua: không phản hồi',
  'Waiting for approval': 'Chờ duyệt',
  'API {zero} · local': 'API {zero} · local',
  '{actual} + ≈{estimated} est.': '{actual} + ≈{estimated} ước tính',
  '1 call unknown': '1 lượt gọi chưa rõ',
  '{n} calls unknown': '{n} lượt gọi chưa rõ',

  'Finished, not checked': 'Đã xong, chưa được check',
  'Finished, needs your review · {time}': 'Đã xong, cần bạn xem lại · {time}',

  // Detail
  'Cost so far: {cost}': 'Chi phí đến giờ: {cost}',
  'The first step is starting.': 'Bước đầu tiên đang khởi động.',
  'Agents run with their own sign-in, so their cost is unknown to Pygg.':
    'Agent chạy bằng đăng nhập riêng của nó, nên Pygg không biết chi phí.',
  'Steps': 'Các bước',
  'None approved': 'Chưa duyệt check nào',
  'Started from': 'Bắt đầu từ',
  'The result was discarded.': 'Kết quả đã bị bỏ.',
  'Checks passed': 'Check đã qua',
  'Needs your review': 'Cần bạn xem lại',
  'Paid step': 'Bước trả phí',
  'Export Patch…': 'Xuất patch…',
  'Save the changes as a file git apply takes; the result stays here': 'Lưu thay đổi thành file dùng được với git apply; kết quả vẫn giữ ở đây',
  'Saved as {path}. The result is still here to keep or discard.': 'Đã lưu thành {path}. Kết quả vẫn ở đây để giữ hoặc bỏ.',
  'Left of today’s budget for paid steps: {amount}.': 'Ngân sách hôm nay cho bước trả phí còn {amount}.',
  'Task budgets': 'Ngân sách task',
  'Paid steps per day': 'Bước trả phí mỗi ngày',
  'All tasks together. A paid step is refused once it is used, and a cap cannot be more than what is left. Resets at 00:00 UTC.':
    'Tính chung mọi task. Hết ngân sách thì bước trả phí bị từ chối, và mức chi không được vượt phần còn lại. Đặt lại lúc 00:00 UTC.',
  'Only paid steps count, at the agent’s own estimate, or at their cap when it reported none. Free, local and subscription steps are not billed per request. A workspace can have its own limit, on the Workspaces screen.':
    'Chỉ tính bước trả phí, theo ước tính của chính agent, hoặc bằng mức chi nếu agent không báo. Bước miễn phí, local và subscription không tính tiền theo request. Mỗi workspace có thể có giới hạn riêng, ở màn Workspaces.',
  '{recorded} on paid steps today': '{recorded} cho bước trả phí hôm nay',
  'Today’s budget for paid steps': 'Ngân sách hôm nay cho bước trả phí',
  'Runs with unknown cost': 'Lần chạy chưa rõ chi phí',
  'Recorded is the agents’ own estimates; a run that reported none counts at its cap. Reserved is held for running steps. Free, local and subscription steps are not included. Resets at 00:00 UTC; set the limit in Settings.':
    'Đã ghi là ước tính của chính các agent; lần chạy không báo chi phí được tính bằng mức chi. Đã giữ là phần dành cho bước đang chạy. Không tính bước miễn phí, local và subscription. Đặt lại lúc 00:00 UTC; đặt giới hạn trong Settings.',
  'No estimate: this agent and model have no earlier run that reported a cost.':
    'Chưa có ước tính: agent và model này chưa có lần chạy nào báo chi phí.',
  'Its last {n} runs cost {amount} each, by the agent’s own list-price estimate.':
    '{n} lần chạy gần nhất đều tốn {amount}, theo ước tính giá niêm yết của chính agent.',
  'Its last {n} runs cost {low} to {high}, by the agent’s own list-price estimate.':
    '{n} lần chạy gần nhất tốn từ {low} đến {high}, theo ước tính giá niêm yết của chính agent.',
  'Spending cap': 'Mức chi tối đa',
  'The agent stops at this amount itself.': 'Agent tự dừng khi chạm mức này.',
  'This agent cannot stop at a cap. If the cost it reports afterwards is over it, Pygg tries no further step.':
    'Agent này không tự dừng theo mức chi. Nếu chi phí nó báo sau khi chạy vượt mức này, Pygg sẽ không thử bước nào nữa.',
  'Approve up to {amount}': 'Duyệt tối đa {amount}',
  'Approve Step': 'Duyệt bước này',
  'Stop further steps above': 'Dừng các bước sau nếu vượt',
  'Discard…': 'Bỏ…',
  'Workspaces could not be loaded: {error}': 'Không tải được danh sách workspace: {error}',
  'Agents could not be listed: {error}': 'Không liệt kê được agent: {error}',
  'Try Again': 'Thử lại',
  'Trying again…': 'Đang thử lại…',
  'Tasks could not be loaded: {error}': 'Không tải được danh sách tác vụ: {error}',
  'Loading tasks…': 'Đang tải tác vụ…',
  'Review Changes': 'Xem thay đổi',
  'Copy Diff': 'Sao chép diff',
  'What it was told about earlier attempts': 'Những gì agent được báo về các lần thử trước',
  'Loading changes…': 'Đang tải thay đổi…',
  'No changes.': 'Không có thay đổi.',
  'Binary file; not shown.': 'File nhị phân, không hiển thị.',
  'Show all {n} lines': 'Hiện cả {n} dòng',
  'Keep as Branch saves the result as a new branch in the repository. The folder you have open, and its branch, are not changed.':
    'Giữ thành nhánh lưu kết quả thành một nhánh mới trong repository. Thư mục bạn đang mở và nhánh của nó không bị thay đổi.',
  'Discard this result?': 'Bỏ kết quả này?',
  'Discard Result': 'Bỏ kết quả',
  'The changes and the copy of the repository they were made in are deleted. This cannot be undone. Export Patch first to keep a copy.':
    'Các thay đổi và bản sao repository chứa chúng sẽ bị xoá, không hoàn tác được. Muốn giữ một bản thì Xuất patch trước.',
  'agent’s estimate {amount}': 'agent ước tính {amount}',
  'Pygg’s ledger {amount}': 'theo sổ chi của Pygg {amount}',
  '{n} requests through Pygg without a price': '{n} request qua Pygg chưa có giá',
  'Some requests through Pygg used a model without a price: enter it in Connections.':
    'Có request qua Pygg dùng model chưa có giá: hãy nhập giá trong Kết nối.',
  'cap {amount}': 'mức chi {amount}',
  'Decline': 'Từ chối',
  'Retry': 'Chạy lại',
  'Stop Task': 'Dừng tác vụ',
  'Step {n}': 'Bước {n}',
  '{input} in · {output} out': '{input} vào · {output} ra',
  'Check output': 'Output của check',

  // New task
  'Describe the change, e.g. “Fix timeout handling and add regression tests.”':
    'Mô tả thay đổi, ví dụ “Sửa xử lý timeout và thêm test hồi quy.”',
  'Strategy': 'Chiến lược',
  'Starts with the agents most likely to be free; a paid step asks first.':
    'Bắt đầu với các agent nhiều khả năng miễn phí; bước trả phí sẽ hỏi trước.',
  'Starts with a capable agent and keeps cheaper ones as fallbacks.':
    'Bắt đầu với agent đủ mạnh, giữ các agent rẻ hơn làm dự phòng.',
  'Starts with the strongest agent for hard or risky work.': 'Bắt đầu bằng agent mạnh nhất cho việc khó hoặc rủi ro.',
  'Free First': 'Ưu tiên miễn phí',
  'Balanced': 'Cân bằng',
  'Quality': 'Chất lượng',
  'Workspace': 'Workspace',
  'Checks': 'Check',
  'No agent is installed. Install Claude Code, Codex CLI, Gemini CLI or OpenCode (see Agents) to run tasks.':
    'Chưa có agent nào được cài. Cài Claude Code, Codex CLI, Gemini CLI hoặc OpenCode (xem Agents) để chạy tác vụ.',
  'Agents may run commands in their copy, such as tests. Codex always runs in its own sandbox.':
    'Agent được chạy lệnh trong bản sao của nó, ví dụ test. Codex luôn chạy trong sandbox riêng.',
  'Off: agents may read and edit files only. Codex always runs commands in its own sandbox.':
    'Tắt: agent chỉ được đọc và sửa file. Codex luôn chạy lệnh trong sandbox riêng.',
  'Each step uses the agent’s own sign-in, in a fresh copy of the workspace’s current commit. Tier and billing are your classing: Pygg cannot see which plan an agent uses, so its cost is recorded as unknown.':
    'Mỗi bước dùng đăng nhập riêng của agent, trên một bản sao mới của commit hiện tại trong workspace. Tier và cách tính phí là do bạn phân loại: Pygg không thấy agent dùng gói nào, nên chi phí được ghi là chưa rõ.',
  'Start Task': 'Bắt đầu tác vụ',
  'Add…': 'Thêm…',
  'Folder': 'Thư mục',
  'A Git repository. Every attempt works on its own worktree of it, so your files and branch are never touched.':
    'Một repository Git. Mỗi lần thử làm trên worktree riêng của nó, nên file và nhánh của bạn không bao giờ bị động đến.',
  'Add Workspace': 'Thêm workspace',
  'added by you': 'do bạn thêm',
  'found in {file}': 'tìm thấy trong {file}',
  'Add a check': 'Thêm check',
  'Add a check, e.g. pnpm test': 'Thêm check, ví dụ pnpm test',
  'Add': 'Thêm',
  'Look for checks again': 'Tìm check lại',
  'Detect': 'Phát hiện',
  'No check is approved: a finished attempt waits for your review instead of being confirmed.':
    'Chưa duyệt check nào: lần thử xong sẽ chờ bạn xem lại thay vì được xác nhận tự động.',
  'Approved checks run in order on each attempt’s copy, and stop at the first failure. They run the code the agent wrote, on this computer.':
    'Các check đã duyệt chạy lần lượt trên bản sao của từng lần thử và dừng ở lỗi đầu tiên. Chúng chạy code do agent viết, trên máy này.',
  'Agent for step {n}': 'Agent cho bước {n}',
  'Model for step {n}': 'Model cho bước {n}',
  'default model': 'model mặc định',
  'Tier for step {n}': 'Tier cho bước {n}',
  'Billing for step {n}': 'Cách tính phí cho bước {n}',
  'Move up': 'Lên trên',
  'Move down': 'Xuống dưới',
  'Remove step {n}': 'Xoá bước {n}',
  'Add Step': 'Thêm bước',
  'Steps marked paid wait for your approval before they run.': 'Bước đánh dấu trả phí sẽ chờ bạn duyệt trước khi chạy.',
  // Workspaces (WorkspacesView.tsx)
  'Tasks from your IDE add their repository here by themselves. Approve its checks so results can be confirmed.':
    'Task từ IDE tự thêm repository của nó vào đây. Hãy duyệt các check để kết quả được xác nhận.',
  'Add Workspace…': 'Thêm workspace…',
  'No Workspaces Yet': 'Chưa có workspace',
  'Add a Git repository, or start a task from your IDE: its repository is added for you.':
    'Thêm một repository Git, hoặc bắt đầu task từ IDE: repository sẽ được thêm tự động.',
  'Checks to review': 'Có check cần duyệt',
  '1 task': '1 task',
  '{n} tasks': '{n} task',
  'Every step asks before an agent sees this code, free ones too: Pygg cannot yet check where an agent sends it.':
    'Mọi bước, kể cả bước miễn phí, đều hỏi trước khi agent xem code này: Pygg chưa kiểm được agent gửi code đi đâu.',
  'Free and subscription steps run without asking; paid steps ask.': 'Bước miễn phí và subscription chạy không cần hỏi; bước trả phí sẽ hỏi.',
  'Privacy': 'Riêng tư',
  'Standard': 'Tiêu chuẩn',
  'Sensitive': 'Nhạy cảm',
  'Paid steps per day here': 'Bước trả phí mỗi ngày ở đây',
  'On top of the overall limit in Settings. Empty: no limit of its own.': 'Cộng thêm với giới hạn chung trong Settings. Để trống: không có giới hạn riêng.',
  'Browse…': 'Chọn…',
  'Sensitive workspace': 'Workspace nhạy cảm',
  'This step would let the agent read and change this workspace’s code. Pygg cannot yet check where an agent sends it, so it asks first.':
    'Bước này cho agent đọc và sửa code của workspace này. Pygg chưa kiểm được agent gửi code đi đâu, nên hỏi trước.',
  'Allow This Step': 'Cho phép bước này',

  // Engine text (src/lib/engineText.ts, src-tauri/src/messages.rs)
  "Step {0} ({1}) is paid, and today's budget for paid steps is used; it resets at 00:00 UTC. Raise the budget in Settings, or start the task again after the reset.":
    "Bước {0} ({1}) là bước trả phí, mà ngân sách hôm nay cho bước trả phí đã hết; ngân sách đặt lại lúc 00:00 UTC. Hãy nâng ngân sách trong Cài đặt, hoặc chạy lại tác vụ sau giờ đó.",
  "Step {0} ({1}) is marked paid, so it waits for your approval.":
    "Bước {0} ({1}) là bước trả phí, nên chờ bạn duyệt.",
  "Step {0} ({1}) waits for your approval: the workspace is sensitive, so every step asks before an agent sees its code.":
    "Bước {0} ({1}) chờ duyệt: workspace này nhạy cảm, nên mọi bước đều hỏi trước khi agent được xem code.",
  "You stopped it":
    "Bạn đã dừng tác vụ",
  "You declined the step it was waiting on":
    "Bạn đã từ chối bước mà tác vụ đang chờ",
  "{0}. Pygg stopped before trying another step.":
    "{0}. Pygg đã dừng, không thử bước tiếp theo.",
  "Every step failed. Last: {0}":
    "Mọi bước đều thất bại. Lần cuối: {0}",
  "no attempt ran":
    "chưa có lần chạy nào",
  "No checks are approved for this workspace, so nothing confirmed the result. Review the changes yourself.":
    "Workspace này chưa có check nào được duyệt, nên chưa có gì xác nhận kết quả. Hãy tự xem lại các thay đổi.",
  "No checks are approved for this workspace, so nothing confirmed the result. Review the changes yourself. {0}.":
    "Workspace này chưa có check nào được duyệt, nên chưa có gì xác nhận kết quả. Hãy tự xem lại các thay đổi. {0}.",
  "The checks passed, but this attempt changed {0}, which defines them. Review the changes before trusting the result.":
    "Các check đã qua, nhưng lần chạy này sửa {0}, là nơi định nghĩa các check đó. Hãy xem lại thay đổi trước khi tin kết quả.",
  "The checks passed, but this attempt changed {0}, which defines them. Review the changes before trusting the result. {1}.":
    "Các check đã qua, nhưng lần chạy này sửa {0}, là nơi định nghĩa các check đó. Hãy xem lại thay đổi trước khi tin kết quả. {1}.",
  "{0}.":
    "{0}.",
  "You discarded the result":
    "Bạn đã bỏ kết quả",
  "The app closed while this task ran. Retry runs the step again from the same code.":
    "Ứng dụng bị đóng khi tác vụ đang chạy. Chạy lại sẽ chạy lại bước đó từ cùng phiên bản code.",
  "{0} for this step, ${1}, is over your cap of ${2}":
    "{0} cho bước này là ${1}, vượt mức trần ${2} của bạn",
  "Pygg's ledger":
    "Sổ chi của Pygg",
  "{0}'s own estimate":
    "Ước tính của chính {0}",
  "{0} of this step's requests through Pygg had no price (enter the model's price in Connections), so your cap for this step could not be checked":
    "{0} request của bước này qua Pygg chưa có giá (nhập giá model trong Kết nối), nên không kiểm được mức trần của bước",
  "{0} reported no cost, so your cap for this step could not be checked":
    "{0} không báo chi phí, nên không kiểm được mức trần của bước",
  "Skipped: {0} timed out on its last two runs in the past 30 minutes without answering, so Pygg did not wait for it again":
    "Bỏ qua: {0} đã hết giờ mà không trả lời ở hai lần chạy gần nhất trong 30 phút qua, nên Pygg không chờ nữa",
  "You declined this step":
    "Bạn đã từ chối bước này",
  "{0} could not run: {1}":
    "{0} không chạy được: {1}",
  "{0} did not finish: {1}":
    "{0} không chạy xong: {1}",
  "The agent made no changes":
    "Agent không sửa gì",
  "`{0}` {1}":
    "`{0}` {1}",
  "{0}: {1}":
    "{0}: {1}",
  "exit {0}":
    "thoát với mã {0}",
  "exited {0}":
    "thoát với mã {0}",
  "ended abnormally":
    "kết thúc bất thường",
  "timed out":
    "hết giờ",
  "canceled":
    "đã huỷ",
  "could not start: {0}":
    "không khởi động được: {0}",
  "the agent reported an error":
    "agent báo lỗi",
  "unknown agent {0}":
    "không biết agent {0}",
  "Describe the task":
    "Hãy mô tả tác vụ",
  "The workspace must be a Git repository: the agent works on a separate copy of it.":
    "Workspace phải là một Git repository: agent làm việc trên một bản sao riêng của nó.",
  "{0} is not installed, or not on PATH. See {1}":
    "{0} chưa được cài, hoặc không có trong PATH. Xem {1}",
  "{0} is not a Git repository":
    "{0} không phải là Git repository",
  "Git was not found on this computer":
    "Không tìm thấy Git trên máy này",
  "A branch named {0} already exists":
    "Đã có nhánh tên {0}",
  "git {0} failed: {1}":
    "git {0} thất bại: {1}",
  "{0} · {1}":
    "{0} · {1}",
  "Choose the workspace folder":
    "Hãy chọn thư mục workspace",
  "{0} is not a folder":
    "{0} không phải là thư mục",
  "The workspace must be a Git repository: every attempt works on its own worktree, so your files and branch are never touched.":
    "Workspace phải là một Git repository: mỗi lần thử làm trên worktree riêng, nên file và nhánh của bạn không bao giờ bị động đến.",
  "That workspace no longer exists":
    "Workspace đó không còn nữa",
  "That check no longer exists":
    "Check đó không còn nữa",
  "Type the command, e.g. pnpm test":
    "Nhập lệnh, ví dụ pnpm test",
  "A check is one command line of at most 500 characters":
    "Một check là một dòng lệnh, tối đa 500 ký tự",
  "Unknown privacy class {0}":
    "Không biết lớp riêng tư {0}",
  "This workspace has task history, so it stays.":
    "Workspace này có lịch sử tác vụ, nên được giữ lại.",
  "Unknown strategy {0}":
    "Không biết chiến lược {0}",
  "Add at least one step":
    "Hãy thêm ít nhất một bước",
  "A task has at most {0} steps":
    "Một tác vụ có tối đa {0} bước",
  "Unknown agent {0}":
    "Không biết agent {0}",
  "A step's tier is T0 to T4":
    "Tier của một bước là T0 đến T4",
  "Unknown billing {0}":
    "Không biết cách tính phí {0}",
  "Unknown handoff {0}: use none, tail or evidence":
    "Không biết kiểu bàn giao {0}: dùng none, tail hoặc evidence",
  "That task no longer exists":
    "Tác vụ đó không còn nữa",
  "Only a running or waiting task can be stopped":
    "Chỉ dừng được tác vụ đang chạy hoặc đang chờ",
  "This task is not waiting for an approval":
    "Tác vụ này không chờ duyệt",
  "The waiting step is not in the task's policy":
    "Bước đang chờ không có trong chính sách của tác vụ",
  "Set a spending cap for this paid step":
    "Hãy đặt mức trần chi cho bước trả phí này",
  "This step's estimate or your budget changed since you were asked. Check the new estimate and approve again.":
    "Ước tính của bước này hoặc ngân sách của bạn đã đổi từ lúc được hỏi. Hãy xem ước tính mới rồi duyệt lại.",
  "The cap is more than what is left of today's budget for paid steps (${0})":
    "Mức trần lớn hơn phần còn lại của ngân sách hôm nay cho bước trả phí (${0})",
  "Only an interrupted task can be retried":
    "Chỉ chạy lại được tác vụ bị gián đoạn",
  "The task has no base commit":
    "Tác vụ không có commit gốc",
  "This task has no result waiting for you":
    "Tác vụ này không có kết quả nào chờ bạn",
  "The result's worktree is gone":
    "Worktree của kết quả không còn nữa",

  // Added 2026-11-25: the rest of the task, plan and board screens, so the whole interface reads in Vietnamese.
  'Work': 'Công việc',
  'Project settings': 'Cài đặt dự án',
  'Project': 'Dự án',
  'Plan': 'Kế hoạch',
  'New work': 'Việc mới',
  'facts as of {date}{stale}': 'dữ kiện tính đến {date}{stale}',
  'could not refresh': 'không làm mới được',
  'no facts yet; ranked by tier': 'chưa có dữ kiện; xếp theo tier',
  'ranked by tier; facts off': 'xếp theo tier; dữ kiện đang tắt',
  'Models for {role}': 'Model cho {role}',
  'Turn facts on': 'Bật dữ kiện',
  'Refresh facts': 'Làm mới dữ kiện',
  'Find a model': 'Tìm model',
  'Which models': 'Những model nào',
  'Eligible under my policy': 'Đủ điều kiện theo chính sách',
  'No model matches “{q}”.': 'Không có model nào khớp “{q}”.',
  'Nothing eligible for {role} under this client’s policy: connect a provider or loosen the policy. All connections shows why each is out.':
    'Không có gì đủ điều kiện cho {role} theo chính sách của client này: hãy kết nối một nhà cung cấp hoặc nới chính sách. Mọi kết nối cho biết lý do từng model bị loại.',
  'No connected model is enabled.': 'Chưa bật model nào đã kết nối.',
  'per run': 'mỗi lần chạy',
  'nothing a request': 'không tốn gì mỗi request',
  'a typical turn': 'một lượt thông thường',
  'price unknown': 'chưa rõ giá',
  'current': 'hiện tại',
  'your saved pick': 'lựa chọn đã lưu của bạn',
  'Not eligible: {reason}': 'Không đủ điều kiện: {reason}',
  'Not in this client’s route': 'Không nằm trong tuyến của client này',
  'Why {model}?': 'Vì sao chọn {model}?',
  'Why this model?': 'Vì sao chọn model này?',
  'Use': 'Dùng',
  '{n} connected models are not eligible under this client’s policy; All connections lists them with the reason.':
    '{n} model đã kết nối không đủ điều kiện theo chính sách của client này; Mọi kết nối liệt kê chúng kèm lý do.',
  'saved on Brain Storming; wins over everything below': 'đã lưu ở Brain Storming; ưu tiên hơn mọi thứ bên dưới',
  'none for this role': 'không có cho vai trò này',
  'your runs (10+ attempts)': 'lần chạy của bạn (10+ lần thử)',
  '{kept}/{n} kept · {passed}/{n} checks passed{ranks}': '{kept}/{n} được giữ · {passed}/{n} qua check{ranks}',
  'under ten attempts, not ranking yet': 'dưới mười lần thử, chưa xếp hạng',
  'none yet': 'chưa có',
  'facts feed': 'nguồn dữ kiện',
  'facts off': 'dữ kiện đang tắt',
  'no fact for this role': 'không có dữ kiện cho vai trò này',
  'tier': 'tier',
  '{tier} from {source}': '{tier} theo {source}',
  'Pygg’s table': 'bảng của Pygg',
  'wins': 'quyết định',
  'run with {agent}': 'chạy bằng {agent}',
  '{n} votes': '{n} phiếu',
  'older than a year: shown, not ranking': 'cũ hơn một năm: chỉ hiển thị, không dùng để xếp hạng',
  'No public fact names this model for this role.': 'Không có dữ kiện công khai nào nêu model này cho vai trò này.',
  'Restore': 'Khôi phục',
  'Ask': 'Hỏi',
  'A question about the code. An agent reads and answers; nothing changes.':
    'Một câu hỏi về code. Agent đọc và trả lời; không có gì bị thay đổi.',
  'One change. An agent makes it on a copy, the checks test it, you keep it as a branch.':
    'Một thay đổi. Agent làm trên bản sao, check kiểm tra nó, bạn giữ lại thành một nhánh.',
  'A larger change. Brain Storming splits it into cards you review and approve, then runs them.':
    'Một thay đổi lớn hơn. Brain Storming chia nó thành các thẻ để bạn xem và duyệt, rồi chạy chúng.',
  'What do you want?': 'Bạn muốn làm gì?',
  'Projects could not be loaded: {error}': 'Không tải được danh sách dự án: {error}',
  'Question': 'Câu hỏi',
  'What should be built?': 'Cần xây gì?',
  'Ask about the code, e.g. “How does the app start, and where is the database opened?”':
    'Hỏi về code, ví dụ “Ứng dụng khởi động thế nào, và cơ sở dữ liệu được mở ở đâu?”',
  'Describe the feature or change as you would to a colleague: what it does, for whom, what must not break.':
    'Mô tả tính năng hoặc thay đổi như khi nói với đồng nghiệp: nó làm gì, cho ai, điều gì không được hỏng.',
  'Pygg’s planner reads only what you write here, not your files. It may ask questions, then proposes cards with a role and a model each; nothing runs until you approve. A paid planner waits for your pick.':
    'Planner của Pygg chỉ đọc những gì bạn viết ở đây, không đọc file của bạn. Nó có thể đặt câu hỏi, rồi đề xuất các thẻ, mỗi thẻ có một vai trò và một model; không có gì chạy cho đến khi bạn duyệt. Planner trả phí sẽ chờ bạn chọn.',
  'Agent settings': 'Cài đặt agent',
  'no agent installed': 'chưa cài agent',
  'Web access': 'Truy cập web',
  'Agents may fetch pages and search the web while they work.': 'Agent được tải trang và tìm kiếm web khi làm việc.',
  'Off: agents work from the repository alone.': 'Tắt: agent chỉ làm việc với repository.',
  'Each step uses the agent’s own sign-in, in a fresh copy of the project’s current commit. If a step fails, the next one tries. Tier and billing are your classing: Pygg cannot see which plan an agent uses.':
    'Mỗi bước dùng đăng nhập riêng của agent, trên một bản sao mới của commit hiện tại trong dự án. Nếu một bước thất bại, bước kế tiếp sẽ thử. Tier và cách tính phí là do bạn phân loại: Pygg không thấy agent dùng gói nào.',
  'Start task': 'Bắt đầu tác vụ',
  'Create plan': 'Tạo kế hoạch',
  'free': 'miễn phí',
  'your plan’s quota': 'quota của gói bạn',
  'unknown: never run': 'chưa rõ: chưa chạy lần nào',
  '≈{usd} ({n} runs)': '≈{usd} ({n} lần chạy)',
  '{low}–{high} ({n} runs)': '{low}–{high} ({n} lần chạy)',
  'unknown': 'chưa rõ',
  'updating…': 'đang cập nhật…',
  'Estimating…': 'Đang ước tính…',
  'If the first step succeeds: {cost}': 'Nếu bước đầu thành công: {cost}',
  'If every step runs: the steps with a history cost up to {cost} on past runs, plus {n} never run, unknown':
    'Nếu chạy mọi bước: các bước đã có lịch sử tốn tối đa {cost} theo các lần chạy trước, cộng {n} bước chưa chạy lần nào, chưa rõ',
  'If every step runs: up to {cost} on past runs': 'Nếu chạy mọi bước: tối đa {cost} theo các lần chạy trước',
  'The next step starts only when a step fails: the checks fail, it changes nothing, or the agent stops. A paid step asks you first. Estimates are what the same agent and model cost on earlier attempts, not a quote or a ceiling.':
    'Bước kế tiếp chỉ bắt đầu khi một bước thất bại: check thất bại, bước đó không sửa gì, hoặc agent dừng. Bước trả phí sẽ hỏi bạn trước. Ước tính là chi phí của cùng agent và model ở các lần thử trước, không phải báo giá hay mức trần.',
  'Stop the task at': 'Dừng tác vụ khi đạt',
  'Optional. No further step starts once the task has cost this much, or once a paid step’s cost is unknown; a paid step’s cap is never above what is left. Only Claude Code stops itself mid-run at a cap: another agent’s step may go over, and the task stops after it.':
    'Không bắt buộc. Không bước nào bắt đầu thêm khi tác vụ đã tốn đến mức này, hoặc khi chưa rõ chi phí của một bước trả phí; mức chi của bước trả phí không bao giờ vượt phần còn lại. Chỉ Claude Code tự dừng giữa chừng khi chạm mức chi: bước của agent khác có thể vượt, và tác vụ dừng sau bước đó.',
  'no cap': 'không giới hạn',
  'Safety': 'An toàn',
  'Writes outside its copy': 'Ghi ngoài bản sao',
  'Reads outside': 'Đọc bên ngoài',
  'Network': 'Mạng',
  'Secrets in the environment': 'Bí mật trong môi trường',
  'Every attempt starts in its own copy of the repository. Where writes outside the copy are not blocked, other files, your checkout among them, can be changed. Variables in Pygg’s environment that look like secrets and that the agent’s own sign-in does not use (cloud, GitHub, database keys, URLs with a password) are not passed to it; secrets in files and credential stores are not covered.':
    'Mỗi lần thử bắt đầu trên bản sao repository riêng. Nơi không chặn việc ghi ra ngoài bản sao, các file khác, kể cả checkout của bạn, có thể bị sửa. Biến môi trường của Pygg trông giống bí mật mà đăng nhập riêng của agent không dùng (key cloud, GitHub, cơ sở dữ liệu, URL có mật khẩu) sẽ không được truyền cho nó; bí mật trong file và kho thông tin đăng nhập thì không được che chắn.',
  'Not guaranteed means Pygg cannot hold that agent to it here: an allowed shell reaches past tool rules. Keep secrets out of the repository, or choose another agent.':
    'Không đảm bảo nghĩa là Pygg không thể buộc agent đó tuân theo ở đây: một shell được phép có thể vượt qua luật của tool. Hãy giữ bí mật ngoài repository, hoặc chọn agent khác.',
  '{name}: what differs': '{name}: điểm khác biệt',
  'Only what is set here differs from Brain Storming; everything else follows it.':
    'Chỉ những gì đặt ở đây khác với Brain Storming; mọi thứ khác theo Brain Storming.',
  'Default': 'Mặc định',
  'by the rules': 'theo luật',
  'This project': 'Dự án này',
  'the default': 'mặc định',
  'Override the {role} model for {name}': 'Ghi đè model {role} cho {name}',
  'Change…': 'Đổi…',
  'Override…': 'Ghi đè…',
  'Use the default {role} model for {name}': 'Dùng model {role} mặc định cho {name}',
  'Use default': 'Dùng mặc định',
  'WIP limits': 'Hạn mức WIP',
  'Default: Running {r} · Checking {c}': 'Mặc định: Đang chạy {r} · Đang chạy check {c}',
  'Own limits for this project': 'Hạn mức riêng cho dự án này',
  'Running limit for {name}': 'Giới hạn cột Đang chạy cho {name}',
  'Checking limit for {name}': 'Giới hạn cột Đang chạy check cho {name}',
  'Save limits': 'Lưu hạn mức',
  'Search work': 'Tìm công việc',
  'Nothing matches “{query}”': 'Không có gì khớp “{query}”',
  'No work yet': 'Chưa có việc nào',
  'Ask about a project, give it one task, or plan a larger change with Brain Storming.':
    'Hỏi về một dự án, giao một tác vụ, hoặc lập kế hoạch thay đổi lớn hơn với Brain Storming.',
  'Needs you · {n}': 'Cần bạn xử lý · {n}',
  'Running · {n}': 'Đang chạy · {n}',
  'Recent': 'Gần đây',
  'Choose a planner to start': 'Chọn planner để bắt đầu',
  'The planner is working': 'Planner đang làm việc',
  'The planner failed': 'Planner thất bại',
  'Proposed · {n} cards to review': 'Đã đề xuất · {n} thẻ cần xem',
  '{done} of {total} cards done · {n} waiting for you': '{done}/{total} thẻ đã xong · {n} chờ bạn',
  'Every card done · output ready': 'Mọi thẻ đã xong · đầu ra sẵn sàng',
  '{done} of {total} cards done': '{done}/{total} thẻ đã xong',
  'from your coding agent': 'từ agent lập trình của bạn',
  'A project is one of your Git repositories. Pygg works on copies of it and never changes your checkout.':
    'Một dự án là một repository Git của bạn. Pygg làm việc trên các bản sao của nó và không bao giờ sửa checkout của bạn.',
  'Add project…': 'Thêm dự án…',
  'Add a Git repository to ask about it, give it a task, or plan a change. A task your coding agent starts adds its repository by itself.':
    'Thêm một repository Git để hỏi về nó, giao một tác vụ, hoặc lập kế hoạch thay đổi. Tác vụ do agent lập trình của bạn khởi chạy sẽ tự thêm repository của nó.',
  '{n} waiting for you': '{n} chờ bạn',
  'no work yet': 'chưa có việc',
  'nothing running': 'không có gì đang chạy',
  'Open project {name}': 'Mở dự án {name}',
  'Answered · {time}': 'Đã trả lời · {time}',
  'Set up once; every project uses it unless the project overrides it below. The LLM writes the cards; these picks and the rules choose the models; you approve before money is spent.':
    'Thiết lập một lần; mọi dự án dùng nó trừ khi dự án ghi đè ở dưới. LLM viết các thẻ; các lựa chọn này và luật chọn model; bạn duyệt trước khi tiền được chi.',
  'Models by role': 'Model theo vai trò',
  'A saved pick wins over your runs, the facts feed and the tier. Clear it to let the rules decide.':
    'Lựa chọn đã lưu được ưu tiên hơn các lần chạy của bạn, nguồn dữ kiện và tier. Xoá nó để luật quyết định.',
  'Defaults for a new brainstorm': 'Mặc định cho brainstorm mới',
  'Runner': 'Bên chạy',
  'Who runs a card unless the card says otherwise': 'Ai chạy thẻ, trừ khi thẻ ghi khác',
  'Pygg engine': 'Bộ máy Pygg',
  'My IDE agent': 'Agent trong IDE của tôi',
  'WIP limit · Running': 'Hạn mức WIP · Đang chạy',
  'Paid steps in flight at once; never below 1': 'Số bước trả phí chạy cùng lúc; không dưới 1',
  'WIP limit · Checking': 'Hạn mức WIP · Đang chạy check',
  'Runs whose agent is done wait before their checks while Checking is full':
    'Khi Đang chạy check đầy, lần chạy có agent đã xong phải chờ mới được chạy check',
  'Remember a planner picked in a brainstorm': 'Ghi nhớ planner đã chọn trong brainstorm',
  'Saves it as the plan role above': 'Lưu làm vai trò plan ở trên',
  'Changing a card’s model': 'Đổi model của thẻ',
  'What Change… offers first': 'Nút Đổi… đề xuất gì trước',
  'Ask: this card or all of its role': 'Hỏi: thẻ này hay mọi thẻ cùng vai trò',
  'This card only': 'Chỉ thẻ này',
  'All cards of its role': 'Mọi thẻ cùng vai trò',
  'Facts and prompt': 'Thông tin và prompt',
  'Model facts': 'Dữ kiện model',
  'Role ranks from public benchmarks (ADR-040) · built {built}{fetched}{stale}{err}':
    'Xếp hạng vai trò từ benchmark công khai (ADR-040) · tạo {built}{fetched}{stale}{err}',
  'fetched {ago}': 'tải về {ago}',
  'older than the weekly cadence': 'cũ hơn chu kỳ hằng tuần',
  'last fetch failed: {e}': 'lần tải gần nhất lỗi: {e}',
  'No feed kept yet; roles rank by tier{err}': 'Chưa lưu nguồn dữ kiện nào; các vai trò xếp hạng theo tier{err}',
  'Refresh': 'Làm mới',
  'Sources': 'Nguồn',
  'Each with its licence; a fact older than a year is shown, never ranked':
    'Mỗi nguồn kèm giấy phép; dữ kiện cũ hơn một năm chỉ hiển thị, không dùng để xếp hạng',
  'none kept': 'chưa giữ gì',
  'rows': 'dòng',
  'carried forward': 'giữ từ lần trước',
  'Brain prompt': 'Prompt của Brain',
  'What the planner is asked; a prompt in Skills & Prompts': 'Điều planner được hỏi; một prompt trong Skill & Prompt',
  'Projects that differ': 'Dự án có thiết lập riêng',
  '{n} workspaces': '{n} dự án',
  'Running {r} · Checking {c}': 'Đang chạy {r} · Đang chạy check {c}',
  'Uses the defaults above': 'Dùng mặc định ở trên',
  '{action} for {name}': '{action} cho {name}',
  'Use the defaults for {name}': 'Dùng mặc định cho {name}',
  'Use defaults': 'Dùng mặc định',
  'No workspace yet; a brainstorm from an IDE adds its repository by itself.':
    'Chưa có dự án; brainstorm từ IDE sẽ tự thêm repository của nó.',
  'An override holds only what differs: a pick for a role, or the WIP limits. The board’s Limit… sets the same limits.':
    'Phần ghi đè chỉ chứa điểm khác biệt: model chọn cho một vai trò, hoặc hạn mức WIP. Nút Hạn mức… trên bảng đặt cùng các hạn mức này.',
  'How it starts': 'Cách bắt đầu',
  'Start a brainstorm from your coding agent': 'Bắt đầu brainstorm từ agent lập trình',
  'There is no text box here. Start the agent through Pygg on the model pygg/brain (set up on CLI Tools) and type the requirement; the proposal shows up under Tasks.':
    'Ở đây không có ô nhập. Khởi động agent qua Pygg với model pygg/brain (cài ở Công cụ CLI) rồi nhập yêu cầu; đề xuất sẽ hiện trong Công việc.',
  'Start a brainstorm': 'Bắt đầu brainstorm',
  'Your saved pick': 'Lựa chọn bạn đã lưu',
  'By the rules today (no saved pick)': 'Theo luật hiện tại (chưa lưu lựa chọn)',
  'nothing eligible: connect a provider or loosen the policy':
    'không có model đủ điều kiện: hãy kết nối nhà cung cấp hoặc nới chính sách',
  'No longer eligible: not connected, turned off, or below the policy. Pick again or clear.':
    'Không còn đủ điều kiện: chưa kết nối, đã tắt, hoặc dưới mức chính sách. Hãy chọn lại hoặc xoá.',
  'Change the {role} model': 'Đổi model {role}',
  'Clear': 'Xoá',
  'Activity': 'Hoạt động',
  'Now': 'Hiện tại',
  'Total: {cost}': 'Tổng: {cost}',
  'cap {usd}': 'mức chi {usd}',
  'for one kept result': 'cho một kết quả được giữ',
  'for one answer': 'cho một câu trả lời',
  'Every step in full': 'Mọi bước đầy đủ',
  'Allowed': 'Được phép',
  'shell': 'shell',
  'web': 'web',
  'editing files in its copy only': 'chỉ sửa file trong bản sao của nó',
  'Every step, in full': 'Mọi bước, đầy đủ',
  'A finished step keeps the end of its agent’s output; the live lines last while the app runs.':
    'Bước đã xong giữ lại phần cuối output của agent; các dòng trực tiếp chỉ còn khi ứng dụng đang chạy.',
  'This result was kept on its plan’s branch, which holds the other cards too: see the plan’s Output.':
    'Kết quả này được giữ trên nhánh của kế hoạch, nơi có cả các thẻ khác: xem Đầu ra của kế hoạch.',
  'Nothing to show: the result was discarded, or the copy it was made in is gone.':
    'Không có gì để hiển thị: kết quả đã bị bỏ, hoặc bản sao tạo ra nó không còn nữa.',
  'Show less': 'Thu gọn',
  'Show the whole task': 'Hiện toàn bộ tác vụ',
  'This card’s work is on its plan’s branch, with the cards before it.':
    'Kết quả của thẻ này nằm trên nhánh của kế hoạch, cùng các thẻ trước nó.',
  'This card ran beside another and did not apply cleanly on its plan’s branch, so it is on a branch of its own: merge it by hand.':
    'Thẻ này chạy song song với thẻ khác và không áp dụng gọn vào nhánh của kế hoạch, nên nằm ở nhánh riêng: hãy merge thủ công.',
  'Kept as a branch. Your checkout was not changed.': 'Đã giữ thành một nhánh. Checkout của bạn không bị thay đổi.',
  'Review the branch, then merge it into yours when you are happy. Pygg never merges for you.':
    'Xem lại nhánh, rồi merge vào nhánh của bạn khi đã ưng ý. Pygg không bao giờ merge thay bạn.',
  'This is a card of a plan. To try it again, after raising a limit or changing its models, use Run again on the card in the project’s board.':
    'Đây là một thẻ của kế hoạch. Để thử lại, sau khi nâng hạn mức hoặc đổi model, hãy dùng Chạy lại trên thẻ ở bảng của dự án.',
  'Pygg refuses any request of this step that could take its spend past this amount.':
    'Pygg từ chối mọi request của bước này có thể làm chi phí vượt mức này.',
  'Live activity': 'Hoạt động trực tiếp',
  '{n} earlier lines not kept': '{n} dòng trước đó không được giữ',
  'Waiting for the agent’s first output…': 'Đang chờ output đầu tiên của agent…',
  'This agent reports what it did when it finishes; its output appears below then.':
    'Agent này báo những gì nó đã làm khi xong; output của nó sẽ hiện bên dưới lúc đó.',
  'Answer': 'Câu trả lời',
  'The agent left no text.': 'Agent không để lại văn bản nào.',
  'quota': 'quota',
  'Nothing checked this result: the project has no approved checks.':
    'Chưa có gì check kết quả này: dự án chưa có check nào được duyệt.',
  'The checks test only what their commands run; behaviour they do not cover is unchecked.':
    'Check chỉ kiểm những gì lệnh của chúng chạy; hành vi ngoài phạm vi đó chưa được check.',
  'No test file changed in this result.': 'Không có file test nào thay đổi trong kết quả này.',
  'The card’s done criterion, to confirm yourself: {done}': 'Tiêu chí hoàn thành của thẻ, bạn tự xác nhận: {done}',
  'Evidence': 'Bằng chứng',
  'Asked': 'Đã hỏi',
  '{n} test files': '{n} file test',
  'Files': 'File',
  'Checked by': 'Được check bởi',
  'no check ran': 'chưa chạy check nào',
  'Not checked': 'Chưa được check',
  'Could not copy: select the text and copy it yourself.': 'Không sao chép được: hãy tự chọn và sao chép văn bản.',
  'Brief copied.': 'Đã sao chép bản bàn giao.',
  'Brief for another agent…': 'Bàn giao cho agent khác…',
  'Brief for another agent': 'Bàn giao cho agent khác',
  'Read it before sharing it. Key-shaped text and passwords in URLs are masked; the task’s own words and file names go as written.':
    'Hãy đọc trước khi chia sẻ. Đoạn trông giống key và mật khẩu trong URL đã được che; nội dung tác vụ và tên file giữ nguyên.',
  'Loading the board…': 'Đang tải bảng…',
  '{lane} limit: Enter saves, Escape keeps {n}': 'Hạn mức {lane}: Enter để lưu, Escape để giữ {n}',
  'Showing the cards of the plan “{title}”. The running limit counts every plan’s cards.':
    'Đang hiện các thẻ của kế hoạch “{title}”. Hạn mức đang chạy tính thẻ của mọi kế hoạch.',
  'Show every card': 'Hiện mọi thẻ',
  'Running {r}/{l} · Blocked {b} · Ready {y} · Review {v} · Done {d}':
    'Đang chạy {r}/{l} · Bị chặn {b} · Sẵn sàng {y} · Xem xét {v} · Xong {d}',
  'Measures': 'Chỉ số',
  'cycle time median {c} · {n} done today · blocked today {b} · known money per kept result {money} · from the cards’ times and their tasks’ ledger':
    'trung vị thời gian chu trình {c} · hôm nay xong {n} · bị chặn hôm nay {b} · chi phí đã biết cho mỗi kết quả giữ lại {money} · theo thời gian của thẻ và sổ chi của tác vụ',
  '{t} ({k} of {n} cards timed)': '{t} (đo được {k}/{n} thẻ)',
  '{n} unpriced': '{n} chưa có giá',
  'measures appear after 5 cards are done ({n} so far)': 'chỉ số hiện sau khi xong 5 thẻ (đến giờ {n})',
  'Use the default limits': 'Dùng hạn mức mặc định',
  'Gateway failed: {error}': 'Gateway lỗi: {error}',
  'Gateway stopped': 'Gateway đã dừng',
  'cards run through it, so Ready waits': 'thẻ chạy qua gateway, nên Sẵn sàng phải chờ',
  'Start': 'Bật',
  'No cards yet. Start a plan with New plan on the project page (or from your coding agent with the model pygg/brain), then approve it.':
    'Chưa có thẻ nào. Bắt đầu một kế hoạch bằng Việc mới → Kế hoạch ở trang dự án (hoặc từ agent lập trình với model pygg/brain), rồi duyệt nó.',
  'Nothing here': 'Trống',
  'Drop “{title}”?': 'Bỏ “{title}”?',
  'Drop': 'Bỏ',
  'The card leaves the board and never runs. Cards that depend on it will block, naming it.':
    'Thẻ rời khỏi bảng và không bao giờ chạy. Các thẻ phụ thuộc vào nó sẽ bị chặn, kèm tên thẻ này.',
  'Expedite': 'Ưu tiên gấp',
  ', then {n} more': ', rồi thêm {n}',
  'Your coding agent does this card; Pygg does not see its progress yet. Hand it to Pygg to run it here instead.':
    'Agent lập trình của bạn làm thẻ này; Pygg chưa thấy tiến độ của nó. Giao cho Pygg để chạy ở đây.',
  'blocked {time}': 'bị chặn {time}',
  'waits for your approval': 'chờ bạn duyệt',
  '{phase} · {time}': '{phase} · {time}',
  'running': 'đang chạy',
  'held by you': 'bạn đang giữ lại',
  'waits for {deps}': 'chờ {deps}',
  'next in line': 'kế tiếp',
  'checks passed': 'check đã qua',
  'finished': 'đã xong',
  '{action}: {title}': '{action}: {title}',
  'Approve…': 'Duyệt…',
  'Back to Discovery': 'Về Khám phá',
  'Move to Ready': 'Chuyển sang Sẵn sàng',
  'Drop {title}': 'Bỏ {title}',
  'Hand to Pygg': 'Giao cho Pygg',
  'Run again: {title}': 'Chạy lại: {title}',
  'Run again': 'Chạy lại',
  'started here': 'bắt đầu ở đây',
  'from a coding agent': 'từ agent lập trình',
  'Planner': 'Planner',
  'no planner yet': 'chưa có planner',
  'The planner is reading the requirement…': 'Planner đang đọc yêu cầu…',
  'The planner failed.': 'Planner thất bại.',
  'What the planner answered': 'Planner đã trả lời gì',
  'Declined. Nothing was started.': 'Đã từ chối. Chưa có gì được chạy.',
  'The plan as approved: {q} questions, {c} cards': 'Kế hoạch đã duyệt: {q} câu hỏi, {c} thẻ',
  'Retry with {model}': 'Thử lại với {model}',
  'or pick another planner below': 'hoặc chọn planner khác bên dưới',
  'Waits for your pick': 'Chờ bạn chọn',
  'Proposed · revision {n}': 'Đã đề xuất · bản {n}',
  'Approved · board': 'Đã duyệt · trên bảng',
  'Declined': 'Đã từ chối',
  'Planner failed': 'Planner thất bại',
  'Pick another planner': 'Chọn planner khác',
  'Pick a planner': 'Chọn planner',
  'The brain needs a model to read your requirement. This is the first paid request Pygg would start on its own, so it waits for you.':
    'Brain cần một model để đọc yêu cầu của bạn. Đây là request trả phí đầu tiên Pygg tự khởi chạy, nên nó chờ bạn.',
  'No connected model is eligible as a planner under this client’s policy.':
    'Không model đã kết nối nào đủ điều kiện làm planner theo chính sách của client này.',
  'per brainstorm': 'mỗi brainstorm',
  'the rule’s pick': 'lựa chọn theo luật',
  'Why these models? Every model for plan, with its evidence…':
    'Vì sao là các model này? Mọi model cho plan, kèm bằng chứng…',
  'Remember this planner for every project': 'Ghi nhớ planner này cho mọi dự án',
  'The candidates are the plan role’s ranking: your runs, then the model facts feed, then the tier. Each line says which.':
    'Các ứng viên là bảng xếp hạng của vai trò plan: các lần chạy của bạn, rồi nguồn dữ kiện model, rồi tier. Mỗi dòng ghi rõ theo nguồn nào.',
  'Questions · {n}, {m} answered': 'Câu hỏi · {n}, đã trả lời {m}',
  'Answered in your coding agent, not here': 'Trả lời trong agent lập trình, không phải ở đây',
  'The planner asked nothing.': 'Planner không hỏi gì.',
  'Answer to {q}': 'Trả lời {q}',
  'Your answer': 'Câu trả lời của bạn',
  'answered': 'đã trả lời',
  'open': 'chưa trả lời',
  'Anything else to change in the plan': 'Điều gì khác cần đổi trong kế hoạch',
  'For example: one card for the API, another for the page; leave out the export.':
    'Ví dụ: một thẻ cho API, một thẻ cho trang; bỏ phần export.',
  'Sending…': 'Đang gửi…',
  'Send to the planner': 'Gửi cho planner',
  'The planner writes a new revision; open questions you leave blank stay open.':
    'Planner sẽ viết bản sửa đổi mới; câu hỏi bạn để trống vẫn để mở.',
  'Cards · {kept} kept of {total}': 'Thẻ · giữ {kept}/{total}',
  '{n} more rejected by the schema': '{n} thẻ khác bị schema loại',
  'Models picked by Pygg’s rules; the planner wrote the cards': 'Model do luật của Pygg chọn; planner viết các thẻ',
  'Wave 1 · depends on nothing': 'Đợt 1 · không phụ thuộc gì',
  'Wave {n} · after wave {m}': 'Đợt {n} · sau đợt {m}',
  'needs {q} answered': 'cần trả lời {q}',
  'nothing eligible': 'không model nào đủ điều kiện',
  'Apply {model} to': 'Áp dụng {model} cho',
  'This card': 'Thẻ này',
  'All {role} cards': 'Mọi thẻ {role}',
  'unpriced': 'chưa có giá',
  'Fallbacks': 'Dự phòng',
  'then': 'rồi',
  'Remove fallback {model}': 'Gỡ dự phòng {model}',
  'Add a fallback to {title}': 'Thêm model dự phòng cho {title}',
  'Add a fallback…': 'Thêm dự phòng…',
  'tried in order when an attempt fails: a connection error, a spent quota, a failed check':
    'thử lần lượt khi một lần thử thất bại: lỗi kết nối, hết quota, check thất bại',
  'Details': 'Chi tiết',
  'Done when': 'Xong khi',
  'Depends on': 'Phụ thuộc',
  'Pygg engine (default)': 'Bộ máy Pygg (mặc định)',
  'Class': 'Loại',
  'Restore {title}': 'Khôi phục {title}',
  'Change the model of {title}': 'Đổi model của {title}',
  'Estimate: Unknown (no card is priced)': 'Ước tính: Chưa rõ (chưa thẻ nào có giá)',
  'Known estimate ≈{usd} for one run of every card': 'Ước tính đã biết ≈{usd} cho một lần chạy mọi thẻ',
  'unpriced cards: {n}': 'thẻ chưa có giá: {n}',
  'no daily task limit': 'không có hạn mức tác vụ theo ngày',
  'today’s task budget left {usd}': 'ngân sách tác vụ hôm nay còn {usd}',
  'Running limit {n}': 'Giới hạn Đang chạy: {n}',
  '{n} cards on the board; the first ready ones are being pulled into Running':
    '{n} thẻ trên bảng; các thẻ sẵn sàng đầu tiên đang được kéo vào Đang chạy',
  '{n} cards': '{n} thẻ',
  '{n} without an eligible model': '{n} thẻ không có model đủ điều kiện',
  'Choose a project…': 'Chọn một dự án…',
  'Decline brainstorm': 'Từ chối brainstorm',
  'Approve {n} cards…': 'Duyệt {n} thẻ…',
  'Only OpenCode through Pygg runs a card on the model Pygg picked. {agent} runs with its own sign-in and model; the pick is told to it.':
    'Chỉ OpenCode qua Pygg mới chạy thẻ trên model Pygg đã chọn. {agent} chạy bằng đăng nhập và model riêng; lựa chọn chỉ được báo cho nó.',
  'No agent is installed; install one on Agents to run the cards.':
    'Chưa cài agent nào; cài một agent ở mục Agent & mặc định để chạy các thẻ.',
  'No project yet: add the repository under Projects first, then approve here.':
    'Chưa có dự án: thêm repository ở mục Dự án trước, rồi duyệt ở đây.',
  'Choose the project (repository) this plan is for; its cards run there.':
    'Chọn dự án (repository) cho kế hoạch này; các thẻ của nó chạy ở đó.',
  'Approve {n} cards?': 'Duyệt {n} thẻ?',
  'Approve': 'Duyệt',
  'Each card becomes a card on the board of {workspace}; when pulled, {agent} runs it on the model shown, then its fallbacks, judged by the workspace’s approved checks.':
    'Mỗi thẻ thành một thẻ trên bảng của {workspace}; khi được kéo vào chạy, {agent} chạy nó bằng model hiển thị, rồi tới các model dự phòng, và được đánh giá bằng các check đã duyệt của workspace.',
  'Each card becomes a card on the board of {workspace}; when pulled, {agent} runs it with its own sign-in and model (the pick is told to it), judged by the workspace’s approved checks.':
    'Mỗi thẻ thành một thẻ trên bảng của {workspace}; khi được kéo vào chạy, {agent} chạy nó bằng đăng nhập và model riêng (model đã chọn được báo cho agent), và được đánh giá bằng các check đã duyệt của workspace.',
  'A paid step still waits for your approval when it starts.': 'Bước trả phí vẫn chờ bạn duyệt khi bắt đầu.',
  'Choose a planner': 'Chọn planner',
  'The planner failed: retry or pick another': 'Planner thất bại: thử lại hoặc chọn planner khác',
  'Answer, adjust, then approve': 'Trả lời, chỉnh sửa, rồi duyệt',
  'Approved · {n} cards': 'Đã duyệt · {n} thẻ',
  'After you approve the plan': 'Sau khi bạn duyệt kế hoạch',
  'Loading the cards…': 'Đang tải thẻ…',
  '{done} of {total} done': '{done}/{total} đã xong',
  'One branch to review and merge': 'Một nhánh để xem lại và merge',
  'When every card is done': 'Khi mọi thẻ đã xong',
  'Where this plan is': 'Kế hoạch đang ở bước nào',
  'Cards · {done} of {total} done': 'Thẻ · {done}/{total} đã xong',
  'View board': 'Xem bảng',
  'The board runs the cards within its running limit ({n} at a time), each on the plan’s branch and checked before the next starts. A paid step waits for your approval.':
    'Bảng chạy các thẻ trong hạn mức đang chạy ({n} thẻ cùng lúc), mỗi thẻ trên nhánh của kế hoạch và được check trước khi thẻ tiếp theo bắt đầu. Bước trả phí chờ bạn duyệt.',
  'in progress': 'đang diễn ra',
  'A paid step waits for your approval': 'Bước trả phí đang chờ bạn duyệt',
  'Your coding agent does this card; Pygg does not see its progress':
    'Agent lập trình của bạn làm thẻ này; Pygg không thấy tiến độ',
  'Hand {title} to Pygg': 'Giao {title} cho Pygg',
  'Move {title} to Ready': 'Chuyển {title} sang Sẵn sàng',
  'Every card is done. The plan’s branch has all but {n} of them:':
    'Mọi thẻ đã xong. Nhánh của kế hoạch có tất cả trừ {n} thẻ:',
  'Every card is done. The work is on one branch:': 'Mọi thẻ đã xong. Kết quả nằm trên một nhánh:',
  '{done} of {total} cards done so far; the plan’s branch:': '{done}/{total} thẻ đã xong; nhánh của kế hoạch:',
  'Review it, then merge it into your branch when you are happy:': 'Xem lại, rồi merge vào nhánh của bạn khi đã ưng ý:',
  '{n} results on branches of their own:': '{n} kết quả ở nhánh riêng:',
  'their work could not be added to the plan’s branch (it conflicted with a card that ran beside it, or the branch was checked out). Review each and merge it by hand.':
    'không thêm được vào nhánh của kế hoạch (bị xung đột với một thẻ chạy song song, hoặc nhánh đang được checkout). Hãy xem lại từng nhánh và merge thủ công.',
  'Copy {text}': 'Sao chép {text}',
  'Hide the changes': 'Ẩn thay đổi',
  'Review the changes': 'Xem thay đổi',
  'No changes against your branch: it may be merged already.':
    'Không có thay đổi so với nhánh của bạn: có thể đã được merge.',
  'Board': 'Bảng',
  'The board is empty': 'Bảng đang trống',
  'An approved plan puts its cards here. Pygg runs them one at a time, each checked before the next starts.':
    'Kế hoạch đã duyệt sẽ đặt các thẻ ở đây. Pygg chạy lần lượt từng thẻ, mỗi thẻ được check trước khi thẻ sau bắt đầu.',
  'Waiting for Checking': 'Chờ chạy check',
  'Discovery': 'Khám phá',
  'Blocked': 'Bị chặn',
  'Discarded': 'Đã bỏ',
  'Dropped': 'Đã bỏ',
  'Waiting for earlier cards': 'Chờ các thẻ trước',
  'Next in line': 'Kế tiếp',
  'Finished: review it': 'Xong: hãy xem xét',
  'allowed by you': 'do bạn cho phép',
  'not guaranteed': 'không đảm bảo',
  'filtered, best effort': 'có lọc, trong khả năng có thể',
  'in its sandbox': 'trong sandbox của nó',
  'Answered. Nothing was changed; the answer is below.': 'Đã trả lời. Không có gì bị thay đổi; câu trả lời ở bên dưới.',
  'The agent finished without an answer': 'Agent kết thúc mà không có câu trả lời',
  'Skipped {0}: its account is {1} ({2}). Fix it under Providers to use this model again.':
    'Đã bỏ qua {0}: tài khoản của nó đang {1} ({2}). Sửa trong Nhà cung cấp để dùng lại model này.',
  'Stopped at this task\'s cap of {0}: it has cost {1} so far. No further step was started.':
    'Đã dừng ở mức chi {0} của tác vụ này: đến nay đã tốn {1}. Không bắt đầu thêm bước nào.',
  'Stopped before a paid step: this task\'s cap of {0} cannot be kept, because what attempt {1} cost is unknown.':
    'Đã dừng trước một bước trả phí: không giữ được mức chi {0} của tác vụ này, vì chưa rõ lần thử {1} đã tốn bao nhiêu.',
  'The cap is more than this task\'s own cap leaves (${0}). Lower it, or raise the task\'s cap.':
    'Mức chi lớn hơn phần còn lại trong mức chi của tác vụ (${0}). Hãy hạ nó xuống, hoặc nâng mức chi của tác vụ.',
};
