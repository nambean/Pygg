# Real error answers from providers, saved as fixtures for the gateway's error policy (ROADMAP C1, P0-2 "fixtures per
# real provider"). Nothing here can generate text, so nothing is billed:
#   - an invalid key everywhere (each provider's own 401 body);
#   - with a real key from the environment, where there is one: an unknown model and a malformed request.
# Keys are read from the user's environment and never written: a body that contains a real key is not saved.
# Output: apps/desktop/src-tauri/tests/fixtures/provider-errors/<case>.json
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root 'apps\desktop\src-tauri\tests\fixtures\provider-errors'
New-Item -ItemType Directory -Force $out | Out-Null
$tmp = Join-Path ([IO.Path]::GetTempPath()) "pygg-errors-$(Get-Random)"
New-Item -ItemType Directory $tmp | Out-Null
function Key([string]$name) {
  foreach ($scope in 'Process', 'User') { $v = [Environment]::GetEnvironmentVariable($name, $scope); if ($v) { return $v } }
  return $null
}
$invalid = 'pygg-invalid-key-for-fixtures-0000000000'
$secrets = @('ANTHROPIC_API_KEY', 'GEMINI_API_KEY', 'QWEN_API_KEY') | ForEach-Object { Key $_ } | Where-Object { $_ }

function Capture([string]$case, [string]$url, [hashtable]$headers, $body) {
  $file = "$tmp\body.json"; [IO.File]::WriteAllText($file, ($body | ConvertTo-Json -Depth 10 -Compress))
  $cargs = @('-s', '-D', "$tmp\headers.txt", '-o', "$tmp\out.txt", '-w', '%{http_code}', '-X', 'POST', $url, '-H', 'content-type: application/json')
  foreach ($k in $headers.Keys) { $cargs += @('-H', "${k}: $($headers[$k])") }
  $status = [int](& curl.exe @cargs --data-binary "@$file")
  $text = [IO.File]::ReadAllText("$tmp\out.txt")
  foreach ($s in $secrets) { if ($text.Contains($s)) { "${case}: the body contains a real key; not saved"; return } }
  $keep = @('retry-after', 'retry-after-ms', 'content-type', 'x-should-retry')
  $hdrs = [ordered]@{}
  foreach ($line in Get-Content "$tmp\headers.txt") {
    if ($line -match '^([^:]+):\s*(.*)$' -and $keep -contains $Matches[1].ToLower()) { $hdrs[$Matches[1].ToLower()] = $Matches[2].Trim() }
  }
  $parsed = $null; try { $parsed = $text | ConvertFrom-Json } catch { }
  $fixture = [ordered]@{ case = $case; captured = (Get-Date -Format 'yyyy-MM-dd'); url = $url; status = $status; headers = $hdrs
    body = $(if ($parsed) { $parsed } else { $text }) }
  [IO.File]::WriteAllText((Join-Path $out "$case.json"), ($fixture | ConvertTo-Json -Depth 20))
  "{0,-34} {1}" -f $case, $status
}

function Chat([string]$model = 'm') { @{ model = $model; max_tokens = 1; messages = @(@{ role = 'user'; content = 'x' }) } }
$bearer = @{ Authorization = "Bearer $invalid" }
try {
  Capture 'anthropic-401' 'https://api.anthropic.com/v1/messages' @{ 'x-api-key' = $invalid; 'anthropic-version' = '2023-06-01' } (Chat 'claude-haiku-4-5')
  Capture 'gemini-401' 'https://generativelanguage.googleapis.com/v1beta/openai/chat/completions' $bearer (Chat 'gemini-2.5-flash')
  Capture 'openrouter-401' 'https://openrouter.ai/api/v1/chat/completions' $bearer (Chat)
  Capture 'openai-401' 'https://api.openai.com/v1/chat/completions' $bearer (Chat 'gpt-4.1-mini')
  Capture 'groq-401' 'https://api.groq.com/openai/v1/chat/completions' $bearer (Chat)
  Capture 'deepseek-401' 'https://api.deepseek.com/chat/completions' $bearer (Chat 'deepseek-chat')
  Capture 'mistral-401' 'https://api.mistral.ai/v1/chat/completions' $bearer (Chat)
  Capture 'qwen-401' 'https://token-plan.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1/chat/completions' $bearer (Chat)

  $anthropic = Key 'ANTHROPIC_API_KEY'
  if ($anthropic) {
    $h = @{ 'x-api-key' = $anthropic; 'anthropic-version' = '2023-06-01' }
    Capture 'anthropic-404-model' 'https://api.anthropic.com/v1/messages' $h (Chat 'claude-does-not-exist')
    Capture 'anthropic-400-request' 'https://api.anthropic.com/v1/messages' $h @{ model = 'claude-haiku-4-5'; messages = @(@{ role = 'user'; content = 'x' }) }
  }
  $gemini = Key 'GEMINI_API_KEY'
  if ($gemini) {
    Capture 'gemini-404-model' 'https://generativelanguage.googleapis.com/v1beta/openai/chat/completions' @{ Authorization = "Bearer $gemini" } (Chat 'gemini-does-not-exist')
  }
  $qwen = Key 'QWEN_API_KEY'
  if ($qwen) {
    Capture 'qwen-404-model' 'https://token-plan.ap-southeast-1.maas.aliyuncs.com/compatible-mode/v1/chat/completions' @{ Authorization = "Bearer $qwen" } (Chat 'qwen-does-not-exist')
  }
} finally {
  Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
  $anthropic = $null; $gemini = $null; $qwen = $null; $secrets = $null
}
