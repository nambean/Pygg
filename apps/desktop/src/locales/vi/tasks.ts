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
  'Local': 'Local',
  'Subscription': 'Subscription',
  'Paid': 'Trả phí',
  'Unknown billing': 'Chưa rõ cách tính phí',
  'Passed': 'Đã qua',
  'Check failed': 'Check thất bại',
  'Waiting for approval': 'Chờ duyệt',
  'API {zero} · local': 'API {zero} · local',
  '{actual} + ≈{estimated} est.': '{actual} + ≈{estimated} ước tính',
  '1 call unknown': '1 lượt gọi chưa rõ',
  '{n} calls unknown': '{n} lượt gọi chưa rõ',

  // Detail
  'Cost so far: {cost}': 'Chi phí đến giờ: {cost}',
  'The plan, live output and result review come with the task engine.':
    'Kế hoạch, output trực tiếp và phần duyệt kết quả sẽ có khi xây xong bộ máy chạy tác vụ.',

  // New task
  'Describe the change, e.g. “Fix timeout handling and add regression tests.”':
    'Mô tả thay đổi, ví dụ “Sửa xử lý timeout và thêm test hồi quy.”',
  'Strategy': 'Chiến lược',
  'Starts with the lowest eligible tier; paid steps ask first.':
    'Bắt đầu ở tier thấp nhất đủ điều kiện; bước trả phí sẽ hỏi trước.',
  'Balances cost, speed and quality.': 'Cân bằng chi phí, tốc độ và chất lượng.',
  'Starts with a strong model for hard or risky work.': 'Bắt đầu bằng model mạnh cho việc khó hoặc rủi ro.',
  'Free First': 'Ưu tiên miễn phí',
  'Balanced': 'Cân bằng',
  'Quality': 'Chất lượng',
  'Workspace': 'Workspace',
  'Choose…': 'Chọn…',
  'From Workspace': 'Từ workspace',
  'Path': 'Đường dẫn',
  'Privacy': 'Riêng tư',
  'Not set yet': 'Chưa đặt',
  'Checks': 'Check',
  'Not detected yet': 'Chưa phát hiện',
  'Privacy, checks and permissions come from Workspace Settings. A task can only tighten them.':
    'Mức riêng tư, check và quyền lấy từ Cài đặt workspace. Một tác vụ chỉ có thể siết chặt hơn.',
  'Needs the task engine': 'Cần bộ máy chạy tác vụ',
  'Analyze Task': 'Phân tích tác vụ',
  'Analysis needs the task engine, which isn’t built yet. Nothing is sent anywhere.':
    'Việc phân tích cần bộ máy chạy tác vụ, phần này chưa được xây. Không có dữ liệu nào được gửi đi.',
};
