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
};
