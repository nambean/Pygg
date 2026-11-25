/** Vietnamese for the gateway screens: Overview, Connections, Combos, Clients, Usage, and shared controls. */
export const gateway: Record<string, string> = {
  // Shell and navigation
  'Gateway': 'Gateway',
  'Overview': 'Tổng quan',
  'Combos': 'Combo',
  'Clients': 'Client',
  'Smart Tasks': 'Tác vụ thông minh',
  // 'Tasks' is in shell.ts: 'Tác vụ', as in 'Tác vụ mới'.
  'All': 'Tất cả',
  'Show': 'Hiển thị',
  'Browser preview with demo data. Nothing is stored or sent; the desktop app runs the real gateway.':
    'Bản xem trước trên trình duyệt với dữ liệu demo. Không có gì được lưu hay gửi đi; gateway thật chạy trong ứng dụng desktop.',
  'Gateway Running · {port}': 'Gateway đang chạy · {port}',
  'Gateway Couldn’t Start': 'Gateway không khởi động được',
  'Open Overview': 'Mở Tổng quan',
  'Port and start-up': 'Cổng và khởi động',
  'Set in Overview, next to the gateway’s address.': 'Đặt trong Tổng quan, cạnh địa chỉ của gateway.',

  // Shared controls
  'Cancel': 'Hủy',
  'Done': 'Xong',
  'Save': 'Lưu',
  'Copy': 'Sao chép',
  'Copied': 'Đã sao chép',
  'Remove': 'Gỡ',
  'Delete': 'Xóa',
  'Edit…': 'Sửa…',
  'Name': 'Tên',
  'Optional': 'Không bắt buộc',
  'Required': 'Bắt buộc',
  'optional': 'không bắt buộc',
  'Actions': 'Thao tác',
  'Unknown': 'Chưa rõ',
  'Never': 'Chưa bao giờ',
  'Error': 'Lỗi',
  '{n} task': '{n} task',
  '{n} tasks': '{n} task',

  // Overview
  'Checking the gateway…': 'Đang kiểm tra gateway…',
  'Gateway Running': 'Gateway đang chạy',
  'Gateway Stopped': 'Gateway đã dừng',
  'Loopback only · other devices can’t reach it': 'Chỉ trên máy này · thiết bị khác không truy cập được',
  'Clients can’t connect until the gateway is started.': 'Client chưa kết nối được cho tới khi gateway được bật.',
  'Copy URL': 'Sao chép URL',
  'Configure…': 'Cấu hình…',
  'Stop': 'Dừng',
  'Start Gateway': 'Bật gateway',
  'Try Again': 'Thử lại',
  'Get Started': 'Bắt đầu',
  'Connect a provider': 'Kết nối một nhà cung cấp',
  'An API key or a local runtime such as Ollama. Keys stay in the system’s credential store.':
    'Một API key hoặc một runtime cục bộ như Ollama. Key được giữ trong kho thông tin xác thực của hệ thống.',
  'Make a combo': 'Tạo một combo',
  'A named list of models tried in order, so a quota error moves on to the next one.':
    'Một danh sách model có tên, được thử lần lượt, để khi hết quota thì chuyển sang model kế tiếp.',
  'Create a client token': 'Tạo token cho client',
  'Each app that uses the gateway gets its own token and, if you like, a monthly limit.':
    'Mỗi ứng dụng dùng gateway có token riêng và, nếu muốn, một hạn mức hằng tháng.',
  'Connect a Client': 'Kết nối một client',
  'Client': 'Client',
  'Model': 'Model',
  'Shell': 'Shell',
  'OpenAI SDK': 'OpenAI SDK',
  'No models yet': 'Chưa có model',
  '{name} (combo)': '{name} (combo)',
  'Setup snippet': 'Đoạn cấu hình',
  'Claude Code talks the Anthropic Messages API; the gateway translates it for every connection. All three default models point at the route you pick.':
    'Claude Code dùng Anthropic Messages API; gateway chuyển đổi nó cho mọi kết nối. Cả ba model mặc định đều trỏ tới tuyến bạn chọn.',
  'Add this provider to opencode.json in your project or ~/.config/opencode/.':
    'Thêm nhà cung cấp này vào opencode.json trong dự án hoặc trong ~/.config/opencode/.',
  'Any OpenAI-compatible SDK works the same way: base URL, token, and a route as the model.':
    'Mọi SDK tương thích OpenAI đều dùng như nhau: base URL, token, và một tuyến ở chỗ tên model.',
  'A quick check from a terminal.': 'Kiểm tra nhanh từ terminal.',
  'Set PYGG_API_KEY to the token you saved when you created the client.':
    'Đặt PYGG_API_KEY bằng token bạn đã lưu khi tạo client.',
  'You’ll need a client token first.': 'Bạn cần có token client trước.',
  'Last 24 Hours': '24 giờ qua',
  'Open Usage': 'Mở Mức dùng',
  'Requests': 'Yêu cầu',
  'Known cost': 'Chi phí đã biết',
  'includes estimates': 'gồm cả ước tính',
  'Cost unknown': 'Chưa rõ chi phí',
  'requests, not in the total': 'yêu cầu, không tính vào tổng',
  'Fell back': 'Đã chuyển model',
  'to the next model': 'sang model kế tiếp',
  'Failed': 'Thất bại',
  'No client': 'Không có client',
  'asked for {model}': 'yêu cầu {model}',
  'No requests yet. Requests from your clients appear here as they happen.':
    'Chưa có yêu cầu nào. Yêu cầu từ các client sẽ hiện ở đây ngay khi diễn ra.',
  'Gateway Settings': 'Cài đặt gateway',
  'Changing the port restarts the gateway; clients need the new URL.': 'Đổi cổng sẽ khởi động lại gateway; client cần URL mới.',
  'Port': 'Cổng',
  'A number from 1024 to 65535.': 'Một số từ 1024 đến 65535.',
  'Start with the app': 'Bật cùng ứng dụng',
  'Clients work as soon as Pygg is open.': 'Client dùng được ngay khi Pygg đang mở.',
  'Address': 'Địa chỉ',
  'The gateway listens on 127.0.0.1 only. Sharing it with other devices isn’t offered.':
    'Gateway chỉ lắng nghe trên 127.0.0.1. Không có tùy chọn chia sẻ cho thiết bị khác.',

  // Connections
  'Loading connections…': 'Đang tải kết nối…',
  'No Connections Yet': 'Chưa có kết nối',
  'Connect an API key from Gemini, OpenRouter, OpenAI and others, or a local runtime such as Ollama. Every client of the gateway can then use its models.':
    'Kết nối API key từ Gemini, OpenRouter, OpenAI và các nhà cung cấp khác, hoặc runtime cục bộ như Ollama. Sau đó mọi client của gateway đều dùng được các model đó.',
  'Connect Provider…': 'Kết nối nhà cung cấp…',
  'Connect Provider': 'Kết nối nhà cung cấp',
  'Providers': 'Nhà cung cấp',
  'Provider': 'Nhà cung cấp',
  'Disabled': 'Đã tắt',
  'Key missing': 'Thiếu key',
  'Check failed': 'Kiểm tra thất bại',
  'Ready': 'Sẵn sàng',
  'Checked': 'Đã kiểm tra',
  'No problem seen': 'Chưa thấy lỗi',
  '{n} model': '{n} model',
  '{n} models': '{n} model',
  'Disable {name}': 'Tắt {name}',
  'Enable {name}': 'Bật {name}',
  'Check & Refresh Models': 'Kiểm tra & làm mới model',
  'Replace Key…': 'Thay key…',
  'Add Key…': 'Thêm key…',
  'Remove Connection…': 'Gỡ kết nối…',
  'Authentication': 'Xác thực',
  'API key · stored in {store}': 'API key · lưu trong {store}',
  'No key needed': 'Không cần key',
  'Endpoint': 'Endpoint',
  'Health': 'Tình trạng',
  'Reachable · {n} models listed': 'Truy cập được · liệt kê {n} model',
  'checked {time}': 'kiểm tra lúc {time}',
  'Not checked since the app started': 'Chưa kiểm tra từ khi mở ứng dụng',
  'Billing': 'Tính phí',
  'Data use': 'Sử dụng dữ liệu',
  'Whether the provider may train on your requests. Check its terms; the app doesn’t know yet.':
    'Nhà cung cấp có được dùng yêu cầu của bạn để huấn luyện hay không. Hãy xem điều khoản của họ; ứng dụng chưa biết điều này.',
  'Model names': 'Tên model',
  'Clients ask for a model of this connection by this name.': 'Client gọi model của kết nối này bằng tên này.',
  '{listed} models listed, {added} new.': 'Liệt kê {listed} model, {added} model mới.',
  '{listed} models listed, nothing new.': 'Liệt kê {listed} model, không có gì mới.',
  'Models': 'Model',
  'Search models': 'Tìm model',
  'All models': 'Mọi model',
  'Enabled': 'Đang bật',
  'Paid or unknown, no price': 'Trả phí hoặc chưa rõ, chưa có giá',
  'Add Model…': 'Thêm model…',
  'No models yet. Check & Refresh Models asks the provider for its list; Add Model… adds one by name.':
    'Chưa có model. “Kiểm tra & làm mới model” sẽ hỏi danh sách từ nhà cung cấp; “Thêm model…” thêm một model theo tên.',
  'Price': 'Giá',
  'No models match.': 'Không có model nào khớp.',
  'No price · cost Unknown': 'Chưa có giá · chi phí chưa rõ',
  'Quota, not money': 'Tính theo quota, không phải tiền',
  'Not needed': 'Không cần',
  'your price': 'giá của bạn',
  'Use {model}': 'Dùng {model}',
  'Edit Price…': 'Sửa giá…',
  'Set Price…': 'Đặt giá…',
  'Remove {model}': 'Gỡ {model}',
  'Remove {name}?': 'Gỡ {name}?',
  'Remove Connection': 'Gỡ kết nối',
  'Its key is deleted from {store}, its models leave every combo, and requests for {id}/… stop working.':
    'Key của nó bị xóa khỏi {store}, các model của nó bị bỏ khỏi mọi combo, và yêu cầu tới {id}/… sẽ không còn hoạt động.',
  'Usage already recorded is kept.': 'Mức dùng đã ghi nhận vẫn được giữ.',
  'Costs are recorded as $0. Use this for free tiers you’re sure about.': 'Chi phí được ghi là $0. Chỉ dùng cho gói miễn phí bạn chắc chắn.',
  'Runs on this computer; API cost is $0.': 'Chạy trên máy này; chi phí API là $0.',
  'Counted against a plan’s quota, never shown as money.': 'Tính vào quota của gói, không bao giờ hiển thị bằng tiền.',
  'Billed per token. Enter prices per model to see estimates; without one the cost is Unknown.':
    'Tính phí theo token. Nhập giá cho từng model để xem ước tính; nếu không có giá, chi phí là Chưa rõ.',
  'Costs stay Unknown unless the provider reports them or you enter a price.':
    'Chi phí giữ là Chưa rõ, trừ khi nhà cung cấp báo hoặc bạn nhập giá.',
  'Connecting checks the key by reading the model list. No request is billed.':
    'Khi kết nối, ứng dụng kiểm tra key bằng cách đọc danh sách model. Không có yêu cầu nào bị tính phí.',
  'Connecting…': 'Đang kết nối…',
  'Custom': 'Tùy chỉnh',
  'Also the first part of model names, e.g. {id}/model.': 'Cũng là phần đầu của tên model, ví dụ {id}/model.',
  'My Provider': 'Nhà cung cấp của tôi',
  'The base URL before /chat/completions, e.g. https://api.example.com/v1': 'Base URL đứng trước /chat/completions, ví dụ https://api.example.com/v1',
  'Change it if Ollama runs elsewhere.': 'Đổi nếu Ollama chạy ở chỗ khác.',
  'API Key': 'API key',
  'Stored in {store}. Pygg never shows it again.': 'Lưu trong {store}. Pygg không bao giờ hiển thị lại.',
  'Replace Key for {name}': 'Thay key cho {name}',
  'Add Key for {name}': 'Thêm key cho {name}',
  'The new key replaces the old one in {store} right away.': 'Key mới thay key cũ trong {store} ngay lập tức.',
  'Save Key': 'Lưu key',
  'Add Model to {name}': 'Thêm model vào {name}',
  'For models the provider doesn’t list. Use the exact name the provider expects.':
    'Dành cho model mà nhà cung cấp không liệt kê. Dùng đúng tên nhà cung cấp yêu cầu.',
  'Add Model': 'Thêm model',
  'Clients will ask for {route}': 'Client sẽ gọi {route}',
  'Price for {model}': 'Giá cho {model}',
  'In US dollars per million tokens, from the provider’s price list. Costs are then estimated from the tokens each response reports.':
    'Tính bằng đô la Mỹ cho mỗi triệu token, lấy từ bảng giá của nhà cung cấp. Chi phí sau đó được ước tính từ số token mỗi phản hồi báo về.',
  'Clear Price': 'Xóa giá',
  'Save Price': 'Lưu giá',
  'Input': 'Đầu vào',
  'Output': 'Đầu ra',
  'Cached input': 'Đầu vào từ cache',
  'Optional. Without it, cached tokens are priced as input.': 'Không bắt buộc. Nếu bỏ trống, token từ cache được tính như đầu vào.',
  'Providers that report the charge themselves, such as OpenRouter, are recorded as Actual and don’t need a price.':
    'Nhà cung cấp tự báo số tiền, như OpenRouter, được ghi là Thực tế và không cần nhập giá.',
  '/ 1M tokens': '/ 1 triệu token',
  '{input} in · {output} out per 1M': '{input} vào · {output} ra / 1 triệu',
  'Free': 'Miễn phí',
  'Local': 'Cục bộ',
  'Subscription': 'Gói thuê bao',
  'Paid': 'Trả phí',
  'Unknown billing': 'Chưa rõ cách tính phí',

  // Combos
  'Loading combos…': 'Đang tải combo…',
  'No Combos Yet': 'Chưa có combo',
  'A combo is a model name your clients can use, such as “coding”, that stands for a list of models. If the first is out of quota or unavailable, the next one answers.':
    'Combo là một tên model mà client dùng được, ví dụ “coding”, đại diện cho một danh sách model. Nếu model đầu hết quota hoặc không khả dụng, model kế tiếp sẽ trả lời.',
  'New Combo…': 'Combo mới…',
  'New Combo': 'Combo mới',
  'Copy Name': 'Sao chép tên',
  'Delete {name}': 'Xóa {name}',
  'Delete {name}?': 'Xóa {name}?',
  'Delete Combo': 'Xóa combo',
  'Order': 'Thứ tự',
  'Skipped: its connection is disabled or removed': 'Bỏ qua: kết nối của nó đã tắt hoặc bị gỡ',
  'Empty: its models’ connections were removed.': 'Trống: kết nối của các model trong đó đã bị gỡ.',
  'The next model is tried on a rejected key, an exhausted quota or rate limit, or an unavailable model, and only before any of the answer has reached the client. After a timeout or a server error the provider may already have run (and billed) the request, so the gateway stops there unless the client allows retries (Clients → Limits). A malformed request is returned as it is.':
    'Model kế tiếp được thử khi key bị từ chối, hết quota hoặc bị giới hạn tốc độ, hoặc model không khả dụng, và chỉ khi chưa có phần nào của câu trả lời tới client. Sau khi quá thời gian hoặc lỗi máy chủ, nhà cung cấp có thể đã chạy (và tính phí) request, nên gateway dừng ở đó trừ khi client cho phép thử lại (Clients → Giới hạn). Yêu cầu sai định dạng được trả về nguyên trạng.',
  'Clients that ask for “{name}” will get “model not found” until a combo with that name exists again.':
    'Client gọi “{name}” sẽ nhận lỗi “không tìm thấy model” cho tới khi lại có combo mang tên đó.',
  'Edit {name}': 'Sửa {name}',
  'Requests for this name try the models from the top down.': 'Yêu cầu tới tên này thử các model từ trên xuống.',
  'Save Combo': 'Lưu combo',
  'Create Combo': 'Tạo combo',
  'What clients put in the model field, e.g. coding or cheap.': 'Giá trị client điền vào trường model, ví dụ coding hoặc cheap.',
  'No spaces or slashes.': 'Không dùng dấu cách hoặc dấu gạch chéo.',
  'Another combo has this name.': 'Đã có combo khác mang tên này.',
  'Models in order': 'Các model theo thứ tự',
  'Move up': 'Lên trên',
  'Move down': 'Xuống dưới',
  'Add at least one model.': 'Thêm ít nhất một model.',
  '{connections} share one quota: when one is refused for its quota or rate limit, the gateway does not try the others for that model, since they would be refused too. If they are separate accounts, confirm that in Connections.':
    '{connections} dùng chung một quota: khi một kết nối bị từ chối vì hết quota hoặc giới hạn tốc độ, gateway không thử các kết nối kia cho model đó, vì chúng cũng sẽ bị từ chối. Nếu đó là các tài khoản riêng, hãy xác nhận trong Kết nối.',
  'Add model': 'Thêm model',
  'Choose a model…': 'Chọn một model…',
  'Connect a provider first': 'Hãy kết nối nhà cung cấp trước',
  'Connection removed': 'Kết nối đã bị gỡ',
  'Connection disabled · skipped': 'Kết nối đã tắt · bỏ qua',
  'priced': 'đã có giá',
  'Not in the model list': 'Không có trong danh sách model',
  'Only the models it lists at $0 are on. A model with a price, or none, starts off; turn one on to use it.':
    'Chỉ các model niêm yết $0 được bật. Model có giá hoặc chưa có giá sẽ tắt sẵn; bật model nào thì dùng model đó.',
  'This connection is set as free and this model is not listed at $0, so Pygg turned it off. Turn it on to use it; its cost follows its price, or is Unknown without one.':
    'Kết nối này đặt là miễn phí còn model này không niêm yết $0, nên Pygg đã tắt nó. Bật lên để dùng; chi phí tính theo giá của model, hoặc là Unknown nếu chưa có giá.',
  'Off: not $0': 'Tắt: không phải $0',

  // Clients
  'Loading clients…': 'Đang tải client…',
  'No Client Tokens Yet': 'Chưa có token client',
  'Every app that sends requests through the gateway needs a token. Give each one its own, so you can see what each spends and revoke one without touching the others.':
    'Mỗi ứng dụng gửi yêu cầu qua gateway cần một token. Hãy cấp riêng cho từng ứng dụng để thấy mỗi cái tiêu bao nhiêu và thu hồi một cái mà không ảnh hưởng cái khác.',
  'Create Client Token…': 'Tạo token client…',
  'Client Tokens': 'Token client',
  'Token': 'Token',
  'Last used': 'Dùng lần cuối',
  'This month': 'Tháng này',
  'Revoked': 'Đã thu hồi',
  'Limit…': 'Hạn mức…',
  'Revoke…': 'Thu hồi…',
  'Tokens are stored only as a fingerprint; a lost token can’t be shown again. Create a new one and revoke the old.':
    'Token chỉ được lưu dưới dạng dấu vân tay; token bị mất không thể hiển thị lại. Hãy tạo token mới và thu hồi token cũ.',
  'Revoke {name}?': 'Thu hồi {name}?',
  'Revoke Token': 'Thu hồi token',
  'Requests with this token are refused from now on. Its usage history is kept.':
    'Từ giờ các yêu cầu dùng token này sẽ bị từ chối. Lịch sử mức dùng của nó vẫn được giữ.',
  'no limit': 'không giới hạn',
  'Share of the monthly limit': 'Phần đã dùng của hạn mức tháng',
  '+ {n} request with unknown cost': '+ {n} yêu cầu chưa rõ chi phí',
  '+ {n} requests with unknown cost': '+ {n} yêu cầu chưa rõ chi phí',
  'Token for {name}': 'Token cho {name}',
  'Copy it now. It isn’t stored and can’t be shown again.': 'Hãy sao chép ngay. Token không được lưu và không thể hiển thị lại.',
  'Copy Token': 'Sao chép token',
  'The setup snippets in Overview read it from PYGG_API_KEY. To set that for your user:':
    'Các đoạn cấu hình trong Tổng quan đọc token từ PYGG_API_KEY. Để đặt biến này cho tài khoản của bạn:',
  'Open a new terminal afterwards so it sees the variable.': 'Sau đó mở terminal mới để nó nhận biến này.',
  'Create Client Token': 'Tạo token client',
  'For one app, such as Claude Code on this computer.': 'Dành cho một ứng dụng, ví dụ Claude Code trên máy này.',
  'Create Token': 'Tạo token',
  'Monthly limit': 'Hạn mức hằng tháng',
  'Optional. Each request is checked against an estimate of its cost before it is sent, and refused once this month’s spend reaches the limit. The estimate isn’t a guarantee: the provider’s bill can be higher.':
    'Không bắt buộc. Mỗi request được đối chiếu với chi phí ước tính trước khi gửi, và bị từ chối khi chi tiêu trong tháng chạm hạn mức. Ước tính không phải cam kết: hoá đơn của nhà cung cấp có thể cao hơn.',
  'No limit': 'Không giới hạn',
  'Monthly Limit for {name}': 'Hạn mức hằng tháng cho {name}',
  'Spent so far this month: {amount} known.': 'Đã tiêu trong tháng: {amount} đã biết.',
  'Save Limit': 'Lưu hạn mức',
  'Leave empty for no limit. Resets on the 1st (UTC). Checked against an estimate before each request, so the provider’s bill can end up higher.':
    'Để trống nếu không giới hạn. Đặt lại vào ngày 1 (giờ UTC). Mỗi request được đối chiếu với chi phí ước tính, nên hoá đơn của nhà cung cấp vẫn có thể cao hơn.',
  '{n} request this month had an unknown cost. Only spend held for it counts against the limit.':
    '{n} request trong tháng chưa rõ chi phí. Chỉ phần tiền đã giữ cho nó được tính vào hạn mức.',
  '{n} requests this month had an unknown cost. Only spend held for them counts against the limit.':
    '{n} request trong tháng chưa rõ chi phí. Chỉ phần tiền đã giữ cho chúng được tính vào hạn mức.',
  'Spent so far this month: {amount} known, {risk} possibly charged.':
    'Đã tiêu trong tháng: {amount} đã biết, {risk} có thể đã bị tính.',
  'Over the limit by {amount}': 'Vượt hạn mức {amount}',
  '+ {amount} possibly charged': '+ {amount} có thể đã bị tính',
  'Requests that failed or were cut off after reaching the provider. Their cost is unknown, so the amount held for them counts against the limit.':
    'Request lỗi hoặc bị cắt ngang sau khi đã tới nhà cung cấp. Chi phí chưa rõ, nên phần tiền đã giữ cho chúng được tính vào hạn mức.',

  // Usage
  'Period': 'Khoảng thời gian',
  '24 Hours': '24 giờ',
  '7 Days': '7 ngày',
  '30 Days': '30 ngày',
  'Loading usage…': 'Đang tải mức dùng…',
  'No Usage Recorded Yet': 'Chưa ghi nhận mức dùng',
  'Point a client at the gateway and its requests appear here, each with its cost and where that figure comes from.':
    'Trỏ một client vào gateway và các yêu cầu của nó sẽ hiện ở đây, mỗi yêu cầu kèm chi phí và nguồn của con số đó.',
  'API Costs': 'Chi phí API',
  'Task Results': 'Kết quả task',
  'known money per kept result': 'tiền đã biết cho mỗi kết quả được giữ',
  '{kept} of {tasks} kept': 'Giữ {kept} trên {tasks}',
  '{passed} of {checked} attempts passed their checks': '{passed} trên {checked} lần chạy đạt check',
  '{d} discarded · {r} retried · {c} stopped': '{d} bị bỏ · {r} chạy lại · {c} bị dừng',
  '{n} agent run with unknown cost': '{n} lần chạy agent chưa rõ chi phí',
  '{n} agent runs with unknown cost': '{n} lần chạy agent chưa rõ chi phí',
  'Every paid step counts, kept or not, so a cheap step that failed is in the cost of the result that followed. Agents signed in with a subscription report no money, so their runs are not in the figure.':
    'Mọi bước trả phí đều được tính, dù kết quả có được giữ hay không, nên một bước rẻ đã thất bại cũng nằm trong chi phí của kết quả sau nó. Agent đăng nhập bằng gói thuê bao không báo tiền, nên các lần chạy đó không có trong con số này.',
  'known': 'đã biết',
  '{amount} actual': '{amount} thực tế',
  '≈{amount} estimated': '≈{amount} ước tính',
  '{n} request with unknown cost': '{n} yêu cầu chưa rõ chi phí',
  '{n} requests with unknown cost': '{n} yêu cầu chưa rõ chi phí',
  'Unknown costs are not in the total. Give a model a price in Connections to estimate them.':
    'Chi phí chưa rõ không nằm trong tổng. Đặt giá cho model trong Kết nối để ước tính chúng.',
  'By Model': 'Theo model',
  'By Client': 'Theo client',
  'Tokens': 'Token',
  '{n} not OK': '{n} không thành công',
  '{n} unknown': '{n} chưa rõ',
  'All requests': 'Mọi yêu cầu',
  'Fallbacks and failures': 'Chuyển model và thất bại',
  'Unknown cost': 'Chưa rõ chi phí',
  'Time': 'Thời gian',
  'Tokens in → out': 'Token vào → ra',
  'Cost': 'Chi phí',
  'Source': 'Nguồn',
  'Result': 'Kết quả',
  'Nothing to show for this filter.': 'Không có gì cho bộ lọc này.',
  'Shows up to 200 of the most recent requests in this period. Failed attempts before a fallback are listed too. A request the provider refused before answering is usually not billed; one cut off after it started may have been, so its cost shows as unknown or estimated.':
    'Hiển thị tối đa 200 yêu cầu gần nhất trong khoảng này. Các lần thử thất bại trước khi chuyển model cũng được liệt kê. Yêu cầu bị nhà cung cấp từ chối trước khi trả lời thường không bị tính phí; yêu cầu bị ngắt sau khi đã bắt đầu thì có thể đã bị tính, nên chi phí hiện là chưa rõ hoặc ước tính.',
  'Asked for': 'Yêu cầu',
  'Answered by': 'Trả lời bởi',
  'Cost source': 'Nguồn chi phí',
  'Time taken': 'Thời gian xử lý',
  'OK': 'OK',
  'Failed; the next model of the combo was tried': 'Thất bại; đã thử model kế tiếp trong combo',
  'Client left': 'Client đã ngắt',
  'Cut off': 'Bị cắt ngang',
  'Actual': 'Thực tế',
  'Estimated': 'Ước tính',

  // Cost provenance
  'Rejected by the provider (HTTP {status}), not billed': 'Bị nhà cung cấp từ chối (HTTP {status}), không tính phí',
  'Reported by the provider': 'Do nhà cung cấp báo',
  'Your price × reported tokens': 'Giá của bạn × số token được báo',
  'Connection marked free': 'Kết nối được đánh dấu miễn phí',
  'Local model': 'Model cục bộ',
  'No price recorded for this model': 'Chưa có giá cho model này',
  'The provider reported no usage': 'Nhà cung cấp không báo mức dùng',
  'Subscription: counts against a quota, not money': 'Gói thuê bao: tính vào quota, không phải tiền',
  'Failed after reaching the provider; billing unknown': 'Thất bại sau khi tới nhà cung cấp; chưa rõ có tính phí không',
  'Cut short before usage was reported': 'Bị cắt ngang trước khi mức dùng được báo',
  'Not sent': 'Chưa gửi',
  'Could not connect; not sent': 'Không kết nối được; chưa gửi',

  // Cost-safe gateway (ROADMAP P0-1..P0-4)
  'API': 'API',
  'Anthropic': 'Anthropic',
  'Anthropic Messages': 'Anthropic Messages',
  'OpenAI Chat Completions': 'OpenAI Chat Completions',
  'Anthropic requests (Claude Code) pass through unchanged, prompt cache included. OpenAI-format clients can’t use it yet.':
    'Request Anthropic (Claude Code) được chuyển tiếp nguyên vẹn, giữ cả prompt cache. Client định dạng OpenAI chưa dùng được kết nối này.',
  'OpenAI-format requests pass through unchanged. Anthropic requests are translated, and prompt-cache hints survive only where the provider accepts them.':
    'Request định dạng OpenAI được chuyển tiếp nguyên vẹn. Request Anthropic được dịch, và đánh dấu prompt cache chỉ được giữ ở nhà cung cấp chấp nhận nó.',
  'Anthropic clients only': 'chỉ client Anthropic',
  'Custom (OpenAI)': 'Tùy chỉnh (OpenAI)',
  'Custom (Anthropic)': 'Tùy chỉnh (Anthropic)',
  'The base URL before /messages, e.g. https://api.example.com/v1': 'Base URL đứng trước /messages, ví dụ https://api.example.com/v1',
  'Key rejected': 'Key bị từ chối',
  'Not allowed': 'Không được phép',
  'Payment required': 'Cần thanh toán',
  'Rate-limited': 'Bị giới hạn tốc độ',
  'Model unavailable': 'Model không khả dụng',
  'Provider error': 'Lỗi từ nhà cung cấp',
  'Unreachable': 'Không kết nối được',
  'whole connection': 'cả kết nối',
  'skipped {when}': 'bỏ qua {when}',
  'for {n} s': 'trong {n} giây',
  'for {n} min': 'trong {n} phút',
  'until you replace the key': 'cho tới khi bạn thay key',
  'until you clear it': 'cho tới khi bạn xoá trạng thái này',
  '{n} model cooling down': '{n} model đang tạm nghỉ',
  '{n} models cooling down': '{n} model đang tạm nghỉ',
  'Skipped targets are not called, so they cost nothing and don’t make the limit worse.':
    'Model bị bỏ qua sẽ không được gọi, nên không tốn tiền và không làm giới hạn tệ thêm.',
  'Try Again Now': 'Thử lại ngay',
  'See below': 'Xem bên dưới',
  'No problems seen': 'Chưa thấy vấn đề',
  'Independent quota confirmed': 'Đã xác nhận quota độc lập',
  'You confirmed on {date} that this connection has its own provider quota. When another connection is rate-limited or refused, this one may still answer. The provider’s own limits still apply.':
    'Bạn đã xác nhận ngày {date} rằng kết nối này có quota riêng ở nhà cung cấp. Khi kết nối khác bị giới hạn hoặc từ chối, kết nối này vẫn có thể trả lời. Giới hạn của nhà cung cấp vẫn áp dụng.',
  'Shares limits with {names}. While one is rate-limited or refused, the others aren’t used for the same model; using a second key to get around a limit can break the provider’s terms.':
    'Dùng chung giới hạn với {names}. Khi một kết nối đang bị giới hạn hoặc từ chối, các kết nối kia không được dùng cho cùng model đó; dùng key thứ hai để lách giới hạn có thể vi phạm điều khoản của nhà cung cấp.',
  'Assumed to share limits with other {provider} connections you add. Turn on only if the provider gives it a separate quota, for example a different organization.':
    'Được coi là dùng chung giới hạn với các kết nối {provider} khác bạn thêm. Chỉ bật nếu nhà cung cấp cấp cho nó quota riêng, ví dụ một tổ chức khác.',
  'Confirm an independent quota for {name}?': 'Xác nhận {name} có quota độc lập?',
  'Confirm Independent Quota': 'Xác nhận quota độc lập',
  'A different key is not proof of a different quota. Keys of one organization or project usually share its rate limits and credit.':
    'Key khác không có nghĩa là quota khác. Các key của cùng một tổ chức hoặc dự án thường dùng chung giới hạn và số dư.',
  'Confirm only if {provider} gives this connection its own quota, for example a separate organization you are allowed to use. The gateway can’t check this, and the provider’s terms still apply.':
    'Chỉ xác nhận nếu {provider} cấp cho kết nối này quota riêng, ví dụ một tổ chức riêng mà bạn được phép dùng. Gateway không kiểm tra được điều này, và điều khoản của nhà cung cấp vẫn áp dụng.',
  'Cache write': 'Ghi cache',
  'Optional. Anthropic charges extra to write the cache; without it, writes are priced as input and the ledger says so.':
    'Không bắt buộc. Anthropic tính thêm phí khi ghi cache; nếu bỏ trống, phần ghi được tính như đầu vào và sổ chi phí ghi rõ điều đó.',
  'Limits…': 'Giới hạn…',
  'Limits for {name}': 'Giới hạn cho {name}',
  'Save Limits': 'Lưu giới hạn',
  'Unknown price': 'Giá chưa rõ',
  'Block': 'Chặn',
  'Allow': 'Cho phép',
  'Applies once the client has a limit.': 'Có hiệu lực khi client có hạn mức.',
  'Models without a price are skipped, so the limit can’t be passed unseen.':
    'Model chưa có giá sẽ bị bỏ qua, để hạn mức không bị vượt mà không ai thấy.',
  'Models without a price may answer; their cost isn’t counted against the limit.':
    'Model chưa có giá vẫn được trả lời; chi phí của chúng không được tính vào hạn mức.',
  'At once': 'Cùng lúc',
  'Requests this client may have in flight. More are refused with 429 until one finishes.':
    'Số request client này được chạy cùng lúc. Request vượt quá bị từ chối với mã 429 cho tới khi có request xong.',
  'up to {n} request at once': 'tối đa {n} request cùng lúc',
  'up to {n} requests at once': 'tối đa {n} request cùng lúc',
  'unknown prices blocked': 'chặn giá chưa rõ',
  'unknown prices allowed': 'cho phép giá chưa rõ',
  '{amount} held for requests in flight': '{amount} đang giữ cho request đang chạy',
  'Prompt cache': 'Prompt cache',
  '{read} read · {write} written': '{read} đọc · {write} ghi',
  'Route': 'Đường đi',
  'Passed through in the same format': 'Chuyển tiếp nguyên định dạng',
  'Translated ({mode})': 'Đã dịch ({mode})',
  'Session': 'Phiên',
  'Claude Code talks the Anthropic Messages API. To an Anthropic connection it passes through unchanged, prompt cache included; other connections get a translation. The session header keeps each terminal on one model.':
    'Claude Code dùng Anthropic Messages API. Tới kết nối Anthropic, request được chuyển tiếp nguyên vẹn, giữ cả prompt cache; các kết nối khác nhận bản dịch. Header session giữ mỗi terminal trên một model.',
  'Add this provider to opencode.json in your project or ~/.config/opencode/. The fixed session header keeps OpenCode on one model of a combo.':
    'Thêm nhà cung cấp này vào opencode.json trong dự án hoặc trong ~/.config/opencode/. Header session cố định giữ OpenCode trên một model của combo.',
  'Any OpenAI-compatible SDK works the same way: base URL, token, and a route as the model. Send one session header per conversation to keep it on one model.':
    'Mọi SDK tương thích OpenAI đều dùng như nhau: base URL, token, và một tuyến ở chỗ tên model. Gửi một header session cho mỗi hội thoại để giữ nó trên một model.',
  'Requests that were running are still finishing, so their cost is recorded. Until they do, no other gateway can use this store.':
    'Các request đang chạy vẫn đang hoàn tất để chi phí được ghi sổ. Trong lúc đó, không gateway nào khác dùng được store này.',
  'Interrupted': 'Bị gián đoạn',
  'Change it if the server runs on another address or port.': 'Đổi nếu server chạy ở địa chỉ hoặc cổng khác.',
  'Leave empty for the provider’s standard endpoint. Change it if your plan has its own, such as a token plan or a regional endpoint.':
    'Để trống để dùng endpoint chuẩn của nhà cung cấp. Đổi nếu gói của bạn có endpoint riêng, ví dụ token plan hoặc endpoint theo vùng.',
  'Search connections': 'Tìm kết nối',
  'Needs attention': 'Cần xử lý',
  'Anthropic API': 'API Anthropic',
  'No connection matches.': 'Không có kết nối nào khớp.',
  'Free tier': 'Có gói miễn phí',
  'An ongoing free tier or free models, per the provider’s docs. Limits apply.':
    'Có gói miễn phí lâu dài hoặc model miễn phí, theo tài liệu của nhà cung cấp. Có giới hạn sử dụng.',
  'Trial credits': 'Có credit dùng thử',
  'Starting credit or a granted balance, then paid.': 'Có credit ban đầu hoặc số dư được tặng, sau đó trả phí.',
  'Billed per token from the start.': 'Tính phí theo token ngay từ đầu.',
  'On this computer': 'Chạy trên máy này',
  'Runs locally: no key, no per-token cost.': 'Chạy local: không cần key, không tính phí theo token.',
  'Any other OpenAI- or Anthropic-compatible endpoint.': 'Bất kỳ endpoint nào tương thích OpenAI hoặc Anthropic.',
  'Search providers': 'Tìm nhà cung cấp',
  'Free or credits': 'Miễn phí hoặc có credit',
  'No provider matches. Custom (OpenAI) connects any compatible endpoint.':
    'Không có nhà cung cấp nào khớp. Custom (OpenAI) kết nối được với mọi endpoint tương thích.',
  // Agents (official agent CLIs with their own sign-in)
  'Agents': 'Agent',
  'Agents on This Computer': 'Agent trên máy này',
  'Pygg starts each agent’s own program with the sign-in it already has: a subscription, a free tier or a key it manages. Pygg never reads, stores or shares that sign-in, and never offers it to other apps.':
    'Pygg chạy chính chương trình của từng agent bằng đăng nhập sẵn có của nó: gói subscription, gói miễn phí hoặc key do agent tự quản lý. Pygg không đọc, không lưu, không chia sẻ thông tin đăng nhập đó, và không cho ứng dụng khác dùng.',
  'Looking for agents…': 'Đang tìm agent…',
  'Show all {n}': 'Hiện tất cả {n}',
  'Requests ({n})': 'Yêu cầu ({n})',
  'Rate limits, as the provider last said': 'Giới hạn tốc độ, theo lời nhà cung cấp lần gần nhất',
  'Copied from the headers of its last answer for each model. Pygg does not route on them, and a provider that sends none shows nothing here.':
    'Chép từ header của lần trả lời gần nhất cho từng model. Pygg không định tuyến dựa vào chúng; nhà cung cấp nào không gửi thì ở đây trống.',
  'See it in Usage': 'Xem trong Mức dùng',
  'No request yet. Start the client with these settings; its first request shows here within seconds.':
    'Chưa có yêu cầu nào. Chạy client với cấu hình này; yêu cầu đầu tiên sẽ hiện ở đây sau vài giây.',
  'Working: {client} sent a request {ago}, answered by {model}.': 'Đã chạy: {client} gửi yêu cầu {ago}, do {model} trả lời.',
  'The last request, from {client} {ago}, did not succeed.': 'Yêu cầu gần nhất, từ {client} {ago}, không thành công.',
  'Every client': 'Mọi client',
  'Every model': 'Mọi model',
  'Clear Filters': 'Bỏ lọc',
  'Show the requests of {name}': 'Xem các yêu cầu của {name}',
  'Only want your IDE’s agent to hand tasks to Pygg, run by the agents you already sign in to? That needs no provider here.':
    'Chỉ muốn agent trong IDE giao tác vụ cho Pygg, chạy bằng các agent bạn đã đăng nhập sẵn? Việc đó không cần kết nối nhà cung cấp ở đây.',
  'Set Up From Your IDE': 'Cài đặt từ IDE',
  'Show the top 8': 'Chỉ hiện 8 dòng đầu',
  // Each agent's sign-in, as agents.rs describes it.
  'Your Claude subscription or Anthropic Console login, managed by Claude Code (`claude` then /login).':
    'Gói Claude hoặc đăng nhập Anthropic Console, do Claude Code quản lý (chạy `claude` rồi /login).',
  'Your ChatGPT plan or an OpenAI API key, managed by Codex (`codex login`).':
    'Gói ChatGPT hoặc API key OpenAI, do Codex quản lý (`codex login`).',
  'A Gemini API key set in Gemini CLI (run `gemini`, choose the API key). Google no longer accepts its free Google-account login from this CLI.':
    'API key Gemini đặt trong Gemini CLI (chạy `gemini`, chọn API key). Google không còn nhận đăng nhập miễn phí bằng tài khoản Google từ CLI này.',
  'The providers configured in OpenCode (`opencode auth login`).':
    'Các nhà cung cấp đã cấu hình trong OpenCode (`opencode auth login`).',
  'Installed': 'Đã cài',
  'Not found': 'Không tìm thấy',
  'How to Install': 'Cách cài',
  'Run a Task': 'Chạy task',
  'Install one of the agents above to run tasks with it.': 'Cài một trong các agent ở trên để chạy task với nó.',
  'Agent': 'Agent',
  'Repository': 'Repository',
  'A folder under Git. The agent works on a separate copy (a worktree) of its current commit; your files and branch are not touched.':
    'Một thư mục có Git. Agent làm việc trên một bản sao riêng (worktree) của commit hiện tại; file và nhánh của bạn không bị đụng tới.',
  'What should the agent do? Be specific about the files and what done looks like.':
    'Agent cần làm gì? Nêu rõ file nào và thế nào là xong.',
  'Optional, in the agent’s own naming. Empty uses the agent’s default.': 'Không bắt buộc, theo cách đặt tên của agent. Để trống thì dùng mặc định của agent.',
  'Shell commands': 'Lệnh shell',
  'The agent may run commands in the copy, such as tests. Codex always runs in its own sandbox.':
    'Agent được chạy lệnh trong bản sao, ví dụ chạy test. Codex luôn chạy trong sandbox riêng của nó.',
  'Off: the agent may read and edit files only. Codex always runs commands in its own sandbox.':
    'Tắt: agent chỉ được đọc và sửa file. Codex luôn chạy lệnh trong sandbox riêng của nó.',
  'Up to 15 minutes. The run uses the agent’s own plan or quota.': 'Tối đa 15 phút. Lần chạy dùng gói hoặc quota của chính agent.',
  'Running…': 'Đang chạy…',
  'Run Task': 'Chạy task',
  'Kept as branch {branch}. Your checkout was not changed.': 'Đã giữ thành nhánh {branch}. Thư mục làm việc của bạn không bị thay đổi.',
  'Discarded.': 'Đã bỏ.',
  'Timed out': 'Quá thời gian',
  '{s} s': '{s} giây',
  '{files} files, +{ins} −{del}': '{files} file, +{ins} −{del}',
  'agent’s default': 'mặc định của agent',
  '{input} in · {output} out · {cache} from cache': '{input} vào · {output} ra · {cache} từ cache',
  'Not reported': 'Không báo',
  'Unknown: depends on the agent’s sign-in. Its own list-price estimate: ${amount}':
    'Chưa rõ: tuỳ cách agent đăng nhập. Ước tính theo giá niêm yết của chính agent: ${amount}',
  'Unknown: depends on the agent’s sign-in (a subscription is not billed per request).':
    'Chưa rõ: tuỳ cách agent đăng nhập (gói subscription không tính tiền theo từng request).',
  'Agent output': 'Output của agent',
  'Changes': 'Thay đổi',
  'Branch title, e.g. Fix add()': 'Tên nhánh, ví dụ Sửa add()',
  'Branch title': 'Tên nhánh',
  'Keep as Branch': 'Giữ thành nhánh',
  'Discard': 'Bỏ',
  // Auto routing (ROADMAP P0-5)
  'Auto routing': 'Định tuyến auto',
  'list price': 'giá niêm yết',
  'Auto…': 'Auto…',
  'Not set': 'Chưa đặt',
  'no tier': 'chưa có tier',
  '{n}k context': 'context {n}k',
  'tools': 'tool',
  'images': 'ảnh',
  'Set by you {when}.': 'Do bạn đặt {when}.',
  'Read from the provider’s model list ({source}) {when}. Saving here makes them yours, and a refresh no longer changes them.':
    'Đọc từ danh sách model của nhà cung cấp ({source}) {when}. Lưu ở đây thì các giá trị thành của bạn, và lần làm mới sau không ghi đè nữa.',
  'Auto Routing for {model}': 'Định tuyến auto cho {model}',
  'auto chooses a model only for what it is known to support. Leave a field empty when you don’t know it.':
    'auto chỉ chọn model cho những gì đã biết chắc là nó hỗ trợ. Để trống ô nào bạn không biết.',
  'Tier': 'Tier',
  'T0 local, T1 small and fast, T2 capable at low cost, T3 strong coding and reasoning, T4 frontier. Your judgment; Pygg’s table only fills tiers nobody set.':
    'T0 chạy trên máy, T1 nhỏ và nhanh, T2 đủ mạnh với chi phí thấp, T3 mạnh về code và suy luận, T4 hàng đầu. Do bạn đánh giá; bảng của Pygg chỉ điền những tier chưa ai đặt.',
  '{tier} (Pygg’s table)': '{tier} (bảng của Pygg)',
  'From Pygg’s table: {why}. Saving here makes the tier yours, and the table no longer changes it.':
    'Theo bảng của Pygg: {why}. Lưu ở đây thì tier là của bạn, và bảng không đổi nó nữa.',
  'Not set (auto skips it)': 'Chưa đặt (auto bỏ qua)',
  'Context': 'Context',
  'Prompt and answer together, in tokens.': 'Cả prompt và câu trả lời, tính bằng token.',
  'Maximum output': 'Đầu ra tối đa',
  'Needed when a request sets its own output limit.': 'Cần khi request tự đặt giới hạn đầu ra.',
  'Tool calls': 'Gọi tool',
  'Supported': 'Có hỗ trợ',
  'Not supported': 'Không hỗ trợ',
  'Images': 'Ảnh',
  'Auto minimum tier': 'Tier tối thiểu cho auto',
  'The lowest tier “auto” may choose for this client. “auto/cheap” ignores it.':
    'Tier thấp nhất mà “auto” được chọn cho client này. “auto/cheap” không dùng thiết lập này.',
  'Why this model': 'Vì sao chọn model này',
  'auto (automatic)': 'auto (tự động)',
  'auto/cheap (automatic, lowest cost)': 'auto/cheap (tự động, chi phí thấp nhất)',
  'Automatic Routing': 'Định tuyến tự động',
  'Clients can ask for “auto” or “auto/cheap” instead of a model. The gateway then picks from your connected models when the request arrives, by fixed rules, and says why in the response.':
    'Client có thể gọi “auto” hoặc “auto/cheap” thay cho tên model. Khi request tới, gateway chọn trong các model bạn đã kết nối theo luật cố định, và ghi lý do vào response.',
  ' — the lowest tier at or above the client’s minimum (T2 unless you change it in Clients), then local, free, subscription and paid in that order, then the lower estimated cost. This is the lowest eligible tier under your policy, not a promise of the cheapest answer.':
    ' — tier thấp nhất đạt mức tối thiểu của client (T2 nếu bạn không đổi trong Clients), rồi ưu tiên local, free, subscription, paid theo thứ tự đó, rồi tới chi phí ước tính thấp hơn. Đây là tier đủ điều kiện thấp nhất theo chính sách của bạn, không phải cam kết câu trả lời rẻ nhất.',
  ' — the lowest estimated cost of this request among the models that can take it. No tier needed.':
    ' — chi phí ước tính thấp nhất cho request này trong số các model nhận được nó. Không cần tier.',
  'A model takes part only for what it is known to support: set tiers, context sizes and the rest under Connections → Auto… (OpenRouter’s list fills most of it in). Unknown counts as missing. A session stays on the model that first answered it.':
    'Model chỉ được chọn cho những gì đã biết chắc là nó hỗ trợ: đặt tier, context và các thông tin khác ở Connections → Auto… (danh sách của OpenRouter tự điền phần lớn). Chưa rõ thì coi như không có. Một session giữ nguyên model đã trả lời nó đầu tiên.',
  'Alias': 'Alias',
  'A new client (defaults)': 'Client mới (mặc định)',
  'Request format': 'Định dạng request',
  'OpenAI format': 'Định dạng OpenAI',
  'Anthropic (Claude Code)': 'Anthropic (Claude Code)',
  'With tools': 'Có tool',
  'With images': 'Có ảnh',
  'Prompt tokens': 'Token prompt',
  'Output limit': 'Giới hạn đầu ra',
  'None': 'Không có',
  'Simulating…': 'Đang mô phỏng…',
  'No connected model is eligible: the request would be refused with the reasons below.':
    'Không model nào đủ điều kiện: request sẽ bị từ chối với các lý do bên dưới.',
  'Estimate': 'Ước tính',
  'Why': 'Lý do',
  '$0 (as set)': '$0 (theo thiết lập)',
  'about {amount}': 'khoảng {amount}',
  'The request goes to the first; the others are its fallbacks, under the usual fallback rules.':
    'Request đi tới model đầu tiên; các model sau là dự phòng, theo luật fallback thông thường.',
  '{n} more model is eligible but beyond the route.': 'Còn {n} model đủ điều kiện nhưng nằm ngoài tuyến.',
  '{n} more models are eligible but beyond the route.': 'Còn {n} model đủ điều kiện nhưng nằm ngoài tuyến.',
  '{n} model left out': '{n} model bị loại',
  '{n} models left out': '{n} model bị loại',
  'only {n} connection': 'chỉ {n} kết nối',
  'only {n} connections': 'chỉ {n} kết nối',
  'retries {n} model after a server error': 'thử lại {n} model sau lỗi máy chủ',
  'retries up to {n} models after a server error': 'thử lại tối đa {n} model sau lỗi máy chủ',
  'After a server error': 'Sau lỗi máy chủ',
  'Stop and return the error. The provider may have run and billed the request, so trying another model could charge twice.':
    'Dừng và trả lỗi. Nhà cung cấp có thể đã chạy và tính phí request, nên thử model khác có thể bị tính tiền hai lần.',
  'Try the next model of the combo. The first provider may still bill the failed request; each try holds its estimated cost against the limit.':
    'Thử model kế tiếp của combo. Nhà cung cấp đầu vẫn có thể tính phí request lỗi; mỗi lần thử giữ chi phí ước tính của nó trong hạn mức.',
  'Stop (no retry)': 'Dừng (không thử lại)',
  'Retry {n} model': 'Thử lại {n} model',
  'Retry up to {n} models': 'Thử lại tối đa {n} model',
  'Connections': 'Kết nối',
  'Requests from this client go only to the connections ticked here, whatever a combo or a fallback says. Use it to keep sensitive work with providers whose terms you have checked.':
    'Request của client này chỉ tới các kết nối được đánh dấu ở đây, bất kể combo hay fallback. Dùng để giữ việc nhạy cảm ở các nhà cung cấp mà bạn đã kiểm tra điều khoản.',
  'Any connection of a combo may answer, including fallbacks to other providers.':
    'Mọi kết nối trong combo đều có thể trả lời, kể cả khi fallback sang nhà cung cấp khác.',
  // Data-use terms (review GW-10)
  'only providers that do not train on its data': 'chỉ nhà cung cấp không huấn luyện trên dữ liệu của nó',
  'Requests go only to connections whose terms you recorded as not training on requests, under Connections → Data use. Not recorded counts as may train.':
    'Request chỉ đi tới các kết nối mà bạn đã ghi là không huấn luyện trên request, ở Connections → Dùng dữ liệu. Chưa ghi được coi là có thể huấn luyện.',
  'Any connection may answer, whatever its terms say about training.': 'Mọi kết nối đều có thể trả lời, bất kể điều khoản nói gì về việc huấn luyện.',
  'Only providers that do not train on requests': 'Chỉ nhà cung cấp không huấn luyện trên request',
  'Not recorded: clients that require providers not to train on their data skip this connection. Check the provider’s terms and record them here.':
    'Chưa ghi: client yêu cầu nhà cung cấp không huấn luyện trên dữ liệu sẽ bỏ qua kết nối này. Hãy đọc điều khoản của nhà cung cấp và ghi lại ở đây.',
  'Recorded by you {when}. Pygg does not check it; a provider can change its terms.':
    'Bạn đã ghi {when}. Pygg không kiểm tra điều này; nhà cung cấp có thể đổi điều khoản.',
  'Not recorded': 'Chưa ghi',
  'Does not train on requests': 'Không huấn luyện trên request',
  'May train on requests': 'Có thể huấn luyện trên request',
  'Where the terms say so: a link, or the page and date': 'Điều khoản nói ở đâu: một liên kết, hoặc trang và ngày đọc',
  // Auto: thinking and effort (ROADMAP B1)
  'adaptive thinking': 'adaptive thinking',
  'Adaptive thinking': 'Adaptive thinking',
  'Anthropic API only. Claude Code asks for it; a model that does not take it answers 400, so auto skips it unless this says Supported.':
    'Chỉ với API Anthropic. Claude Code yêu cầu tính năng này; model không hỗ trợ sẽ trả lỗi 400, nên auto bỏ qua trừ khi ở đây ghi Hỗ trợ.',
  'Effort levels': 'Các mức effort',
  'Anthropic API only: the levels it takes, e.g. low, medium, high, xhigh, max; “none” when it takes no effort setting. Empty is unknown.':
    'Chỉ với API Anthropic: các mức model nhận, ví dụ low, medium, high, xhigh, max; ghi “none” nếu không nhận effort. Để trống là chưa rõ.',
  'Thinking': 'Thinking',
  'No thinking field': 'Không có trường thinking',
  'Thinking budget': 'Ngân sách thinking',
  'Thinking disabled': 'Tắt thinking',
  'Effort': 'Effort',
  'No effort': 'Không có effort',
  'Effort: {level}': 'Effort: {level}',
  'An Anthropic request that asks for thinking or an effort level goes only to Anthropic-API models known to take it; their model list says so (Connections → Refresh). Claude Code asks for adaptive thinking.':
    'Request Anthropic có yêu cầu thinking hoặc mức effort chỉ đi tới model API Anthropic đã biết là nhận nó; danh sách model của họ cho biết điều này (Connections → Refresh). Claude Code yêu cầu adaptive thinking.',
  'All connections': 'Mọi kết nối',
  'Only selected': 'Chỉ kết nối đã chọn',
  'Allowed connections': 'Kết nối được phép',
  'With nothing ticked, every request from this client is refused.': 'Nếu không đánh dấu kết nối nào, mọi request của client này sẽ bị từ chối.',
  'Counted against the limit': 'Tính vào hạn mức',
  '{amount}, the amount held for it: the provider may have charged for it':
    '{amount}, phần tiền đã giữ cho nó: nhà cung cấp có thể đã tính phí',
  'HTTP {status}: the provider may have run the request; billing unknown':
    'HTTP {status}: nhà cung cấp có thể đã chạy request; chưa rõ có tính phí không',
  'The gateway stopped before this request was settled; billing unknown':
    'Gateway dừng trước khi request này được ghi sổ; chưa rõ có tính phí không',
  'The last usage entry couldn’t be written: {error}. The request stays in the journal and is recorded with an unknown cost when the gateway next starts. Requests are refused while nothing can be written.':
    'Không ghi được mục chi phí gần nhất: {error}. Request vẫn nằm trong nhật ký và sẽ được ghi với chi phí chưa rõ khi gateway khởi động lại. Khi không ghi được gì, request sẽ bị từ chối.',
  // Smart tasks from IDE agents (pygg-mcp)
  'may start smart tasks': 'được tạo task thông minh',
  'Smart tasks': 'Task thông minh',
  'This client may start tasks through pygg-mcp or the task API. They run here with your agents’ sign-in, in a separate worktree; paid steps still wait for your approval in the app.':
    'Client này được tạo task qua pygg-mcp hoặc task API. Task chạy trên máy này bằng đăng nhập của các agent, trong worktree riêng; bước trả phí vẫn chờ bạn duyệt trong ứng dụng.',
  'Off: this client may only send model requests.': 'Tắt: client này chỉ được gửi request tới model.',
  'May start tasks': 'Được tạo task',
  'from {client}': 'từ {client}',
  'From Your IDE': 'Từ IDE của bạn',
  'Set It Up by Hand': 'Tự cài đặt bằng tay',
  'Asking your agents…': 'Đang hỏi các agent…',
  'Set up': 'Đã cài',
  'Not set up': 'Chưa cài',
  'Set up by hand': 'Cài bằng tay',
  'Removed outside Pygg': 'Đã bị gỡ ngoài Pygg',
  'Set up {date} with the token {client}. Backup: {backup}': 'Cài lúc {date} với token {client}. Bản sao lưu: {backup}',
  'none, the file did not exist': 'không có, lúc đó tệp chưa tồn tại',
  '{name} already has a server named pygg that Pygg did not make. Pygg leaves it alone.':
    '{name} đã có một server tên pygg không do Pygg tạo. Pygg không động vào nó.',
  'Pygg set this up, and the entry is gone. Remove revokes its token.':
    'Pygg đã cài mục này, nhưng mục đó đã mất. Gỡ sẽ thu hồi token của nó.',
  'Not found on this computer.': 'Không tìm thấy trên máy này.',
  'Set Up…': 'Cài đặt…',
  'Remove…': 'Gỡ…',
  'Remove Pygg from {name}?': 'Gỡ Pygg khỏi {name}?',
  'This removes the pygg server you added to {name} by hand. Its token, if you made one, stays valid until you revoke it in Clients.':
    'Thao tác này gỡ server pygg bạn đã tự thêm vào {name}. Token của nó (nếu có) vẫn còn hiệu lực cho tới khi bạn thu hồi trong Client.',
  'This removes the pygg server from {name} and revokes the token Pygg made for it. The configuration file is backed up first. Tasks already started are not affected.':
    'Thao tác này gỡ server pygg khỏi {name} và thu hồi token Pygg đã tạo cho nó. Tệp cấu hình được sao lưu trước. Các task đã chạy không bị ảnh hưởng.',
  'Set Up Pygg in {name}': 'Cài Pygg vào {name}',
  'Send OpenCode’s Models through Pygg': 'Cho model của OpenCode đi qua Pygg',
  'Nothing changes until you choose Set Up.': 'Chưa có gì thay đổi cho tới khi bạn chọn Cài đặt.',
  'Setting Up…': 'Đang cài…',
  'Set Up': 'Cài đặt',
  'Copy {file} into {dir} first.': 'Sao lưu {file} vào {dir} trước.',
  '{file} does not exist yet; it will be created.': '{file} chưa có; tệp sẽ được tạo.',
  'Create a client token, “{client}”, that may start tasks and cannot use any model connection.':
    'Tạo một token client, “{client}”, được tạo task và không dùng được kết nối model nào.',
  'Run {name}’s own command, so {name} edits its configuration itself (the new token in place of <new-token>):':
    'Chạy lệnh của chính {name}, để {name} tự sửa cấu hình của nó (token mới thay cho <new-token>):',
  'Add this entry to {file}, keeping everything else in it (the new token in place of <new-token>):':
    'Thêm mục này vào {file}, giữ nguyên mọi thứ khác trong tệp (token mới thay cho <new-token>):',
  'Change': 'Thay đổi',
  'Create a client token, “{client}”, that reaches your connections like any client; set its limits in Clients.':
    'Tạo một token client, “{client}”, dùng được các kết nối của bạn như mọi client khác; đặt giới hạn cho nó trong Clients.',
  'Check that the pygg provider is there. If anything fails, the file is put back and the token revoked.':
    'Kiểm tra provider pygg đã có trong tệp. Nếu có lỗi, tệp được khôi phục và token bị thu hồi.',
  'OpenCode then lists auto, auto/cheap and the models enabled in Connections under the provider “Pygg”; pick one with /models. Pygg keeps the list current as models are enabled, disabled or looked up; only the list changes. When Pygg runs OpenCode for a task, each request carries the step’s id, so the task shows what Pygg’s ledger recorded for it. OpenCode’s own providers and sign-in are not touched.':
    'Sau đó OpenCode liệt kê auto, auto/cheap và các model đang bật trong Kết nối dưới provider “Pygg”; chọn bằng /models. Pygg tự cập nhật danh sách khi model được bật, tắt hoặc tra cứu thêm thông tin; chỉ danh sách thay đổi. Khi Pygg chạy OpenCode cho một task, mỗi request mang id của bước, nên task hiện đúng số tiền sổ chi của Pygg ghi cho bước đó. Provider và đăng nhập riêng của OpenCode không bị động tới.',
  'OpenCode already has a provider named pygg that Pygg did not make. Pygg leaves it alone.':
    'OpenCode đã có một provider tên pygg không do Pygg tạo. Pygg không động tới nó.',
  'OpenCode can use your connections through Pygg, with auto routing, limits and Pygg’s own record of what a task step cost. {file}':
    'OpenCode có thể dùng các kết nối của bạn qua Pygg, có định tuyến auto, giới hạn chi và sổ chi của Pygg cho từng bước task. {file}',
  'This removes the pygg provider you added to OpenCode by hand. Its token, if you made one, stays valid until you revoke it in Clients.':
    'Thao tác này gỡ provider pygg bạn tự thêm vào OpenCode. Token của nó, nếu có, vẫn dùng được cho tới khi bạn thu hồi trong Clients.',
  'This removes the pygg provider from OpenCode and revokes the token Pygg made for it. The configuration file is backed up first. OpenCode then uses only its own providers.':
    'Thao tác này gỡ provider pygg khỏi OpenCode và thu hồi token Pygg đã tạo cho nó. Tệp cấu hình được sao lưu trước. Sau đó OpenCode chỉ dùng provider riêng của nó.',
  'Check that {name} lists the pygg server. If anything fails, the file is put back and the token revoked.':
    'Kiểm tra {name} đã thấy server pygg. Nếu có lỗi, tệp được khôi phục và token bị thu hồi.',
  'The entry serves all your projects: the server takes the project from the folder your agent runs in. The token is written into {name}’s own configuration for your user, never into a project file. {name}’s own sign-in is not touched.':
    'Mục này dùng cho mọi dự án: server lấy dự án từ thư mục agent đang chạy. Token được ghi vào cấu hình người dùng của {name}, không bao giờ vào tệp của dự án. Đăng nhập của {name} không bị động tới.',
  'Your coding agent (Claude Code, Codex, Cursor…) can hand a subtask to Pygg with the pygg_run_task tool. It runs here, on the agents below, in a separate worktree and against your approved checks; your agent reviews the result and keeps or discards it. Tasks appear in Tasks, where paid steps wait for your approval.':
    'Agent lập trình của bạn (Claude Code, Codex, Cursor…) có thể giao một việc con cho Pygg bằng tool pygg_run_task. Việc đó chạy trên máy này, bằng các agent bên dưới, trong worktree riêng và được kiểm bằng các check bạn đã duyệt; agent của bạn xem kết quả rồi giữ hoặc bỏ. Task hiện trong mục Tasks, nơi bước trả phí chờ bạn duyệt.',
  'In Clients, create a token for your IDE, then open Limits… and turn on “May start tasks”.':
    'Trong Clients, tạo token cho IDE, rồi mở Limits… và bật “Được tạo task”.',
  'Register Pygg as an MCP server with your agent, with that token in place of <token>. The app’s own program serves it:':
    'Đăng ký Pygg làm MCP server với agent của bạn, thay <token> bằng token đó. Chính chương trình của ứng dụng phục vụ MCP:',
  'Cursor and others (JSON)': 'Cursor và các IDE khác (JSON)',
  'Setup': 'Cài đặt',
  'The token ends up in your agent’s configuration file. It can only reach this computer’s gateway, and you can revoke it in Clients.':
    'Token sẽ nằm trong file cấu hình của agent. Nó chỉ dùng được với gateway trên máy này, và bạn có thể thu hồi trong Clients.',
  // Strict limit (ROADMAP B4)
  'strict limit': 'giới hạn nghiêm ngặt',
  'Strict limit': 'Giới hạn nghiêm ngặt',
  'Each request is held at the most it could cost: its own output limit, or all the model can write. Models whose price or maximum output is unknown are not used. The bill can still differ if a provider changes its price.':
    'Mỗi request được giữ chỗ bằng mức tối đa nó có thể tốn: giới hạn output của chính nó, hoặc toàn bộ output model có thể viết. Model chưa rõ giá hoặc chưa rõ output tối đa sẽ không được dùng. Hoá đơn vẫn có thể khác nếu nhà cung cấp đổi giá.',
  'Off: a request without an output limit is estimated at 4,096 output tokens, so a long answer can take the month past the limit.':
    'Tắt: request không đặt giới hạn output được ước tính 4.096 token output, nên một câu trả lời dài có thể làm tháng vượt giới hạn.',
  'Hold the most a request could cost': 'Giữ chỗ bằng mức tối đa request có thể tốn',
  // Cost over time (CostOverTime.tsx)
  "Cost per Hour": "Chi phí theo giờ",
  "Cost per Day": "Chi phí theo ngày",
  "{amount} known": "{amount} đã biết",
  "{n} request": "{n} request",
  "{n} requests": "{n} request",
  "{n} failed": "{n} lỗi",
  "{n} with unknown cost": "{n} chưa rõ chi phí",
  "Has requests with unknown cost, which are not in the bar": "Có request chưa rõ chi phí, không tính trong cột",

  // Added 2026-11-25: the rest of the gateway screens, so the whole interface reads in Vietnamese.
  'Projects': 'Dự án',
  'Create API Key…': 'Tạo API key…',
  'none': 'không có',
  'nothing': 'không có',
  'Enter an amount above 0, or leave it empty.': 'Nhập số tiền lớn hơn 0, hoặc để trống.',
  'Show {n} more': 'Hiện thêm {n}',
  'Saved to {path}': 'Đã lưu vào {path}',
  'Saving…': 'Đang lưu…',
  'No projects yet': 'Chưa có dự án',
  'Open Endpoint': 'Mở Endpoint',
  'Daily spending limit for paid task steps': 'Hạn mức chi hằng ngày cho bước trả phí của tác vụ',
  'Changed': 'Thay đổi',
  'Open Providers': 'Mở Nhà cung cấp',
  'Open CLI Tools': 'Mở Công cụ CLI',
  'Codex': 'Codex',
  'API Key Policies': 'Chính sách API key',
  'No API keys yet. Create one in Endpoint or set up a tool in CLI Tools.':
    'Chưa có API key. Tạo một key trong Endpoint hoặc cài một công cụ trong Công cụ CLI.',
  'Key': 'Key',
  'Min. tier': 'Tier tối thiểu',
  'Unknown prices': 'Giá chưa rõ',
  'Paid retries': 'Thử lại trả phí',
  'tasks': 'tác vụ',
  'strict': 'nghiêm ngặt',
  'estimated': 'ước tính',
  'blocked': 'chặn',
  'allowed': 'cho phép',
  'all': 'tất cả',
  '{n} only': 'chỉ {n}',
  'no training': 'không huấn luyện',
  'Min. tier: the lowest tier auto may choose for that key; auto/cheap ignores it. Unknown prices: whether a model without a price may be used while the key has a monthly limit.':
    'Tier tối thiểu: tier thấp nhất auto được chọn cho key đó; auto/cheap không dùng thiết lập này. Giá chưa rõ: có được dùng model chưa có giá khi key có hạn mức tháng hay không.',
  'Connection Policies': 'Chính sách kết nối',
  'Couldn’t load the connections: {error}': 'Không tải được danh sách kết nối: {error}',
  'Connection': 'Kết nối',
  'Quota': 'Quota',
  'Pool': 'Nhóm',
  'Off': 'Tắt',
  'does not train on requests': 'không huấn luyện trên request',
  'may train': 'có thể huấn luyện',
  'not recorded': 'chưa ghi',
  'own quota, confirmed': 'quota riêng, đã xác nhận',
  'shared with {names}': 'dùng chung với {names}',
  'Shared': 'Dùng chung',
  'assumed': 'giả định',
  'No connections yet.': 'Chưa có kết nối.',
  'Data-use terms are what you recorded from the provider’s terms, not something Pygg can check. Not recorded counts as may train for keys that require no training.':
    'Điều khoản sử dụng dữ liệu là những gì bạn ghi lại từ điều khoản của nhà cung cấp, Pygg không kiểm tra được. Với key yêu cầu không huấn luyện, “chưa ghi” được coi là “có thể huấn luyện”.',
  'Always On': 'Luôn bật',
  'The gateway listens on 127.0.0.1 only: other devices cannot reach it, and that is not a setting.':
    'Gateway chỉ lắng nghe trên 127.0.0.1: thiết bị khác không truy cập được, và đây không phải là một tùy chọn.',
  'Provider keys and API-key copies stay in the Windows Credential Manager; the store keeps only fingerprints.':
    'Key của nhà cung cấp và bản sao API key được giữ trong Windows Credential Manager; kho dữ liệu chỉ lưu dấu vân tay.',
  'A cost Pygg cannot know is unknown, never $0, and is left out of totals.':
    'Chi phí Pygg không thể biết là chưa rõ, không bao giờ là $0, và không được tính vào tổng.',
  'A paid task step always waits for your approval in the app; no API can approve one.':
    'Bước trả phí của tác vụ luôn chờ bạn duyệt trong ứng dụng; không API nào duyệt thay được.',
  'No telemetry: Pygg reports nothing about your use to anyone. A request goes only to the provider it is routed to.':
    'Không telemetry: Pygg không báo gì về việc bạn dùng cho bất kỳ ai. Request chỉ đi tới nhà cung cấp mà nó được định tuyến tới.',
  'on hold': 'tạm hoãn',
  'All tasks together; resets at 00:00 UTC.': 'Tính chung mọi tác vụ; đặt lại lúc 00:00 UTC.',
  'Change in Settings': 'Đổi trong Cài đặt',
  'Checks, privacy and budget per workspace': 'Check, riêng tư và ngân sách theo dự án',
  'Open Workspaces': 'Mở Dự án',
  'Couldn’t load the defaults: {error}': 'Không tải được thiết lập mặc định: {error}',
  'Saved. Keys created from now on get these.': 'Đã lưu. Key tạo từ giờ sẽ nhận các thiết lập này.',
  'Defaults for New API Keys': 'Mặc định cho API key mới',
  'Applied when a key is created: in Endpoint, by pygg gateway create-client, or by a tool’s set-up in CLI Tools. Existing keys keep their saved settings; change them below.':
    'Áp dụng khi tạo key: trong Endpoint, bằng pygg gateway create-client, hoặc khi cài một công cụ trong Công cụ CLI. Key đã có giữ thiết lập đã lưu; đổi chúng ở bên dưới.',
  'Minimum model tier': 'Tier model tối thiểu',
  'The lowest tier auto may choose. T2 suits coding agents; lower lets cheaper, weaker models answer. auto/cheap and combos ignore it.':
    'Tier thấp nhất auto được chọn. T2 hợp với agent lập trình; thấp hơn thì model rẻ hơn, yếu hơn cũng được trả lời. auto/cheap và combo không dùng thiết lập này.',
  'Empty: no limit. A limit counts known spend and spend that may have been charged.':
    'Để trống: không giới hạn. Hạn mức tính cả chi tiêu đã biết và khoản có thể đã bị tính phí.',
  'Limit mode': 'Chế độ hạn mức',
  'Strict reserves the most a request could cost before sending it; a model without a price or maximum output is not used. Estimated counts requests as they finish, so one last request can pass the limit. Provider price changes or unreported charges can make the bill differ either way.':
    'Nghiêm ngặt: giữ chỗ mức tối đa một request có thể tốn trước khi gửi; model chưa có giá hoặc chưa rõ output tối đa sẽ không được dùng. Ước tính: tính request khi chúng xong, nên request cuối cùng có thể vượt hạn mức. Nhà cung cấp đổi giá hoặc có khoản phí không được báo có thể làm hoá đơn chênh lệch theo cả hai chiều.',
  'Strict': 'Nghiêm ngặt',
  'When to choose which': 'Khi nào chọn chế độ nào',
  'A strict limit holds each request at the most it could cost (output limit × price): Claude Code on a large repository is held at over $1 a request, so prefer the estimated limit there.':
    'Giới hạn nghiêm ngặt giữ chỗ mỗi request bằng mức tối đa nó có thể tốn (giới hạn đầu ra × giá): Claude Code trên repository lớn bị giữ hơn $1 mỗi request, nên ở đó hãy dùng giới hạn ước tính.',
  'While a key has a limit: block keeps it from models without a price, whose cost could not be counted.':
    'Khi key có hạn mức: chặn sẽ không cho key dùng model chưa có giá, vì chi phí của chúng không tính được.',
  'Further models tried after a timeout or server error the provider may have billed. 0 stops there.':
    'Số model thử thêm sau khi quá thời gian hoặc lỗi máy chủ mà nhà cung cấp có thể đã tính phí. 0 là dừng ở đó.',
  'Requests at once': 'Request cùng lúc',
  'Only providers that do not train': 'Chỉ nhà cung cấp không huấn luyện',
  'Uses only connections whose recorded terms say they do not train on requests.':
    'Chỉ dùng các kết nối có điều khoản đã ghi là không huấn luyện trên request.',
  'Save Defaults': 'Lưu mặc định',
  'no pauses cost a cache': 'không quãng nghỉ nào làm mất cache',
  'could save {amount}': 'có thể tiết kiệm {amount}',
  'would cost more than it saves': 'sẽ tốn hơn phần tiết kiệm được',
  'Cache Lifetime': 'Thời hạn cache',
  'measured only · {verdict}': 'chỉ đo lường · {verdict}',
  'Nothing lost': 'Không mất gì',
  'net, with a one-hour cache lifetime': 'ròng, với thời hạn cache một giờ',
  'net, with a keep-alive': 'ròng, với keep-alive',
  'Reading their ≈{n} tokens instead of writing them again: {amount} less':
    'Đọc ≈{n} token của chúng thay vì ghi lại: bớt {amount}',
  'One-hour lifetime: {extra} more on the other cache writes, {net} net':
    'Thời hạn một giờ: tốn thêm {extra} cho các lần ghi cache khác, ròng {net}',
  'Measured only, on models that bill cache writes, such as Claude: their prompt cache lasts five minutes, so an answer after a longer pause writes the whole conversation again. A one-hour lifetime would make every cache write cost twice the input price instead of 1.25 times; a keep-alive would re-send each session’s last request every 4.5 minutes for up to an hour, also after the last answer of a session. Pygg will offer one only if it stays above zero.':
    'Chỉ đo lường, trên model tính phí ghi cache, như Claude: prompt cache của chúng tồn tại năm phút, nên câu trả lời sau một quãng nghỉ lâu hơn sẽ ghi lại toàn bộ hội thoại. Thời hạn một giờ sẽ khiến mỗi lần ghi cache tốn gấp đôi giá đầu vào thay vì 1,25 lần; keep-alive sẽ gửi lại request cuối của mỗi phiên mỗi 4,5 phút trong tối đa một giờ, kể cả sau câu trả lời cuối của phiên. Pygg chỉ đề xuất một trong hai nếu con số này vẫn trên 0.',
  '{n} answer missed the cache after a pause of 5 to 60 minutes':
    '{n} câu trả lời bị trượt cache sau quãng nghỉ 5 đến 60 phút',
  '{n} answers missed the cache after a pause of 5 to 60 minutes':
    '{n} câu trả lời bị trượt cache sau quãng nghỉ 5 đến 60 phút',
  'of {n} measured': 'trên {n} đã đo',
  '{n} answer on a model without a cached price is not in the money':
    '{n} câu trả lời trên model chưa có giá cache không được tính vào số tiền',
  '{n} answers on models without a cached price are not in the money':
    '{n} câu trả lời trên model chưa có giá cache không được tính vào số tiền',
  '{n} tokens': '{n} token',
  'Requests per Hour': 'Yêu cầu theo giờ',
  'Requests per Day': 'Yêu cầu theo ngày',
  'Tokens per Hour': 'Token theo giờ',
  'Tokens per Day': 'Token theo ngày',
  'Comparison per Hour': 'So sánh theo giờ',
  'Comparison per Day': 'So sánh theo ngày',
  'Chart metric': 'Chỉ số biểu đồ',
  'Comparison': 'So sánh',
  '{amount} through Pygg': '{amount} qua Pygg',
  '{in} in → {out} out': '{in} vào → {out} ra',
  'The same tokens at {model}': 'Cùng số token đó với {model}',
  'What they cost through Pygg': 'Chi phí khi qua Pygg',
  'compared answers only; failed attempts and unknown costs are not in the bars':
    'chỉ các câu trả lời được so sánh; lần thử thất bại và chi phí chưa rõ không có trong cột',
  'No answer compared': 'Chưa có câu trả lời nào được so sánh',
  '≈{amount} at {model}': '≈{amount} với {model}',
  '≈{amount} less': 'ít hơn ≈{amount}',
  '≈{amount} more': 'nhiều hơn ≈{amount}',
  '{n} answer compared': '{n} câu trả lời được so sánh',
  '{n} answers compared': '{n} câu trả lời được so sánh',
  'Experimental agent execution. To route your coding tool’s model requests through Pygg, use CLI Tools.':
    'Chạy agent thử nghiệm. Để định tuyến request model của công cụ lập trình qua Pygg, hãy dùng Công cụ CLI.',
  'Check Again': 'Kiểm tra lại',
  'Looking for agents: asking each one for its version and sign-in…':
    'Đang tìm agent: hỏi từng agent về phiên bản và đăng nhập…',
  'Up to 15 minutes. The run uses the agent’s own account; an API-key sign-in may incur provider charges Pygg may not know.':
    'Tối đa 15 phút. Lần chạy dùng tài khoản riêng của agent; đăng nhập bằng API key có thể phát sinh phí từ nhà cung cấp mà Pygg không biết.',
  'Let Your Agent Delegate Tasks': 'Cho agent của bạn giao tác vụ',
  'The token ends up in your agent’s configuration file. It can only reach this computer’s gateway, and you can revoke it in Endpoint → API Keys.':
    'Token sẽ nằm trong tệp cấu hình của agent. Nó chỉ dùng được với gateway trên máy này, và bạn có thể thu hồi trong Endpoint → API key.',
  'Checking what is set up in each tool…': 'Đang kiểm tra cấu hình trong từng công cụ…',
  'Codex already has a model provider named pygg that Pygg did not make. Pygg leaves it alone.':
    'Codex đã có một model provider tên pygg không do Pygg tạo. Pygg không động vào nó.',
  'Codex can use your connections through Pygg, with auto routing, limits and Pygg’s own record of each request’s cost. Your default model stays; you choose Pygg per run. {file}':
    'Codex có thể dùng các kết nối của bạn qua Pygg, có định tuyến auto, hạn mức và sổ chi của Pygg ghi chi phí từng request. Model mặc định của bạn giữ nguyên; bạn chọn Pygg cho từng lần chạy. {file}',
  'OpenCode can use your connections through Pygg, with auto routing, limits and Pygg’s own record of each request’s cost. {file}':
    'OpenCode có thể dùng các kết nối của bạn qua Pygg, có định tuyến auto, hạn mức và sổ chi của Pygg ghi chi phí từng request. {file}',
  'Start it through Pygg; each launch keeps its own routing session:':
    'Khởi chạy qua Pygg; mỗi lần chạy giữ phiên định tuyến riêng:',
  'Launch command': 'Lệnh khởi chạy',
  'This removes the pygg model provider you added to Codex by hand, and pygg.config.toml if it uses it. Its token, if you made one, stays valid until you revoke it in Endpoint → API Keys.':
    'Thao tác này gỡ model provider pygg bạn đã tự thêm vào Codex, và cả pygg.config.toml nếu nó dùng tệp này. Token của nó, nếu có, vẫn dùng được cho tới khi bạn thu hồi trong Endpoint → API key.',
  'This removes the pygg model provider from Codex’s config.toml and the profile file pygg.config.toml, and revokes the token Pygg made for it. Both files are backed up first; everything else in config.toml stays as it is.':
    'Thao tác này gỡ model provider pygg khỏi config.toml của Codex và tệp profile pygg.config.toml, đồng thời thu hồi token Pygg đã tạo cho nó. Cả hai tệp được sao lưu trước; mọi thứ khác trong config.toml giữ nguyên.',
  'This removes the pygg provider you added to OpenCode by hand. Its token, if you made one, stays valid until you revoke it in Endpoint → API Keys.':
    'Thao tác này gỡ provider pygg bạn tự thêm vào OpenCode. Token của nó, nếu có, vẫn dùng được cho tới khi bạn thu hồi trong Endpoint → API key.',
  'Send Codex’s Models through Pygg': 'Cho model của Codex đi qua Pygg',
  'Create a Pygg API key, “{client}”, that reaches your connections like any other; set its limits in Endpoint → API Keys.':
    'Tạo một API key Pygg, “{client}”, dùng được các kết nối của bạn như mọi key khác; đặt hạn mức cho nó trong Endpoint → API key.',
  'Add this provider at the end of {file}, keeping every other setting and comment in it, and create the profile file pygg.config.toml next to it (the new token in place of <new-token>):':
    'Thêm provider này vào cuối {file}, giữ nguyên mọi thiết lập và chú thích khác trong tệp, rồi tạo tệp profile pygg.config.toml bên cạnh (token mới thay cho <new-token>):',
  'Your default model and provider are not changed. Run pygg run codex to use Pygg with auto (or codex --profile pygg; add -m <connection>/<model> for a model from Connections). Through pygg run, each launch has a routing session of its own, so the gateway keeps it on the model that answered it, and its prompt cache; when Pygg runs Codex for a task, the session is the task step. Codex’s own sign-in is not touched.':
    'Model và provider mặc định của bạn không bị thay đổi. Chạy pygg run codex để dùng Pygg với auto (hoặc codex --profile pygg; thêm -m <connection>/<model> để dùng một model trong Nhà cung cấp). Qua pygg run, mỗi lần chạy có phiên định tuyến riêng, nên gateway giữ nó trên model đã trả lời và giữ cả prompt cache; khi Pygg chạy Codex cho một tác vụ, phiên chính là bước của tác vụ. Đăng nhập riêng của Codex không bị động tới.',
  'OpenCode then lists auto, auto/cheap and the models enabled in Providers under the provider “Pygg”; pick one with /models. Pygg keeps the list current as models are enabled, disabled or looked up; only the list changes. Start it with pygg run opencode and each launch has a routing session of its own, so the gateway keeps it on the model that answered it, and its prompt cache; when Pygg runs OpenCode for a task, the session is the task step. OpenCode’s own providers and sign-in are not touched.':
    'Sau đó OpenCode liệt kê auto, auto/cheap và các model đang bật trong Nhà cung cấp dưới provider “Pygg”; chọn bằng /models. Pygg tự cập nhật danh sách khi model được bật, tắt hoặc tra cứu thêm thông tin; chỉ danh sách thay đổi. Khởi chạy bằng pygg run opencode thì mỗi lần chạy có phiên định tuyến riêng, nên gateway giữ nó trên model đã trả lời và giữ cả prompt cache; khi Pygg chạy OpenCode cho một tác vụ, phiên chính là bước của tác vụ. Nhà cung cấp và đăng nhập riêng của OpenCode không bị động tới.',
  'Billing unknown': 'Chưa rõ cách tính phí',
  'auto (Pygg chose)': 'auto (Pygg chọn)',
  'auto/cheap (Pygg chose the cheapest)': 'auto/cheap (Pygg chọn rẻ nhất)',
  'A combo (your order, Pygg’s pick)': 'Combo (thứ tự của bạn, Pygg chọn)',
  'A model by name (your choice)': 'Model theo tên (bạn chọn)',
  'Model to compare with': 'Model để so sánh',
  'Estimated savings': 'Tiết kiệm ước tính',
  'estimated savings': 'tiết kiệm ước tính',
  'estimated extra cost': 'chi phí thêm ước tính',
  '{n}% less': 'ít hơn {n}%',
  '{n} of {total} answered requests compared with {model}: the same tokens priced at it, against what they cost through Pygg.':
    '{n} trên {total} request đã trả lời được so với {model}: cùng số token tính theo giá model này, đối chiếu với chi phí qua Pygg.',
  'Priced at {model}': 'Tính theo giá {model}',
  'Cost through Pygg': 'Chi phí qua Pygg',
  'Estimated difference': 'Chênh lệch ước tính',
  'Estimated extra cost': 'Chi phí thêm ước tính',
  'See what Pygg saves you': 'Xem Pygg giúp bạn tiết kiệm bao nhiêu',
  'Choose the model you would otherwise use; Pygg prices the same requests at it and shows the difference.':
    'Chọn model bạn sẽ dùng nếu không có Pygg; Pygg tính giá các request đó theo model này và cho thấy chênh lệch.',
  '{model} has no price, so nothing can be compared. Add a price in Providers or choose another model.':
    '{model} chưa có giá, nên không so sánh được. Thêm giá trong Nhà cung cấp hoặc chọn model khác.',
  'No answered request in this period has a known cost and token counts yet.':
    'Chưa có request nào được trả lời trong khoảng này có chi phí và số token đã biết.',
  'Compare with {model}': 'So với {model}',
  'your most-used paid model, asked for by name in this period':
    'model trả phí bạn dùng nhiều nhất, gọi theo tên trong khoảng này',
  'How this estimate works': 'Cách ước tính này hoạt động',
  'The same token counts and cache split, at list prices. Another model would have written a different amount.':
    'Cùng số token và cách chia cache, theo giá niêm yết. Một model khác sẽ viết ra lượng khác.',
  'Cost through Pygg is what the compared answers cost, plus failed attempts with a known cost in the period.':
    'Chi phí qua Pygg là chi phí của các câu trả lời được so sánh, cộng các lần thử thất bại đã biết chi phí trong khoảng này.',
  'Here: {answers} for the answers and {failed} for failed attempts.':
    'Ở đây: {answers} cho các câu trả lời và {failed} cho các lần thử thất bại.',
  'A subscription is a plan, not a price per request: its answers are counted, never priced. Local runs exclude electricity and hardware.':
    'Gói thuê bao là một gói, không phải giá theo từng request: câu trả lời của nó được đếm, không bao giờ được quy ra tiền. Lần chạy cục bộ không tính tiền điện và phần cứng.',
  'Billing and prices are read from the current settings of each connection, so a changed setting changes past figures too.':
    'Cách tính phí và giá được đọc từ thiết lập hiện tại của từng kết nối, nên đổi thiết lập thì số liệu cũ cũng đổi theo.',
  'A request that went to a free, local or plan model used what you already have; that is not proof you would have paid for it otherwise.':
    'Request đi tới model miễn phí, cục bộ hoặc theo gói là dùng thứ bạn đã có; điều đó không chứng minh rằng nếu không có Pygg thì bạn đã phải trả tiền cho nó.',
  'Change comparison': 'Đổi model so sánh',
  'Where the answers came from': 'Câu trả lời đến từ đâu',
  '{n} of {total} from free, local or plan models': '{n} trên {total} từ model miễn phí, cục bộ hoặc theo gói',
  'Compared / answered': 'Đã so / đã trả lời',
  'Through Pygg': 'Qua Pygg',
  'At {model}': 'Theo {model}',
  'Difference': 'Chênh lệch',
  'Comparison Breakdown': 'Chi tiết so sánh',
  'Comparison by client': 'So sánh theo client',
  'By client': 'Theo client',
  'Comparison by how the model was asked for': 'So sánh theo cách gọi model',
  'By how the model was asked for': 'Theo cách gọi model',
  'Answers through auto and combos are Pygg’s choice. A model asked for by name is yours: its difference is listed, not claimed.':
    'Câu trả lời qua auto và combo là do Pygg chọn. Model gọi theo tên là lựa chọn của bạn: chênh lệch của nó được liệt kê, không tính là công của Pygg.',
  '{n} fallback attempts in the period: a model failed and the next one was tried. A fallback keeps a request alive; it is not in itself a saving.':
    '{n} lần chuyển model trong khoảng này: một model thất bại và model kế tiếp được thử. Chuyển model giúp request vẫn được trả lời; bản thân nó không phải là khoản tiết kiệm.',
  'Failed attempts with a known cost are in the total above, not in these rows.':
    'Các lần thử thất bại đã biết chi phí nằm trong tổng ở trên, không nằm trong các dòng này.',
  '{n} answer with unknown cost excluded': 'Loại {n} câu trả lời chưa rõ chi phí',
  '{n} answers with unknown cost excluded': 'Loại {n} câu trả lời chưa rõ chi phí',
  '{n} answer without token counts excluded': 'Loại {n} câu trả lời không có số token',
  '{n} answers without token counts excluded': 'Loại {n} câu trả lời không có số token',
  '{n} subscription answer not compared (a plan, not a price)':
    '{n} câu trả lời từ gói thuê bao không được so sánh (là gói, không phải giá)',
  '{n} subscription answers not compared (a plan, not a price)':
    '{n} câu trả lời từ gói thuê bao không được so sánh (là gói, không phải giá)',
  '{n} failed attempt may have been billed at an unknown cost, which no figure here includes.':
    '{n} lần thử thất bại có thể đã bị tính phí với chi phí chưa rõ, không nằm trong số liệu nào ở đây.',
  '{n} failed attempts may have been billed at an unknown cost, which no figure here includes.':
    '{n} lần thử thất bại có thể đã bị tính phí với chi phí chưa rõ, không nằm trong số liệu nào ở đây.',
  '{name} Template': 'Mẫu {name}',
  'New Combo from a Template': 'Combo mới từ mẫu',
  'This creates a saved list from your models as they are now. Models you connect later are not added by themselves.':
    'Thao tác này tạo một danh sách cố định từ các model hiện có của bạn. Model kết nối sau này sẽ không tự được thêm vào.',
  'Each template picks members from your connected models by rules: billing, tier, tools and context.':
    'Mỗi mẫu chọn thành viên từ các model đã kết nối theo quy tắc: tính phí, tier, tool và context.',
  'Other Templates': 'Mẫu khác',
  'OpenAI-format clients cannot use Anthropic-format connections; Claude Code and Anthropic SDKs can use every connection.':
    'Client định dạng OpenAI không dùng được kết nối định dạng Anthropic; Claude Code và SDK Anthropic dùng được mọi kết nối.',
  'Claude Code / Anthropic': 'Claude Code / Anthropic',
  'Requirements': 'Yêu cầu',
  'Unknown counts as missing: a model whose tools or tier are not known is left out.':
    'Chưa rõ thì coi như không có: model chưa rõ hỗ trợ tool hoặc tier sẽ bị loại.',
  'Takes tools': 'Hỗ trợ tool',
  'Tier at least': 'Tier tối thiểu',
  'Include free and local models without a tier': 'Gồm cả model miễn phí và cục bộ chưa có tier',
  'No free, local or plan model qualified, so this would be paid models only.':
    'Không có model miễn phí, cục bộ hay theo gói nào đạt yêu cầu, nên combo này sẽ chỉ gồm model trả phí.',
  'Pygg won’t create a “{name}” combo that always costs money.': 'Pygg sẽ không tạo combo “{name}” luôn tốn tiền.',
  '{stage} models left out: {reasons}.': 'Model {stage} bị loại: {reasons}.',
  'You have no free, local or plan models connected.': 'Bạn chưa kết nối model miễn phí, cục bộ hay theo gói nào.',
  'Allow tier {tier}': 'Cho phép tier {tier}',
  'Include models without a tier': 'Gồm cả model chưa có tier',
  'Don’t require tools': 'Không yêu cầu tool',
  'Connect a Free Provider…': 'Kết nối nhà cung cấp miễn phí…',
  'Many free models are listed without their tier or tool support: Providers → Check & Refresh Models, or pygg gateway lookup, fills in what is known. A lower tier lets weaker models answer; without tools, coding agents may fail.':
    'Nhiều model miễn phí được liệt kê mà không có tier hay thông tin hỗ trợ tool: Nhà cung cấp → Kiểm tra & làm mới model, hoặc pygg gateway lookup, sẽ điền những gì đã biết. Tier thấp hơn cho phép model yếu hơn trả lời; không có tool, agent lập trình có thể thất bại.',
  '≈{amount} per reference request': '≈{amount} mỗi request tham chiếu',
  'One member matched: this combo has no backup.': 'Chỉ một thành viên khớp: combo này không có dự phòng.',
  'Nothing matched for: {stages}.': 'Không có gì khớp cho: {stages}.',
  'Prices are compared on a reference request of {input} tokens in and {output} out.':
    'Giá được so sánh trên một request tham chiếu gồm {input} token vào và {output} token ra.',
  'None of your models matches this template.': 'Không model nào của bạn khớp mẫu này.',
  'Connect a provider, check & refresh models so their facts are known, or change the requirements above. Pygg never lowers them or adds a paid model by itself.':
    'Hãy kết nối nhà cung cấp, kiểm tra & làm mới model để biết thông tin của chúng, hoặc đổi các yêu cầu ở trên. Pygg không bao giờ tự hạ yêu cầu hay tự thêm model trả phí.',
  'Clients send this as the model.': 'Client gửi tên này ở chỗ tên model.',
  '{n} more model qualified; a combo keeps the first 6.': 'Còn {n} model đạt yêu cầu; combo chỉ giữ 6 model đầu.',
  '{n} more models qualified; a combo keeps the first 6.': 'Còn {n} model đạt yêu cầu; combo chỉ giữ 6 model đầu.',
  'Connection added. Next, set up your coding tool; CLI Tools says whether a model qualifies for it.':
    'Đã thêm kết nối. Tiếp theo, cài đặt công cụ lập trình của bạn; Công cụ CLI cho biết model nào đủ điều kiện.',
  'Not now': 'Để sau',
  '{n} checked': 'Đã kiểm tra {n}',
  '{n} web sessions not included: check each with Check Sign-In':
    'Không gồm {n} phiên web: kiểm tra từng phiên bằng Kiểm tra đăng nhập',
  'Reads each API connection’s model list (free, no prompt). Web sessions are not included, so no site sees a program loading its pages.':
    'Đọc danh sách model của từng kết nối API (miễn phí, không gửi prompt). Không gồm phiên web, để không trang web nào thấy chương trình tải trang của nó.',
  'Check All': 'Kiểm tra tất cả',
  'No requests or checks in the last 24 hours': 'Không có request hay lần kiểm tra nào trong 24 giờ qua',
  'Health {score} of 100 over the last 24 hours: {ok} went well, {failed} failed, failures in {hours} of 24 hours':
    'Tình trạng {score}/100 trong 24 giờ qua: {ok} thành công, {failed} thất bại, có lỗi trong {hours}/24 giờ',
  '{ok} ok · {failed} failed': '{ok} ok · {failed} lỗi',
  'Warning not acknowledged': 'Chưa xác nhận cảnh báo',
  'Checked {ago}': 'Đã kiểm tra {ago}',
  'Not checked yet': 'Chưa kiểm tra',
  '{used} of {limit} requests sent today (UTC). At the limit Pygg sends nothing until 00:00 UTC.':
    'Đã gửi {used}/{limit} request hôm nay (UTC). Khi chạm giới hạn, Pygg không gửi gì cho tới 00:00 UTC.',
  'This site takes a daily limit of 1 to {max} requests.': 'Trang web này nhận giới hạn ngày từ 1 tới {max} request.',
  'This hour: {units} of {unitsCap} units, {requests} of {requestsCap} requests. Last {window} min: {burst} of {burstCap} requests. Today: {today} of {todayCap} units. A unit is a request, and one more for each 2,000 characters of prompt; a prompt over {max} characters is not sent. After each reply Pygg waits at least {rest} s, longer for a long prompt or reply (at most 3 minutes); it slows Pygg down, and does not make an account safe.':
    'Giờ này: {units}/{unitsCap} đơn vị, {requests}/{requestsCap} request. {window} phút qua: {burst}/{burstCap} request. Hôm nay: {today}/{todayCap} đơn vị. Một đơn vị là một request, cộng thêm một cho mỗi 2.000 ký tự prompt; prompt dài hơn {max} ký tự sẽ không được gửi. Sau mỗi câu trả lời Pygg chờ ít nhất {rest} giây, lâu hơn với prompt hoặc câu trả lời dài (tối đa 3 phút); việc này làm Pygg chậm lại, và không bảo đảm tài khoản an toàn.',
  'These limits are always on and cannot be raised. When the site itself warns (a usage limit, "too many requests", a check), Pygg rests the account for at least half an hour, an hour after a usage limit, and longer each time in a row.':
    'Các giới hạn này luôn bật và không thể nâng. Khi chính trang web cảnh báo (hạn mức sử dụng, "too many requests", một bước kiểm tra), Pygg cho tài khoản nghỉ ít nhất nửa giờ, một giờ sau khi chạm hạn mức sử dụng, và lâu hơn mỗi lần liên tiếp.',
  'Resting until {time}.': 'Nghỉ tới {time}.',
  'Backed off until {time}: {reason}': 'Tạm ngưng tới {time}: {reason}',
  'Resume Now': 'Tiếp tục ngay',
  'Signed in': 'Đã đăng nhập',
  'Not signed in': 'Chưa đăng nhập',
  'Needs you': 'Cần bạn xử lý',
  'Page changed': 'Trang đã thay đổi',
  '{n} for auto': '{n} cho auto',
  'Couldn’t save billing: {error}': 'Không lưu được cách tính phí: {error}',
  'The last 24 hours, an hour a dot: its answers and sign-in checks. A web session is checked only when you ask, and not while the web guard has it resting, so its site sees no program loading pages.':
    '24 giờ qua, mỗi chấm là một giờ: các câu trả lời và các lần kiểm tra đăng nhập. Phiên web chỉ được kiểm tra khi bạn yêu cầu, và không kiểm tra khi web guard đang cho nó nghỉ, nên trang web không thấy chương trình nào tải trang.',
  'The last 24 hours, an hour a dot: its answers and Pygg’s checks. Pygg reads its model list every 30 minutes while the gateway runs, which costs nothing and sends no prompt.':
    '24 giờ qua, mỗi chấm là một giờ: các câu trả lời của nó và các lần kiểm tra của Pygg. Pygg đọc danh sách model của nó mỗi 30 phút khi gateway chạy, không tốn tiền và không gửi prompt nào.',
  'Each hour scores its own share that went well, recent hours counting more; an hour with requests is judged by them alone, an hour without by its checks. Requests Pygg refused itself, and requests the provider refused as malformed, are not counted.':
    'Mỗi giờ được chấm theo tỷ lệ thành công của chính nó, giờ gần đây tính nặng hơn; giờ có request chỉ xét theo request, giờ không có thì xét theo các lần kiểm tra. Không tính request do Pygg tự từ chối, và request bị nhà cung cấp từ chối vì sai định dạng.',
  'checked {ago}': 'đã kiểm tra {ago}',
  'Not checked yet.': 'Chưa kiểm tra.',
  'Check Sign-In': 'Kiểm tra đăng nhập',
  'Check Now': 'Kiểm tra ngay',
  'Read from the headers of its last answer for each model. While a limit is used up, auto skips that model until it resets; a provider that sends none shows nothing here.':
    'Đọc từ header của câu trả lời gần nhất cho từng model. Khi một giới hạn đã dùng hết, auto bỏ qua model đó cho tới khi giới hạn đặt lại; nhà cung cấp nào không gửi thì ở đây trống.',
  'Counted against your plan’s quota: each request’s cost is Unknown, never money.':
    'Tính vào quota của gói: chi phí mỗi request là Chưa rõ, không bao giờ là tiền.',
  'Your plan with the site, paid by the month or free, never per request: each request is recorded at $0.':
    'Gói của bạn với trang web, trả theo tháng hoặc miễn phí, không bao giờ theo từng request: mỗi request được ghi là $0.',
  'Advanced: endpoint, API format, quota, pool, data use':
    'Nâng cao: endpoint, định dạng API, quota, nhóm, sử dụng dữ liệu',
  'Another account on the site': 'Một tài khoản khác trên trang web',
  'Turn on if this connection is signed in to a different account from your other {provider} connections. Then it can join a pool with them; each keeps its own daily limit and pace.':
    'Bật nếu kết nối này đăng nhập vào tài khoản khác với các kết nối {provider} còn lại. Khi đó nó có thể vào nhóm cùng chúng; mỗi kết nối giữ giới hạn ngày và nhịp riêng.',
  'Couldn’t save the quota setting: {error}': 'Không lưu được thiết lập quota: {error}',
  'Auto, for a coding tool’s request': 'Auto, cho request của công cụ lập trình',
  'Eligible for auto': 'Đủ điều kiện cho auto',
  'Missing auto facts': 'Thiếu thông tin cho auto',
  'Unavailable to auto now': 'Tạm không khả dụng cho auto',
  'Outside auto’s rules': 'Ngoài luật của auto',
  'Price per 1M tokens': 'Giá/1M token',
  'Remove the key “{name}”?': 'Gỡ key “{name}”?',
  'Remove Key': 'Gỡ key',
  'It is deleted from {store}, and this connection stops using it. Usage already recorded is kept, with its name.':
    'Key bị xóa khỏi {store}, và kết nối này thôi dùng nó. Mức dùng đã ghi nhận vẫn được giữ, kèm tên key.',
  'It is the connection’s own key: the next key takes its place for model lists and checks.':
    'Đây là key riêng của kết nối: key kế tiếp sẽ thay nó để lấy danh sách model và kiểm tra.',
  'Its window closes and its profile folder, with your sign-in to the site, is deleted from this computer. Its models leave every combo, and requests for {id}/… stop working.':
    'Cửa sổ của nó đóng lại và thư mục hồ sơ, cùng phiên đăng nhập trang web của bạn, bị xóa khỏi máy này. Các model của nó bị bỏ khỏi mọi combo, và request tới {id}/… sẽ không còn hoạt động.',
  'Requests for {pool}/<model> try these connections in this order, and move to the next when one is rate-limited, out of credit or refuses its key. Each is a separate account you confirmed.':
    'Request tới {pool}/<model> thử các kết nối này theo thứ tự, và chuyển sang kết nối kế tiếp khi một kết nối bị giới hạn tốc độ, hết credit hoặc từ chối key. Mỗi kết nối là một tài khoản riêng mà bạn đã xác nhận.',
  'Group this connection with other {provider} accounts you hold, to reach them as one name. Only accounts with their own quota can join; several keys of one account cannot.':
    'Gộp kết nối này với các tài khoản {provider} khác của bạn, để gọi chúng bằng một tên. Chỉ tài khoản có quota riêng mới tham gia được; nhiều key của cùng một tài khoản thì không.',
  'To pool this connection with other {provider} accounts, first confirm that it has its own provider quota (below).':
    'Để gộp kết nối này với các tài khoản {provider} khác, trước hết hãy xác nhận nó có quota riêng ở nhà cung cấp (bên dưới).',
  'Earlier': 'Lên trước',
  'Later': 'Xuống sau',
  'Leave Pool': 'Rời nhóm',
  'Join {pool}': 'Vào {pool}',
  'new-pool-name': 'ten-nhom-moi',
  'New pool name': 'Tên nhóm mới',
  'Create Pool': 'Tạo nhóm',
  '{amount} left': 'còn {amount}',
  'the provider says paid calls are not possible now': 'nhà cung cấp cho biết hiện không thể gọi trả phí',
  '{amount} used today': 'hôm nay đã dùng {amount}',
  'Account, as the provider says': 'Tài khoản, theo nhà cung cấp',
  'Read from {source} {when}. Pygg asks again at start-up and when you check the models.':
    'Đọc từ {source} {when}. Pygg hỏi lại khi khởi động và khi bạn kiểm tra model.',
  'The provider can say what the account has left. Nothing has been read yet.':
    'Nhà cung cấp có thể cho biết tài khoản còn lại bao nhiêu. Chưa đọc được gì.',
  'Nothing stated': 'Không có thông tin',
  'Send one short request to {model}? It asks for one word and may incur a small charge at its price. The result appears in Usage.':
    'Gửi một request ngắn tới {model}? Request yêu cầu một từ và có thể bị tính một khoản nhỏ theo giá của model. Kết quả hiện trong Mức dùng.',
  'Send one short request to {model}? This may incur a charge. Pygg cannot estimate it because this model has no recorded price. The result appears in Usage.':
    'Gửi một request ngắn tới {model}? Việc này có thể bị tính phí. Pygg không ước tính được vì model này chưa có giá. Kết quả hiện trong Mức dùng.',
  'Test: answered “{text}” in {ms} ms': 'Kiểm tra: trả lời “{text}” trong {ms} ms',
  'Test failed: {reason}': 'Kiểm tra thất bại: {reason}',
  'Answered in {ms} ms': 'Trả lời trong {ms} ms',
  'Send one short request to this model': 'Gửi một request ngắn tới model này',
  'Testing…': 'Đang thử…',
  'Test': 'Thử',
  'Off: makes media': 'Tắt: model tạo media',
  'The model catalog says this model makes audio, video or images, priced per item rather than per token, so Pygg turned it off. Turn it on if you mean to use it.':
    'Danh mục model cho biết model này tạo âm thanh, video hoặc ảnh, tính giá theo từng sản phẩm thay vì theo token, nên Pygg đã tắt nó. Bật lên nếu bạn định dùng.',
  'From the ledger over the last 30 days; requests the client left are not counted.':
    'Từ sổ chi trong 30 ngày qua; không tính request mà client đã ngắt.',
  '{rate}% of {n} answered': 'Trả lời được {rate}% trong {n} request',
  'median {s} s': 'trung vị {s} giây',
  'Not saved: {error}': 'Chưa lưu: {error}',
  'in': 'vào',
  'out': 'ra',
  'Plan quota': 'Quota của gói',
  'cost Unknown': 'chi phí chưa rõ',
  'More actions for {model}': 'Thao tác khác cho {model}',
  'Auto Routing Facts…': 'Thông tin định tuyến auto…',
  'Remove Model': 'Gỡ model',
  'Web session (unofficial)': 'Phiên web (không chính thức)',
  'A site you use by signing in, in a Pygg window, instead of with an API key. It can put your account at risk.':
    'Một trang web bạn dùng bằng cách đăng nhập, trong cửa sổ Pygg, thay vì bằng API key. Nó có thể khiến tài khoản của bạn gặp rủi ro.',
  'Arrow keys move between providers; Enter picks one.': 'Phím mũi tên để chuyển giữa các nhà cung cấp; Enter để chọn.',
  'Account Saved; Models Not Read': 'Đã lưu tài khoản; chưa đọc được model',
  'Continue': 'Tiếp tục',
  'Pygg saved the connection but couldn’t read its model list, so the key may be wrong or the provider unreachable. Nothing was billed.':
    'Pygg đã lưu kết nối nhưng không đọc được danh sách model, có thể key sai hoặc không truy cập được nhà cung cấp. Chưa có gì bị tính phí.',
  'Open the connection to check it again, replace the key, or remove it.':
    'Mở kết nối để kiểm tra lại, thay key, hoặc gỡ nó.',
  'Connect {name}': 'Kết nối {name}',
  'Choose a provider. You enter its key next.': 'Chọn một nhà cung cấp. Sau đó bạn nhập key của nó.',
  'Loading providers…': 'Đang tải nhà cung cấp…',
  'Change Provider': 'Đổi nhà cung cấp',
  'Sign In with OpenRouter…': 'Đăng nhập bằng OpenRouter…',
  'Opens openrouter.ai in your browser. After you sign in and allow it, OpenRouter issues a key for Pygg, kept in {store}; you don’t need to copy one.':
    'Mở openrouter.ai trong trình duyệt. Sau khi bạn đăng nhập và cho phép, OpenRouter cấp một key cho Pygg, được giữ trong {store}; bạn không cần sao chép key.',
  'Name, endpoint and billing': 'Tên, endpoint và cách tính phí',
  '{site}’s terms may forbid automated use. Pygg is not affiliated with it or approved by it.':
    'Điều khoản của {site} có thể cấm sử dụng tự động. Pygg không liên kết với trang này và không được trang này chấp thuận.',
  'Your account can be limited or banned for it. You carry that risk.':
    'Tài khoản của bạn có thể bị hạn chế hoặc bị khóa vì việc này. Bạn tự chịu rủi ro đó.',
  'The site can change at any time and break this connection without notice.':
    'Trang web có thể thay đổi bất cứ lúc nào và làm hỏng kết nối này mà không báo trước.',
  'Pygg types into the site as you, while signed in as you. {site} is an agent with access to the apps you connected to it: Pygg denies every action it asks to take, but anything you send through Pygg is sent from your account.':
    'Pygg gõ vào trang web với tư cách là bạn, khi đang đăng nhập bằng tài khoản của bạn. {site} là một agent có quyền truy cập các ứng dụng bạn đã kết nối với nó: Pygg từ chối mọi thao tác nó đòi thực hiện, nhưng mọi thứ bạn gửi qua Pygg đều được gửi từ tài khoản của bạn.',
  'Pygg types into the site as you, while signed in as you: anything you send through Pygg is sent from your account and kept in its history there.':
    'Pygg gõ vào trang web với tư cách là bạn, khi đang đăng nhập bằng tài khoản của bạn: mọi thứ bạn gửi qua Pygg đều được gửi từ tài khoản của bạn và lưu trong lịch sử của nó ở đó.',
  'No tools or images, no token counts, and it is never chosen by auto: clients use it by naming it, or in a combo you build.':
    'Không có tool hay ảnh, không đếm token, và không bao giờ được auto chọn: client dùng nó bằng cách gọi đúng tên, hoặc trong combo bạn tạo.',
  'Web session (unofficial). Read this before you connect.': 'Phiên web (không chính thức). Hãy đọc trước khi kết nối.',
  'Connect Web Session': 'Kết nối phiên web',
  'Pygg would use {site} the way you do, in a window where you sign in to the site, not through an API.':
    'Pygg sẽ dùng {site} như cách bạn dùng, trong một cửa sổ nơi bạn đăng nhập vào trang web, không qua API.',
  'Pygg never asks for your password and never copies cookies from your browsers. The sign-in stays in this connection’s own folder on this computer, and is deleted when you remove the connection. If the site shows a CAPTCHA or another check, Pygg stops and asks you to deal with it yourself.':
    'Pygg không bao giờ hỏi mật khẩu của bạn và không bao giờ sao chép cookie từ trình duyệt của bạn. Phiên đăng nhập nằm trong thư mục riêng của kết nối này trên máy này, và bị xóa khi bạn gỡ kết nối. Nếu trang web hiện CAPTCHA hoặc bước kiểm tra khác, Pygg dừng lại và để bạn tự xử lý.',
  'I understand these risks and accept them for my account on {site}.':
    'Tôi hiểu các rủi ro này và chấp nhận chúng cho tài khoản của tôi trên {site}.',
  'Also the first part of model names, e.g. {id}/{model}.': 'Cũng là phần đầu của tên model, ví dụ {id}/{model}.',
  'The connection starts off. Next, open its sign-in window, sign in on the site, check the sign-in, then turn it on.':
    'Kết nối ban đầu ở trạng thái tắt. Tiếp theo, mở cửa sổ đăng nhập, đăng nhập trên trang web, kiểm tra đăng nhập, rồi bật kết nối.',
  'Sign-in': 'Đăng nhập',
  'Signed in as {account}.': 'Đã đăng nhập là {account}.',
  'Open Sign-In Window': 'Mở cửa sổ đăng nhập',
  'Checking…': 'Đang kiểm tra…',
  'Signed in. Turn the connection on in the list above to use it.':
    'Đã đăng nhập. Bật kết nối trong danh sách ở trên để dùng.',
  'You accepted the risks on {date}. Pygg drives the site in its own window, one request at a time, and stops at any check the site shows.':
    'Bạn đã chấp nhận rủi ro ngày {date}. Pygg điều khiển trang web trong cửa sổ riêng, mỗi lần một request, và dừng ở mọi bước kiểm tra trang web hiện ra.',
  'The warning has changed since you accepted it. Remove this connection and connect it again to read and accept the new one; until then it is not used.':
    'Cảnh báo đã thay đổi kể từ khi bạn chấp nhận. Hãy gỡ kết nối này và kết nối lại để đọc và chấp nhận cảnh báo mới; trước đó kết nối không được dùng.',
  'Clients reach it as an OpenAI-format provider; Anthropic and Responses requests are translated. Requests with tools are refused.':
    'Client gọi nó như một nhà cung cấp định dạng OpenAI; request Anthropic và Responses được dịch. Request có tool sẽ bị từ chối.',
  'The site’s web page, driven by Pygg': 'Giao diện web của trang, do Pygg điều khiển',
  'In order': 'Theo thứ tự',
  'Round robin': 'Xoay vòng',
  'Least used today': 'Ít dùng nhất hôm nay',
  'Most room left': 'Còn nhiều hạn mức nhất',
  'Each request tries the keys in this order and moves to the next when one is rate-limited, out of credit or rejected.':
    'Mỗi request thử các key theo thứ tự này và chuyển sang key kế tiếp khi một key bị giới hạn tốc độ, hết credit hoặc bị từ chối.',
  'Each new request starts with the next key in turn; a session stays on the key that answered it.':
    'Mỗi request mới bắt đầu với key kế tiếp theo lượt; một phiên giữ nguyên key đã trả lời nó.',
  'Each new request starts with the key that sent the fewest requests today (UTC).':
    'Mỗi request mới bắt đầu với key đã gửi ít request nhất hôm nay (UTC).',
  'Each new request starts with the key whose account last stated the most room left in its rate limits; a key whose stated limit is used up goes last. Without stated limits, the least used goes first.':
    'Mỗi request mới bắt đầu với key mà tài khoản của nó báo còn nhiều chỗ nhất trong giới hạn tốc độ; key đã dùng hết giới hạn đã nêu xếp cuối. Không có giới hạn được nêu thì key ít dùng nhất đi trước.',
  '{state} on {model} until {time}': '{state} với {model} tới {time}',
  '{state} until {time}': '{state} tới {time}',
  'API keys': 'API key',
  'One key. Add a key from another account you hold, with its own quota, and Pygg uses the keys in turn: when one is rate-limited, out of credit or rejected, the next one answers. Several keys of one account share its limits, so Pygg does not take them.':
    'Một key. Thêm key từ một tài khoản khác của bạn, có quota riêng, và Pygg sẽ dùng lần lượt các key: khi một key bị giới hạn tốc độ, hết credit hoặc bị từ chối, key kế tiếp sẽ trả lời. Nhiều key của cùng một tài khoản dùng chung giới hạn của nó, nên Pygg không nhận.',
  'the connection’s key': 'key của kết nối',
  'Use the key “{name}”': 'Dùng key “{name}”',
  'Remove the key “{name}”': 'Gỡ key “{name}”',
  'last used {ago}': 'dùng lần cuối {ago}',
  'not used yet': 'chưa dùng',
  '{pct}% of its stated limit left': 'còn {pct}% giới hạn đã nêu',
  'Key order': 'Thứ tự key',
  'Set this connection’s key first.': 'Hãy đặt key cho kết nối này trước.',
  'Add Key from Another Account…': 'Thêm key từ tài khoản khác…',
  'Add a Key to {name}': 'Thêm key vào {name}',
  'The key is kept in {store}; Pygg never shows or logs it.':
    'Key được giữ trong {store}; Pygg không bao giờ hiển thị hay ghi log nó.',
  'Add Key': 'Thêm key',
  'Which account it belongs to, such as “Work” or “Second account”.':
    'Key này thuộc tài khoản nào, ví dụ “Công việc” hoặc “Tài khoản thứ hai”.',
  'This key belongs to another {provider} account, with its own quota.':
    'Key này thuộc một tài khoản {provider} khác, có quota riêng.',
  'Several keys of one account or organization share its rate limits and credit; rotating them only gets around those limits, which providers’ terms forbid. Pygg never rotates sign-ins (web sessions, subscriptions).':
    'Nhiều key của cùng một tài khoản hoặc tổ chức dùng chung giới hạn tốc độ và credit; xoay vòng chúng chỉ để lách các giới hạn đó, điều mà điều khoản của nhà cung cấp cấm. Pygg không bao giờ xoay vòng đăng nhập (phiên web, gói thuê bao).',
  'Auto can choose these for such a request.': 'Auto có thể chọn các model này cho request như vậy.',
  'A tier, a context size, tool support or a price is not known. Set it with Auto… or Set Price…, or refresh the model list.':
    'Chưa biết tier, kích thước context, hỗ trợ tool hoặc giá. Đặt bằng Auto… hoặc Đặt giá…, hoặc làm mới danh sách model.',
  'Cooling down, held after repeated 429 answers, or out of quota. They come back by themselves.':
    'Đang tạm nghỉ, bị giữ lại sau nhiều lần trả lỗi 429, hoặc hết quota. Chúng sẽ tự trở lại.',
  'Below a new key’s minimum tier, unable to take tools, or too small for the request. Policies sets the minimum.':
    'Dưới tier tối thiểu của key mới, không nhận được tool, hoặc quá nhỏ cho request. Mức tối thiểu được đặt trong Chính sách.',
  'Auto, for a coding tool’s request (tools, a short prompt, a new key’s defaults):':
    'Auto, cho request của công cụ lập trình (có tool, prompt ngắn, mặc định của key mới):',
  'no model eligible': 'không model nào đủ điều kiện',
  'Auto: eligible': 'Auto: đủ điều kiện',
  'Auto skips: {reason} (Auto… sets it)': 'Auto bỏ qua: {reason} (đặt bằng Auto…)',
  'Auto skips: {reason}': 'Auto bỏ qua: {reason}',
  'Never: web session, used by name': 'Không bao giờ: phiên web, gọi theo tên',
  '{n} request today': '{n} request hôm nay',
  '{n} requests today': '{n} request hôm nay',
  '{n} model eligible': '{n} model đủ điều kiện',
  '{n} models eligible': '{n} model đủ điều kiện',
  '{n} missing routing facts': '{n} thiếu thông tin định tuyến',
  '{n} unavailable now': '{n} tạm không dùng được',
  '{n} outside the rules': '{n} ngoài luật',
  'A connection exists, but none is on with a key and models listed yet. Open Providers to check it.':
    'Đã có kết nối, nhưng chưa kết nối nào đang bật, có key và có danh sách model. Mở Nhà cung cấp để kiểm tra.',
  'Connected, but auto has no model that qualifies for a coding agent (tools, tier T2 or higher, a price or free billing). See How auto chooses in Combos.':
    'Đã kết nối, nhưng auto chưa có model nào đủ điều kiện cho agent lập trình (có tool, tier T2 trở lên, có giá hoặc miễn phí). Xem cách auto chọn model trong Combo.',
  'An API key or a local runtime such as Ollama. Keys stay in the Windows Credential Manager.':
    'Một API key hoặc một runtime cục bộ như Ollama. Key được giữ trong Windows Credential Manager.',
  'Set up your coding tool': 'Thiết lập công cụ lập trình',
  'Claude Code with the launcher, Codex and OpenCode in one click, editors and SDKs by hand. Use the model auto.':
    'Claude Code qua launcher, Codex và OpenCode chỉ một cú nhấp, trình soạn thảo và SDK thì cài bằng tay. Dùng model auto.',
  'First request answered': 'Request đầu tiên đã được trả lời',
  'Work in your IDE or terminal. The first request a tool sends through Pygg shows here, with the model that answered and what it cost.':
    'Làm việc trong IDE hoặc terminal. Request đầu tiên một công cụ gửi qua Pygg sẽ hiện ở đây, kèm model đã trả lời và chi phí.',
  'last 24 hours': '24 giờ qua',
  'Then work in your IDE or terminal as usual. auto picks a model from your connections for each request, within your limits; no combo is needed.':
    'Sau đó làm việc trong IDE hoặc terminal như bình thường. auto chọn một model trong các kết nối của bạn cho mỗi request, trong hạn mức của bạn; không cần combo.',
  'How auto chooses': 'Cách auto chọn model',
  'Fallback attempts': 'Lần chuyển model',
  'another model was tried': 'đã thử model khác',
  'Latest answered request: {client} → {model} · {ago}. Each tool’s own state is in CLI Tools.':
    'Request gần nhất được trả lời: {client} → {model} · {ago}. Trạng thái của từng công cụ nằm trong Công cụ CLI.',
  'For Cline, Continue, Roo Code and other editors that call the API from this computer: choose an OpenAI-compatible provider and fill in these fields. Editors that send requests from their own servers can’t reach 127.0.0.1.':
    'Với Cline, Continue, Roo Code và các trình soạn thảo khác gọi API từ máy này: chọn một nhà cung cấp tương thích OpenAI và điền các trường này. Trình soạn thảo gửi request từ server riêng của họ không truy cập được 127.0.0.1.',
  'Set Up by Hand': 'Cài thủ công',
  'Editors': 'Trình soạn thảo',
  'Set PYGG_API_KEY to the Pygg API key you saved when you created it.':
    'Đặt PYGG_API_KEY bằng API key Pygg bạn đã lưu khi tạo nó.',
  'You’ll need a Pygg API key first.': 'Bạn cần có API key Pygg trước.',
  'New input': 'Đầu vào mới',
  'Cache reads': 'Đọc cache',
  'Cache writes': 'Ghi cache',
  'Prompt tokens the provider read in full': 'Token prompt nhà cung cấp đọc đầy đủ',
  'Prompt tokens served from the provider’s cache, at its cached rate':
    'Token prompt lấy từ cache của nhà cung cấp, tính theo giá cache',
  'Prompt tokens written to the cache, which some providers charge extra for':
    'Token prompt ghi vào cache, một số nhà cung cấp tính thêm phí',
  'Tokens the model generated': 'Token do model sinh ra',
  'Prompt too short': 'Prompt quá ngắn',
  'Cache hints dropped': 'Đánh dấu cache bị bỏ',
  'First request of a session': 'Request đầu tiên của phiên',
  'Model switched': 'Đã đổi model',
  'Model does not cache': 'Model không cache',
  'Cache expired': 'Cache hết hạn',
  'Prompt start changed': 'Đầu prompt đã đổi',
  'No session': 'Không có phiên',
  'The provider did not say how much of the prompt came from its cache.':
    'Nhà cung cấp không cho biết bao nhiêu phần prompt lấy từ cache.',
  'Under about 1,024 tokens, which providers do not cache.': 'Dưới khoảng 1.024 token, mức nhà cung cấp không cache.',
  'The client marked the prompt for caching, but this provider takes no hints in this format.':
    'Client đã đánh dấu prompt để cache, nhưng nhà cung cấp này không nhận đánh dấu ở định dạng này.',
  'Nothing was cached for this session on this model yet.': 'Chưa có gì được cache cho phiên này trên model này.',
  'The session’s last answer came from another model, whose cache this one cannot read.':
    'Câu trả lời gần nhất của phiên đến từ model khác, model này không đọc được cache của nó.',
  'This model has read nothing from its cache in the last 7 days.':
    'Model này không đọc được gì từ cache trong 7 ngày qua.',
  'More than 5 minutes since this session last used this model.':
    'Đã hơn 5 phút kể từ lần cuối phiên này dùng model này.',
  'The same model answered minutes ago, so the start of the prompt (system prompt, tools or early history) likely changed.':
    'Cùng model đã trả lời vài phút trước, nên có lẽ phần đầu prompt (system prompt, tool hoặc lịch sử đầu) đã thay đổi.',
  'The request named no session, so Pygg cannot tell why.': 'Request không nêu phiên, nên Pygg không biết lý do.',
  'Token Mix': 'Cơ cấu token',
  'Show the token mix as': 'Hiện cơ cấu token theo',
  'Estimated Cost': 'Chi phí ước tính',
  'See which tokens were new, read from cache, written to cache, or generated.':
    'Xem token nào là mới, đọc từ cache, ghi vào cache, hay do model sinh ra.',
  'No token counts in this period: the providers did not report any with their answers.':
    'Không có số token trong khoảng này: nhà cung cấp không báo kèm câu trả lời.',
  'No known cost to split in this period: the answers were free, local, on a plan, or of unknown cost.':
    'Không có chi phí đã biết để chia trong khoảng này: các câu trả lời là miễn phí, cục bộ, theo gói, hoặc chưa rõ chi phí.',
  'Input served from cache: {share}%.': 'Đầu vào lấy từ cache: {share}%.',
  'Token counts on {n} of {total} requests.': 'Có số token ở {n} trên {total} request.',
  'Each request’s recorded cost, split by its model’s current prices.':
    'Chi phí đã ghi của từng request, chia theo giá hiện tại của model.',
  '{amount} could not be split: its model has no price now.':
    '{amount} không chia được: model của nó hiện chưa có giá.',
  'A provider that reports no cache figures shows its whole prompt as new input. The cache’s discount is the provider’s, not a saving of Pygg’s.':
    'Nhà cung cấp không báo số liệu cache thì toàn bộ prompt hiện là đầu vào mới. Phần giảm giá nhờ cache là của nhà cung cấp, không phải khoản Pygg tiết kiệm được.',
  'Answers that read nothing from the provider’s cache, with the likeliest cause read from the ledger. The provider does not say why.':
    'Các câu trả lời không đọc được gì từ cache của nhà cung cấp, kèm nguyên nhân khả dĩ nhất suy ra từ sổ chi. Nhà cung cấp không cho biết lý do.',
  'By client and model': 'Theo client và model',
  'Main conversation': 'Hội thoại chính',
  'Subagents': 'Subagent',
  'Workflows': 'Workflow',
  'Compaction': 'Nén hội thoại',
  'Side requests': 'Request phụ',
  'Other': 'Khác',
  'Kind': 'Loại',
  'Subagent type': 'Loại subagent',
  'at most {n} working at once in a session, at most {m} started by one prompt.':
    'tối đa {n} chạy cùng lúc trong một phiên, tối đa {m} do một prompt khởi tạo.',
  'Each subagent starts a conversation of its own, so its first request writes a new cache. Claude Code says what each request is only when started with pygg run claude.':
    'Mỗi subagent bắt đầu hội thoại riêng, nên request đầu tiên của nó ghi cache mới. Claude Code chỉ cho biết từng request là gì khi được chạy bằng pygg run claude.',
  'From cache': 'Từ cache',
  'Tool definitions at each session’s first request, on average':
    'Định nghĩa tool ở request đầu tiên của mỗi phiên, trung bình',
  'Tools per session': 'Tool mỗi phiên',
  'System prompt at each session’s first request, on average':
    'System prompt ở request đầu tiên của mỗi phiên, trung bình',
  'System prompt per session': 'System prompt mỗi phiên',
  'Requests whose tools or system prompt differed from the session’s request before':
    'Các request có tool hoặc system prompt khác với request trước đó của phiên',
  'Changed in a session': 'Thay đổi trong phiên',
  'Later requests that read nothing from the cache, so the tools and system prompt went again in full or as a cache write':
    'Các request sau không đọc được gì từ cache, nên tool và system prompt được gửi lại đầy đủ hoặc dưới dạng ghi cache',
  'Sent again uncached': 'Gửi lại không qua cache',
  'What each session sends before its conversation, measured from the request; sizes are Pygg’s estimate. Sent again uncached counts later requests of a session that read nothing from the provider’s cache. Pygg keeps these numbers only, never the text. Requests without a session are not counted.':
    'Những gì mỗi phiên gửi trước phần hội thoại, đo từ request; kích thước là ước tính của Pygg. “Gửi lại không qua cache” đếm các request sau của một phiên không đọc được gì từ cache của nhà cung cấp. Pygg chỉ giữ các con số này, không bao giờ giữ nội dung. Request không có phiên không được tính.',
  'Sessions that sent their tools and system prompt again the most':
    'Các phiên gửi lại tool và system prompt nhiều nhất',
  'Tools': 'Tool',
  'System prompt': 'System prompt',
  'Sent again': 'Gửi lại',
  'Why the cache was missed': 'Vì sao không trúng cache',
  'Cause': 'Nguyên nhân',
  'Answers': 'Câu trả lời',
  'What it means': 'Ý nghĩa',
  '{n} request with unknown cost is not in it.': '{n} request chưa rõ chi phí không nằm trong đó.',
  '{n} requests with unknown cost are not in it.': '{n} request chưa rõ chi phí không nằm trong đó.',
  'By request kind (Claude Code): {n} request': 'Theo loại request (Claude Code): {n} request',
  'By request kind (Claude Code): {n} requests': 'Theo loại request (Claude Code): {n} request',
  '{n} subagent': '{n} subagent',
  '{n} subagents': '{n} subagent',
  'Tools and system prompt: {n} session': 'Tool và system prompt: {n} phiên',
  'Tools and system prompt: {n} sessions': 'Tool và system prompt: {n} phiên',
  '{n} time': '{n} lần',
  '{n} times': '{n} lần',
  'From Template…': 'Từ mẫu…',
  'Using auto? You don’t need a combo. Make one when you want to choose the models and their fallback order yourself; the next model is tried only before an answer starts.':
    'Dùng auto? Bạn không cần combo. Hãy tạo combo khi muốn tự chọn model và thứ tự dự phòng; model kế tiếp chỉ được thử trước khi câu trả lời bắt đầu.',
  'Start from a Template…': 'Bắt đầu từ mẫu…',
  'One model name, a fallback order you control. A combo such as “coding” stands for a list of models: if the first is out of quota or unavailable, the next one answers. Templates such as Free first or Use my plans first build one from your models.':
    'Một tên model, thứ tự dự phòng do bạn quyết định. Combo như “coding” đại diện cho một danh sách model: nếu model đầu hết quota hoặc không khả dụng, model kế tiếp sẽ trả lời. Các mẫu như Ưu tiên miễn phí hay Ưu tiên gói của tôi tạo combo từ các model của bạn.',
  'One model name, a fallback order you control. Clients send the name as the model.':
    'Một tên model, thứ tự dự phòng do bạn quyết định. Client gửi tên này ở chỗ tên model.',
  'Cheapest first': 'Rẻ nhất trước',
  'Ordered fallback': 'Dự phòng theo thứ tự',
  'Ordered for each request by its estimated cost: free, local and plan members first, unknown prices last.':
    'Xếp theo chi phí ước tính của từng request: thành viên miễn phí, cục bộ và theo gói trước, giá chưa rõ sau cùng.',
  'Tries members in order when a key is rejected, quota is exhausted, or a model is unavailable. Once an answer starts streaming, Pygg does not switch models.':
    'Thử lần lượt các thành viên khi key bị từ chối, hết quota hoặc model không khả dụng. Khi câu trả lời đã bắt đầu stream, Pygg không đổi model.',
  'After a timeout or a server error the provider may already have run (and billed) the request, so the gateway stops there unless the API key allows paid retries (Endpoint → API Keys → Limits). A malformed request is returned as it is.':
    'Sau khi quá thời gian hoặc lỗi máy chủ, nhà cung cấp có thể đã chạy (và tính phí) request, nên gateway dừng ở đó trừ khi API key cho phép thử lại trả phí (Endpoint → API key → Giới hạn). Yêu cầu sai định dạng được trả về nguyên trạng.',
  'Send one short request through {name}? A paid model may answer, and fallback may try more than one model. Charges are recorded in Usage.':
    'Gửi một request ngắn qua {name}? Model trả phí có thể trả lời, và fallback có thể thử nhiều hơn một model. Chi phí được ghi trong Mức dùng.',
  'Answered by {model} in {ms} ms, after {n} members failed':
    '{model} trả lời trong {ms} ms, sau {n} thành viên thất bại',
  'Answered by {model} in {ms} ms': '{model} trả lời trong {ms} ms',
  'Type part of a model or provider name': 'Gõ một phần tên model hoặc nhà cung cấp',
  'in this combo': 'trong combo này',
  'No model matches “{query}”.': 'Không có model nào khớp “{query}”.',
  'no tools': 'không có tool',
  'For each request, the members by its estimated cost: free, local and plan members first, unknown prices last. The next is tried on the same errors as always.':
    'Với mỗi request, các thành viên được xếp theo chi phí ước tính: thành viên miễn phí, cục bộ và theo gói trước, giá chưa rõ sau cùng. Model kế tiếp được thử với cùng các lỗi như thường lệ.',
  'The members in the order below.': 'Các thành viên theo thứ tự bên dưới.',
  'As listed': 'Theo danh sách',
  'Drag to reorder': 'Kéo để sắp xếp lại',
  '{connections} share one quota: when one is refused for its quota or rate limit, the gateway does not try the others for that model, since they would be refused too. If they are separate accounts, confirm that in Providers.':
    '{connections} dùng chung một quota: khi một kết nối bị từ chối vì hết quota hoặc giới hạn tốc độ, gateway không thử các kết nối kia cho model đó, vì chúng cũng sẽ bị từ chối. Nếu đó là các tài khoản riêng, hãy xác nhận trong Nhà cung cấp.',
  'No model is turned on: turn one on in Providers': 'Chưa bật model nào: hãy bật một model trong Nhà cung cấp',
  '{n} off': '{n} đã tắt',
  '{n} missing': '{n} bị thiếu',
  '{n} model matches': '{n} model khớp',
  '{n} models match': '{n} model khớp',
  'OpenAI-compatible': 'Tương thích OpenAI',
  'Chat Completions and Responses: Codex, OpenCode, editors, SDKs':
    'Chat Completions và Responses: Codex, OpenCode, trình soạn thảo, SDK',
  'Messages API: Claude Code, Anthropic SDKs': 'Messages API: Claude Code, SDK của Anthropic',
  'auto picks a model for each request; auto/cheap the cheapest that qualifies. A combo’s name, or connection/model, also works.':
    'auto chọn một model cho mỗi request; auto/cheap chọn model rẻ nhất đủ điều kiện. Tên một combo, hoặc connection/model, cũng dùng được.',
  'Copy OpenAI URL': 'Sao chép URL OpenAI',
  'Copy Anthropic URL': 'Sao chép URL Anthropic',
  'Base URLs': 'Base URL',
  'Send a Pygg API key as the bearer token (or x-api-key). The gateway listens on this computer only.':
    'Gửi một API key Pygg làm bearer token (hoặc x-api-key). Gateway chỉ lắng nghe trên máy này.',
  'Usage view': 'Chế độ xem mức dùng',
  'Subscriptions': 'Gói thuê bao',
  'Local Runtime': 'Runtime cục bộ',
  'Export CSV…': 'Xuất CSV…',
  'API costs view': 'Chế độ xem chi phí API',
  'Summary': 'Tóm tắt',
  'No gateway requests in this period': 'Không có request nào qua gateway trong khoảng này',
  'Set up a coding tool, then send a request: it appears here with its model, its cost and where that figure comes from.':
    'Cài một công cụ lập trình rồi gửi request: request sẽ hiện ở đây kèm model, chi phí và nguồn của con số đó.',
  'last {n} days': '{n} ngày qua',
  'Unknown costs are not in the total. Give a model a price in Providers to estimate them.':
    'Chi phí chưa rõ không nằm trong tổng. Đặt giá cho model trong Nhà cung cấp để ước tính chúng.',
  'Searching the period…': 'Đang tìm trong khoảng này…',
  'Nothing matches in this period.': 'Không có gì khớp trong khoảng này.',
  'Loading older attempts…': 'Đang tải các lần thử cũ hơn…',
  '{n} of the period’s attempts match these filters; {shown} shown, newest first.':
    '{n} lần thử trong khoảng này khớp bộ lọc; đang hiện {shown}, mới nhất trước.',
  'Showing the {shown} most recent of {total} requests in this period; Show More goes further back, and Export CSV has them all.':
    'Đang hiện {shown} request gần nhất trong tổng {total} request của khoảng này; “Hiện thêm” xem lùi xa hơn, còn “Xuất CSV” có tất cả.',
  'Every request of this period is listed, newest first.':
    'Mọi request trong khoảng này đều được liệt kê, mới nhất trước.',
  'Failed attempts before a fallback are listed too. A request the provider refused before answering is usually not billed; one cut off after it started may have been, so its cost shows as unknown or estimated.':
    'Các lần thử thất bại trước khi fallback cũng được liệt kê. Request bị nhà cung cấp từ chối trước khi trả lời thường không bị tính phí; request bị ngắt sau khi đã bắt đầu thì có thể đã bị tính, nên chi phí hiện là chưa rõ hoặc ước tính.',
  'Nothing in this period': 'Không có gì trong khoảng này',
  'No Subscriptions': 'Chưa có gói thuê bao',
  'Set a connection’s billing to Subscription in Providers, for a plan paid by the month. Its requests are counted here against the plan, not as money.':
    'Đặt cách tính phí của kết nối là Gói thuê bao trong Nhà cung cấp, cho gói trả theo tháng. Request của nó được tính ở đây theo gói, không tính bằng tiền.',
  'Limits, as the provider last stated': 'Hạn mức, theo lần báo gần nhất của nhà cung cấp',
  'A subscription costs its monthly price, not a price per request, so these requests have no cost of their own; in API Costs they count as unknown. Limits are read from the headers of the provider’s last answer for each model. Pygg does not guess what is left: a provider that states nothing shows Unknown.':
    'Gói thuê bao tốn theo giá hằng tháng, không tính theo từng request, nên các request này không có chi phí riêng; trong Chi phí API chúng được tính là chưa rõ. Hạn mức được đọc từ header của câu trả lời gần nhất của nhà cung cấp cho từng model. Pygg không đoán phần còn lại: nhà cung cấp không báo gì thì hiện Chưa rõ.',
  'Account': 'Tài khoản',
  'The provider has not stated a limit in its answers yet.': 'Nhà cung cấp chưa báo hạn mức trong các câu trả lời.',
  'No Local Runtime': 'Chưa có runtime cục bộ',
  'Add Ollama, LM Studio or another server on this computer in Providers. The requests it answers are counted here.':
    'Thêm Ollama, LM Studio hoặc server khác trên máy này trong Nhà cung cấp. Các request nó trả lời được tính ở đây.',
  'API cost': 'Chi phí API',
  'No API money is spent on a local model. Electricity and the computer’s time are not measured, so they are not in any figure here.':
    'Model cục bộ không tốn tiền API. Điện và thời gian máy không được đo, nên không có trong số liệu nào ở đây.',
  'Local models': 'Model cục bộ',
  'Median time': 'Thời gian trung vị',
  'Median time is of successful requests, from when the gateway sent the request until the answer ended. It includes loading the model when it was not in memory.':
    'Thời gian trung vị tính trên các request thành công, từ lúc gateway gửi request tới khi câu trả lời kết thúc. Gồm cả thời gian nạp model khi model chưa có trong bộ nhớ.',
  'auto (Pygg’s choice by tier)': 'auto (Pygg chọn theo tier)',
  'auto/cheap (Pygg’s choice by cost)': 'auto/cheap (Pygg chọn theo chi phí)',
  'auto/plan (Pygg’s choice by the facts for plan)': 'auto/plan (Pygg chọn theo dữ kiện cho plan)',
  'auto/code (Pygg’s choice by the facts for code)': 'auto/code (Pygg chọn theo dữ kiện cho code)',
  'auto/test (Pygg’s choice by the facts for test)': 'auto/test (Pygg chọn theo dữ kiện cho test)',
  'auto/review (Pygg’s choice by the facts for review)': 'auto/review (Pygg chọn theo dữ kiện cho review)',
  'a combo, in its order': 'một combo, theo thứ tự của nó',
  'a combo, cheapest first': 'một combo, rẻ nhất trước',
  'a pool of accounts': 'một nhóm tài khoản',
  'a model by name': 'một model theo tên',
  '{route}; the routing session’s model, kept': '{route}; giữ model của phiên định tuyến',
  '{route}; the session back on its first model after a fallback':
    '{route}; phiên trở lại model đầu tiên sau khi fallback',
  'Turned on by Pygg: Anthropic’s automatic caching, which this client cannot ask for':
    'Pygg bật: cache tự động của Anthropic, thứ client này không tự yêu cầu được',
  'Kept as the client sent them': 'Giữ nguyên như client gửi',
  'Carried into the OpenAI format': 'Chuyển sang định dạng OpenAI',
  'Dropped: this provider takes none in this format': 'Bị bỏ: nhà cung cấp này không nhận ở định dạng này',
  'None sent': 'Không gửi',
  'attempt {n}': 'lần thử {n}',
  'Attempted model': 'Model đã thử',
  'How the route was chosen': 'Cách chọn tuyến',
  '{key}, of the connection’s keys': '{key}, trong các key của kết nối',
  'Billing when sent': 'Cách tính phí lúc gửi',
  'Prompt-cache hints': 'Đánh dấu prompt cache',
  'Cache missed': 'Không trúng cache',
  'Attempts of this request': 'Các lần thử của request này',
  'Routing session': 'Phiên định tuyến',
  'one launch of the tool through pygg run': 'một lần chạy tool qua pygg run',
  'named by the tool or a task step': 'do tool hoặc một bước tác vụ đặt tên',
  'Requests through Pygg with a price': 'Request qua Pygg đã có giá',
  '{n} requests · {amount}': '{n} request · {amount}',
  'Requests through Pygg without a price': 'Request qua Pygg chưa có giá',
  '{n} requests: give their models a price in Providers':
    '{n} request: hãy đặt giá cho model của chúng trong Nhà cung cấp',
  'Paid agent runs with an unknown cost, in whole or in part':
    'Lần chạy agent trả phí chưa rõ chi phí, toàn bộ hoặc một phần',
  'could not be read: {error}': 'không đọc được: {error}',
  'loading…': 'đang tải…',
  '{n} runs': '{n} lần chạy',
  'the last refresh failed; these are earlier figures': 'lần làm mới gần nhất thất bại; đây là số liệu trước đó',
  'none connected': 'chưa kết nối gói nào',
  '{n} connected · {m} state their limits; the rest are Unknown':
    '{n} đã kết nối · {m} có báo hạn mức; còn lại Chưa rõ',
  'Tools not pointed at Pygg': 'Công cụ không trỏ tới Pygg',
  'not seen at all: their use and limits are in their own apps':
    'hoàn toàn không thấy: mức dùng và hạn mức nằm trong ứng dụng riêng của chúng',
  'What Pygg sees': 'Những gì Pygg thấy',
  'Totals above count only the first row. Pygg never guesses a price or a quota a provider has not stated.':
    'Tổng ở trên chỉ tính dòng đầu tiên. Pygg không bao giờ đoán giá hay quota mà nhà cung cấp chưa báo.',
  'Routing': 'Định tuyến',
  'Review Fallbacks': 'Xem các lần chuyển model',
  'Whether requests stayed on one model, and what a fallback did.':
    'Request có ở lại trên một model không, và việc chuyển model đã dẫn tới gì.',
  'Recorded for requests sent from this version of Pygg on: none in this period yet.':
    'Chỉ ghi cho request gửi từ phiên bản Pygg này trở đi: chưa có trong khoảng này.',
  'Answered after a fallback': 'Trả lời sau khi chuyển model',
  'of {n} answered': 'trên {n} đã trả lời',
  'Attempts with a routing session': 'Lần thử có phiên định tuyến',
  '{n} sessions': '{n} phiên',
  'Changes of model in a session': 'Lần đổi model trong một phiên',
  'A routing session keeps a launch on the model that answered it, and so on its prompt cache; pygg run gives each launch one. A fallback can happen before any answer, so it is not always a change.':
    'Phiên định tuyến giữ một lần khởi chạy trên model đã trả lời nó, nhờ đó giữ được prompt cache; pygg run cấp cho mỗi lần khởi chạy một phiên. Chuyển model có thể xảy ra trước khi có câu trả lời, nên không phải lúc nào cũng là một lần đổi.',
  'Changes of model ({n})': 'Các lần đổi model ({n})',
  'Changes of model': 'Các lần đổi model',
  'From': 'Từ',
  'To': 'Sang',
  'Cache writes after': 'Ghi cache sau đó',
  'Sent there first': 'Gửi tới đó ngay từ đầu',
  'Showing the 50 latest of {n} changes. ': 'Đang hiện 50 lần đổi gần nhất trong {n}. ',
  'Usage after a change is observed; the extra cost caused by the change is not known.':
    'Mức dùng sau mỗi lần đổi là số đo thực tế; chi phí phát sinh do lần đổi gây ra thì chưa rõ.',
  'Point your coding tool at Pygg and choose the model auto, then work as usual. Pygg picks an eligible model for each request from your providers, free and cheap ones first where they qualify, within your limits, and records what each request cost.':
    'Trỏ công cụ lập trình của bạn tới Pygg và chọn model auto, rồi làm việc như bình thường. Với mỗi request, Pygg chọn một model đủ điều kiện từ các nhà cung cấp của bạn, ưu tiên model miễn phí và rẻ khi đáp ứng được, trong hạn mức của bạn, và ghi lại chi phí từng request.',
  'One-Click Setup': 'Cài đặt một cú nhấp',
  'Each tool gets a Pygg API key of its own, so you can see what it spends and whether it works. Pygg backs up a tool’s configuration before changing it; Remove puts it back and revokes the key.':
    'Mỗi công cụ có API key Pygg riêng, để bạn thấy nó tiêu bao nhiêu và có hoạt động không. Pygg sao lưu cấu hình của công cụ trước khi thay đổi; Gỡ sẽ khôi phục cấu hình và thu hồi key.',
  'Connect a provider before sending requests through Pygg: until one is connected, every tool’s requests would be refused.':
    'Hãy kết nối một nhà cung cấp trước khi gửi request qua Pygg: chưa có kết nối thì mọi request của các công cụ đều bị từ chối.',
  'Claude Code': 'Claude Code',
  'OpenCode': 'OpenCode',
  'Set up by hand: Claude Code without the launcher, Cline, Continue, SDKs, curl':
    'Cài thủ công: Claude Code không dùng launcher, Cline, Continue, SDK, curl',
  'auto has no model yet that qualifies for {tool}’s requests (tools, the minimum tier, a price or free billing, and a provider that takes its format). Its requests would be refused until one does.':
    'auto chưa có model nào đáp ứng request của {tool} (tool, tier tối thiểu, có giá hoặc miễn phí, và nhà cung cấp nhận định dạng của nó). Request của nó sẽ bị từ chối cho tới khi có model đáp ứng.',
  'No provider is connected yet, so {tool}’s requests through Pygg would be refused. Connect a provider first.':
    'Chưa kết nối nhà cung cấp nào, nên request của {tool} qua Pygg sẽ bị từ chối. Hãy kết nối nhà cung cấp trước.',
  'Configured. Waiting for its first request: start the tool and send one.':
    'Đã cấu hình. Đang chờ request đầu tiên: hãy mở công cụ và gửi một request.',
  'Working: answered by {model} {ago}.': 'Hoạt động: {model} đã trả lời {ago}.',
  'Its last request, {ago}, did not succeed ({outcome}).': 'Request gần nhất, {ago}, không thành công ({outcome}).',
  'View Failed Requests': 'Xem request thất bại',
  'Claude Code: models through Pygg': 'Claude Code: model qua Pygg',
  'Start Claude Code with the command below to use your connected providers; plain claude keeps your Claude sign-in.':
    'Khởi chạy Claude Code bằng lệnh dưới đây để dùng các nhà cung cấp đã kết nối; lệnh claude thông thường vẫn dùng đăng nhập Claude của bạn.',
  'Set Up gives you a command that starts Claude Code through Pygg with auto, one launch at a time; plain claude keeps your Claude sign-in.':
    'Cài đặt sẽ cho bạn một lệnh khởi chạy Claude Code qua Pygg với auto, từng lần chạy một; lệnh claude thông thường vẫn dùng đăng nhập Claude của bạn.',
  'What Set Up changes': 'Cài đặt sẽ thay đổi gì',
  'Nothing in Claude Code’s own settings: pointed at another endpoint, it would stop using your Claude plan. Pygg makes an API key for the launcher, kept in the Windows Credential Manager and never in a file, and hands it to Claude Code at each launch. Remove revokes the key.':
    'Không thay đổi gì trong cài đặt riêng của Claude Code: nếu trỏ sang endpoint khác, nó sẽ ngừng dùng gói Claude của bạn. Pygg tạo một API key cho launcher, giữ trong Windows Credential Manager và không bao giờ ghi ra tệp, rồi đưa cho Claude Code mỗi lần khởi chạy. Gỡ sẽ thu hồi key.',
  'Start Claude Code through Pygg': 'Khởi chạy Claude Code qua Pygg',
  'From npm (npm install -g @pygg/cli@dev) it is pygg run claude. Add Claude Code’s own arguments after it; --model before claude picks another model for every slot.':
    'Nếu cài từ npm (npm install -g @pygg/cli@dev) thì lệnh là pygg run claude. Thêm tham số riêng của Claude Code phía sau; --model đặt trước claude sẽ chọn model khác cho mọi vị trí.',
  'Remove the Claude Code launcher?': 'Gỡ launcher của Claude Code?',
  'This revokes the launcher’s key and deletes it from the Credential Manager. The command stops working; Claude Code itself is not changed.':
    'Thao tác này thu hồi key của launcher và xóa nó khỏi Credential Manager. Lệnh sẽ ngừng hoạt động; bản thân Claude Code không bị thay đổi.',
  'Reusable instructions for your coding agents, in the Agent Skills format that Claude Code, Codex and OpenCode share. An agent keeps only each skill’s name and description in context and reads the rest when it uses the skill. A prompt runs only when you type /name, so it costs nothing until then.':
    'Hướng dẫn dùng lại được cho các agent lập trình, theo định dạng Agent Skills mà Claude Code, Codex và OpenCode cùng dùng. Agent chỉ giữ tên và mô tả của mỗi skill trong context và đọc phần còn lại khi dùng skill. Prompt chỉ chạy khi bạn gõ /name, nên không tốn gì cho tới lúc đó.',
  'Sent With Every Request': 'Gửi kèm mọi request',
  'What each agent adds to every request from its global files, before your own words: instruction files are sent whole, listed skills and commands by name and description. A project’s own files add to this; see Projects below. Providers cache it, so repeated requests read it at a fraction of the price, but each new session writes it again.':
    'Những gì mỗi agent thêm vào mọi request từ các file toàn cục của nó, trước lời bạn viết: file hướng dẫn được gửi nguyên vẹn, skill và lệnh có trong danh sách thì gửi tên và mô tả. File riêng của dự án cộng thêm vào đây; xem Dự án bên dưới. Nhà cung cấp cache phần này, nên request lặp lại đọc nó với giá chỉ bằng một phần nhỏ, nhưng mỗi phiên mới lại ghi nó lần nữa.',
  'Reading the agents’ folders…': 'Đang đọc thư mục của các agent…',
  'Library': 'Thư viện',
  'Pygg’s starter skills and your own. Install one for Claude Code (its skills folder) or for Codex and OpenCode (the shared ~/.agents/skills folder); remove takes Pygg’s copy away again.':
    'Skill khởi đầu của Pygg và skill của bạn. Cài một skill cho Claude Code (thư mục skills của nó) hoặc cho Codex và OpenCode (thư mục dùng chung ~/.agents/skills); gỡ sẽ xoá bản sao của Pygg đi.',
  'Loading skills…': 'Đang tải skill…',
  'No skills yet': 'Chưa có skill',
  'Skills for one repository go into its own .claude/skills (Claude Code) and .agents/skills (Codex and OpenCode), so they travel with the code. Pygg writes only into folders you add here, and counts what each agent adds to every request from the files at the project’s root.':
    'Skill cho một repository nằm trong .claude/skills (Claude Code) và .agents/skills (Codex và OpenCode) của chính nó, nên đi cùng code. Pygg chỉ ghi vào các thư mục bạn thêm ở đây, và đếm lượng mỗi agent thêm vào mọi request từ các file ở thư mục gốc của dự án.',
  'Reading the projects…': 'Đang đọc các dự án…',
  'Add Project…': 'Thêm dự án…',
  'Add a repository to install skills into it and see what its CLAUDE.md and AGENTS.md add to every request.':
    'Thêm một repository để cài skill vào đó và xem CLAUDE.md và AGENTS.md của nó thêm gì vào mỗi request.',
  'This deletes the skill from your library. It is not installed anywhere, so no agent loses it.':
    'Thao tác này xóa skill khỏi thư viện của bạn. Skill chưa được cài ở đâu, nên không agent nào bị mất nó.',
  'Remove {name} from Pygg?': 'Gỡ {name} khỏi Pygg?',
  'Pygg stops listing this folder and will not write into it. Nothing in the folder is changed.':
    'Pygg thôi liệt kê thư mục này và sẽ không ghi vào đó. Không có gì trong thư mục bị thay đổi.',
  'about {count} tokens': 'khoảng {count} token',
  'Install for {target}': 'Cài cho {target}',
  'Update for {target}': 'Cập nhật cho {target}',
  'Remove from {target}': 'Gỡ khỏi {target}',
  'View…': 'Xem…',
  'Duplicate…': 'Nhân bản…',
  'Delete…': 'Xóa…',
  'Pygg': 'Pygg',
  'Yours': 'Của bạn',
  'Prompt': 'Prompt',
  'Skill': 'Skill',
  'Costs nothing until you type /{name}; then {tokens}.': 'Không tốn gì cho tới khi bạn gõ /{name}; khi đó {tokens}.',
  'Adds {listed} to every request where it is installed; {tokens} when used.':
    'Thêm {listed} vào mọi request ở nơi đã cài; {tokens} khi được dùng.',
  'Working…': 'Đang xử lý…',
  'Install for All': 'Cài cho tất cả',
  'Actions for {name}': 'Thao tác cho {name}',
  'from Pygg': 'từ Pygg',
  'only when used': 'chỉ khi dùng',
  'Add to Library': 'Thêm vào thư viện',
  'No global instruction file, skill or command found.': 'Không tìm thấy file hướng dẫn, skill hay lệnh toàn cục nào.',
  '{count} files, {counted} sent with every request': '{count} file, {counted} gửi kèm mọi request',
  'Folder not found': 'Không tìm thấy thư mục',
  'up to {tokens} more': 'thêm tối đa {tokens}',
  '1 skill from Pygg': '1 skill từ Pygg',
  '{count} skills from Pygg': '{count} skill từ Pygg',
  '{count} files at the project’s root': '{count} file ở thư mục gốc của dự án',
  'Install Skills…': 'Cài skill…',
  'Remove from Pygg…': 'Gỡ khỏi Pygg…',
  'Add Project': 'Thêm dự án',
  'The repository’s root folder, where its CLAUDE.md or AGENTS.md is.':
    'Thư mục gốc của repository, nơi có CLAUDE.md hoặc AGENTS.md.',
  'Skills in {name}': 'Skill trong {name}',
  'Install {name} for {target}': 'Cài {name} cho {target}',
  'Install': 'Cài',
  'Remove {name} from {target}': 'Gỡ {name} khỏi {target}',
  'Update {name} for {target}': 'Cập nhật {name} cho {target}',
  'Update': 'Cập nhật',
  'New Skill': 'Skill mới',
  'One of Pygg’s starter skills. Duplicate it to make your own version.':
    'Một skill khởi đầu của Pygg. Nhân bản để tạo phiên bản của riêng bạn.',
  'Typed as /name. Lowercase letters, digits and hyphens.': 'Gõ dưới dạng /name. Chữ thường, chữ số và dấu gạch ngang.',
  'Description': 'Mô tả',
  'What it does and when to use it. The agent reads only this to choose a skill.':
    'Skill làm gì và khi nào nên dùng. Agent chỉ đọc phần này để chọn skill.',
  'Instructions': 'Hướng dẫn',
  'Read when the skill is used. $ARGUMENTS is what you type after the name.':
    'Được đọc khi dùng skill. $ARGUMENTS là phần bạn gõ sau tên.',
  'Run only when I type its name (a prompt)': 'Chỉ chạy khi tôi gõ tên (một prompt)',
  'Claude Code and OpenCode then leave it out of the list they send with every request. For Codex, Pygg also writes agents/openai.yaml beside it, so Codex leaves it out too; there you call it as $name.':
    'Khi đó Claude Code và OpenCode bỏ nó khỏi danh sách gửi kèm mọi request. Với Codex, Pygg ghi thêm agents/openai.yaml bên cạnh, để Codex cũng bỏ nó ra; ở Codex bạn gọi nó bằng $name.',
  ' — the lowest tier at or above the client’s minimum (T2 unless you change it in Endpoint → API Keys → Limits…), then local, free, subscription and paid in that order, then the lower estimated cost. This is the lowest eligible tier under your policy, not a promise of the cheapest answer.':
    ' — tier thấp nhất đạt mức tối thiểu của client (T2 nếu bạn không đổi trong Endpoint → API key → Giới hạn…), rồi ưu tiên cục bộ, miễn phí, thuê bao và trả phí theo thứ tự đó, rồi tới chi phí ước tính thấp hơn. Đây là tier đủ điều kiện thấp nhất theo chính sách của bạn, không phải cam kết câu trả lời rẻ nhất.',
  ' — the best fact for that role from the model facts feed first (a public benchmark, named with its date in the response), then the models with no fact by tier. A fact older than a year does not rank. Facts off, or none kept: by tier alone.':
    ' — dữ kiện tốt nhất cho vai trò đó từ nguồn dữ kiện model trước (một benchmark công khai, ghi tên kèm ngày trong phản hồi), rồi tới các model chưa có dữ kiện, xếp theo tier. Dữ kiện cũ hơn một năm không được xếp hạng. Tắt dữ kiện, hoặc không giữ dữ kiện nào: chỉ xếp theo tier.',
  'An Anthropic request that asks for thinking or an effort level goes only to Anthropic-API models known to take it; their model list says so (Providers → Check & Refresh Models). Claude Code asks for adaptive thinking.':
    'Request Anthropic có yêu cầu thinking hoặc mức effort chỉ đi tới model API Anthropic đã biết là nhận nó; danh sách model của họ cho biết điều này (Nhà cung cấp → Kiểm tra & làm mới model). Claude Code yêu cầu adaptive thinking.',
  'A model takes part only for what it is known to support: set tiers, context sizes and the rest under Providers → Auto… (OpenRouter’s list fills most of it in). Unknown counts as missing. A session stays on the model that first answered it.':
    'Model chỉ được chọn cho những gì đã biết chắc là nó hỗ trợ: đặt tier, context và các thông tin khác ở Nhà cung cấp → Auto… (danh sách của OpenRouter tự điền phần lớn). Chưa rõ thì coi như không có. Một phiên giữ nguyên model đã trả lời nó đầu tiên.',
  'Loading API keys…': 'Đang tải API key…',
  'API Keys': 'API key',
  'No API Keys Yet': 'Chưa có API key',
  'Every app that sends requests through the gateway needs a Pygg API key. Give each one its own, so you can see what each spends and revoke one without touching the others. CLI Tools makes them for Codex and OpenCode.':
    'Mỗi ứng dụng gửi request qua gateway cần một API key Pygg. Hãy cấp riêng cho từng ứng dụng để thấy mỗi cái tiêu bao nhiêu và thu hồi một cái mà không ảnh hưởng cái khác. Công cụ CLI tự tạo key cho Codex và OpenCode.',
  'Pygg’s own keys for your apps, not your providers’ keys.':
    'Key riêng của Pygg cho các ứng dụng của bạn, không phải key của nhà cung cấp.',
  'Revoke {name}': 'Thu hồi {name}',
  'Show reads a key back from the Windows Credential Manager; the store keeps only its fingerprint. Keys made before this version can’t be shown: create a new one and revoke the old. A deleted key’s past requests keep its name in Usage.':
    'Hiển thị đọc lại key từ Windows Credential Manager; kho dữ liệu chỉ giữ dấu vân tay của nó. Key tạo trước phiên bản này không hiển thị được: hãy tạo key mới và thu hồi key cũ. Request cũ của key đã xóa vẫn giữ tên của nó trong Mức dùng.',
  'Hide': 'Ẩn',
  'Show the key for {name}': 'Hiện key của {name}',
  'Reading…': 'Đang đọc…',
  'Made before Pygg kept a copy in the Credential Manager, or set in a tool’s own configuration.':
    'Được tạo trước khi Pygg giữ bản sao trong Credential Manager, hoặc được đặt trong cấu hình riêng của một công cụ.',
  'not viewable': 'không xem được',
  'Strict: each request is held at the most it could cost.':
    'Nghiêm ngặt: mỗi request được giữ chỗ bằng mức tối đa nó có thể tốn.',
  'Estimated: counted as requests finish; one last request can pass it.':
    'Ước tính: tính khi request hoàn tất; request cuối cùng có thể vượt hạn mức.',
  'Daily limit reached: {amount} today (UTC); refused until 00:00 UTC':
    'Đã chạm hạn mức ngày: {amount} hôm nay (UTC); bị từ chối tới 00:00 UTC',
  'Today (UTC): {amount} of {limit}': 'Hôm nay (UTC): {amount} trên {limit}',
  '{amount} a session': '{amount} mỗi phiên',
  '{amount} a day': '{amount} mỗi ngày',
  'auto at least {tier}': 'auto tối thiểu {tier}',
  'estimated limit': 'giới hạn ước tính',
  'no paid retries after a server error': 'không thử lại trả phí sau lỗi máy chủ',
  'API Key for {name}': 'API key cho {name}',
  'Saved in the Windows Credential Manager: API Keys → use Show to reveal it again later. The store keeps only its fingerprint.':
    'Đã lưu trong Windows Credential Manager: vào API key → dùng Hiển thị để xem lại sau. Kho dữ liệu chỉ giữ dấu vân tay của nó.',
  'Copy it now. The Credential Manager could not keep a copy, so it can’t be shown again.':
    'Hãy sao chép ngay. Credential Manager không lưu được bản sao, nên không thể hiển thị lại.',
  'Copy API Key': 'Sao chép API key',
  'The snippets in CLI Tools → Set Up by Hand read it from PYGG_API_KEY. To set that for your user:':
    'Các đoạn cấu hình trong Công cụ CLI → Cài thủ công đọc nó từ PYGG_API_KEY. Để đặt biến này cho tài khoản của bạn:',
  'Create API Key': 'Tạo API key',
  'Default: {amount}': 'Mặc định: {amount}',
  'Policy': 'Chính sách',
  'From Policies → Defaults for New API Keys; change it for this key afterwards with Limits….':
    'Lấy từ Chính sách → Mặc định cho API key mới; sau đó đổi riêng cho key này bằng Giới hạn….',
  'Couldn’t read the defaults: {error}': 'Không đọc được giá trị mặc định: {error}',
  'Reading the defaults…': 'Đang đọc giá trị mặc định…',
  'Session limit': 'Hạn mức mỗi phiên',
  'Leave empty for no limit. One routing session: a launch from pygg run, or whatever sends the same x-pygg-session header. Once its counted spend reaches the limit, its next request is refused with 402; requests already running are never cut, so it can end a little over. Strict holds each request at the most it could cost instead. Requests that name no session are held to the other limits only.':
    'Để trống nếu không giới hạn. Một phiên định tuyến: một lần chạy từ pygg run, hoặc bất cứ thứ gì gửi cùng header x-pygg-session. Khi chi tiêu được tính của phiên chạm hạn mức, request kế tiếp bị từ chối với mã 402; request đang chạy không bao giờ bị cắt, nên có thể vượt một chút. Chế độ nghiêm ngặt thì giữ chỗ mỗi request bằng mức tối đa nó có thể tốn. Request không ghi phiên chỉ chịu các hạn mức khác.',
  'Leave empty for no limit. Resets at 00:00 UTC. Once its counted spend reaches the limit, every request from this key is refused with 402 until then; requests already running are never cut.':
    'Để trống nếu không giới hạn. Đặt lại lúc 00:00 UTC. Khi chi tiêu được tính chạm hạn mức, mọi request từ key này bị từ chối với mã 402 cho tới lúc đó; request đang chạy không bao giờ bị cắt.',
  'Requests go only to connections whose terms you recorded as not training on requests, under Providers → Advanced → Data use. Not recorded counts as may train.':
    'Request chỉ đi tới các kết nối mà bạn đã ghi là không huấn luyện trên request, ở Nhà cung cấp → Nâng cao → Sử dụng dữ liệu. “Chưa ghi” được coi là “có thể huấn luyện”.',
  'Delete {n} revoked key': 'Xóa {n} key đã thu hồi',
  'Delete {n} revoked keys': 'Xóa {n} key đã thu hồi',
  'Repeated Tool Output': 'Output tool lặp lại',
  'measured only · {share}% of input': 'chỉ đo lường · {share}% đầu vào',
  '≈{n} tokens': '≈{n} token',
  '{share}% of input': '{share}% đầu vào',
  '{cached}% of their input came from the prompt cache': '{cached}% đầu vào của chúng đến từ prompt cache',
  'Measured only: Pygg changes nothing in what agents send. Removing repeated output would also rewrite the agent’s history and break its prompt cache, which already serves most of this at a discount.':
    'Chỉ đo lường: Pygg không thay đổi gì trong những gì agent gửi. Bỏ output lặp lại cũng sẽ viết lại lịch sử của agent và làm hỏng prompt cache của nó, vốn đã phục vụ phần lớn lượng này với giá ưu đãi.',
  '{n} request repeated a tool’s output': '{n} request lặp lại output của tool',
  '{n} requests repeated a tool’s output': '{n} request lặp lại output của tool',
  'of {n} with tool output': 'trên {n} có output của tool',
  'nothing to clear': 'không có gì để xóa',
  'would save {amount}': 'sẽ tiết kiệm {amount}',
  'would cost {amount} more': 'sẽ tốn thêm {amount}',
  'Clearing Old Tool Results': 'Xóa kết quả tool cũ',
  'Nothing to clear': 'Không có gì để xóa',
  'net, after the cache writes clearing would cause': 'ròng, sau các lần ghi cache mà việc xóa gây ra',
  '≈{n} tokens of old tool results not sent: {amount} less': '≈{n} token kết quả tool cũ không được gửi: bớt {amount}',
  '≈{n} tokens written to the cache again: {amount} more': '≈{n} token được ghi lại vào cache: thêm {amount}',
  'Measured only, for OpenAI-format clients such as Codex; requests in Anthropic’s format, as Claude Code and OpenCode send them, are never measured or changed. At Anthropic’s defaults: past 100,000 prompt tokens, every tool result but the last three would be cleared. Each time one more is cleared, the rest of the conversation is written to the cache again. Clearing would also take those results away from the model, which this does not measure. Pygg will offer it only if this stays above zero.':
    'Chỉ đo lường, cho client định dạng OpenAI như Codex; request định dạng Anthropic, như Claude Code và OpenCode gửi, không bao giờ bị đo hay thay đổi. Theo mặc định của Anthropic: khi vượt 100.000 token prompt, mọi kết quả tool trừ ba kết quả cuối sẽ bị xóa. Mỗi lần xóa thêm một kết quả, phần còn lại của hội thoại lại được ghi vào cache. Việc xóa cũng lấy đi các kết quả đó khỏi model, điều mà phép đo này không tính. Pygg chỉ đề xuất nếu con số này vẫn trên 0.',
  '{n} request past 100,000 prompt tokens': '{n} request vượt 100.000 token prompt',
  '{n} requests past 100,000 prompt tokens': '{n} request vượt 100.000 token prompt',
  '{n} request on a model without a price is not in the money':
    '{n} request trên model chưa có giá không được tính vào số tiền',
  '{n} requests on models without a price are not in the money':
    '{n} request trên model chưa có giá không được tính vào số tiền',
  '{client} has used {used} of its {limit} monthly limit; its requests are refused until next month or a higher limit.':
    '{client} đã dùng {used} trên hạn mức tháng {limit}; request của nó bị từ chối cho tới tháng sau hoặc khi nâng hạn mức.',
  '{client} has used {used} of its {limit} monthly limit.': '{client} đã dùng {used} trên hạn mức tháng {limit}.',
  'The last 24 hours cost {today}, against {average} a day over the week before.':
    '24 giờ qua tốn {today}, so với trung bình {average} mỗi ngày trong tuần trước đó.',
  '{failed} of the last hour’s {total} requests failed.': '{failed} trên {total} request trong giờ qua bị lỗi.',
  'Worth a look': 'Nên xem qua',
  'Review unpriced models': 'Xem model chưa có giá',
  '{n} request in the last 24 hours has no known cost: give its model a price in Providers.':
    '{n} request trong 24 giờ qua chưa rõ chi phí: hãy đặt giá cho model của nó trong Nhà cung cấp.',
  '{n} requests in the last 24 hours have no per-request cost: review models without a price, and subscription usage.':
    '{n} request trong 24 giờ qua không có chi phí theo từng request: hãy xem các model chưa có giá và mức dùng gói thuê bao.',
  'Codex CLI': 'Codex CLI',
  'Another skill of this name': 'Có skill khác cùng tên',
  'Codex and OpenCode': 'Codex và OpenCode',
  'Edited there': 'Đã sửa ở đó',
  'Not installed': 'Chưa cài',
  'Older version': 'Bản cũ hơn',
  'Free first': 'Ưu tiên miễn phí',
  'Try models on this computer and free tiers before paid ones; paid models need a known price.':
    'Thử model trên máy này và gói miễn phí trước model trả phí; model trả phí phải có giá.',
  'Use my plans first': 'Ưu tiên gói của tôi',
  'Spend the quota you already pay for, then free, then paid as a backup.':
    'Dùng trước phần quota bạn đã trả tiền, rồi đến miễn phí, cuối cùng là trả phí để dự phòng.',
  'Subscription only': 'Chỉ gói thuê bao',
  'Only your plans. When their quota is out, the request fails instead of spilling into paid usage.':
    'Chỉ dùng các gói của bạn. Khi hết quota, request thất bại thay vì chuyển sang dùng trả phí.',
  'Strong coding, cheap backup': 'Code mạnh, dự phòng rẻ',
  'The strongest model first, then affordable backups that can still do the work (128k context or more).':
    'Model mạnh nhất trước, rồi các model dự phòng giá vừa phải vẫn làm được việc (context từ 128k).',
  'No metered spend': 'Không tính phí theo lượng',
  'Free and local models only: no per-token charge and no plan quota. Local runs still use your electricity.':
    'Chỉ model miễn phí và cục bộ: không tính phí theo token, không tốn quota gói. Chạy cục bộ vẫn tốn điện của bạn.',
  'Local only': 'Chỉ cục bộ',
  'Keep code on this computer: models served from 127.0.0.1 or localhost only.':
    'Giữ code trên máy này: chỉ dùng model chạy ở 127.0.0.1 hoặc localhost.',
  'Long-context coding': 'Code với context dài',
  'Every member takes large requests (200k context or more), cheapest first.':
    'Mọi model trong combo nhận được request lớn (context từ 200k), rẻ nhất trước.',
  'Cheapest for each request': 'Rẻ nhất cho mỗi request',
  'Qualifying models re-ordered for every request by its estimated cost: free, local and plan first, unknown prices last.':
    'Các model đủ điều kiện được sắp lại cho mỗi request theo chi phí ước tính: miễn phí, cục bộ và gói trước, giá chưa rõ sau cùng.',
  'Why the cache was missed: {n} answer': 'Vì sao không trúng cache: {n} câu trả lời',
  'Why the cache was missed: {n} answers': 'Vì sao không trúng cache: {n} câu trả lời',
};
