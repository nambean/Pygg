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
};
