# Cache parity: the same requests to Anthropic directly and through the gateway (ROADMAP P0 release proof,
# counter-review question 5). Each arm sends two turns with the same cached system prompt: the first should
# write the cache, the second read it. Arms use different nonces so one cannot read the other's cache.
#
# Needs ANTHROPIC_API_KEY in the environment. Sends 8 paid requests of about 8k prompt tokens each to
# claude-haiku-4-5, half of them read from the cache: about $0.05 in all.
# Uses a scratch store; the key is put in the OS credential store for the gateway and removed at the end.
param([string]$Model = 'claude-haiku-4-5', [int]$Port = 8796)
$ErrorActionPreference = 'Stop'
if (-not $env:ANTHROPIC_API_KEY) { throw 'Set ANTHROPIC_API_KEY first.' }
$root = Split-Path $PSScriptRoot -Parent
$g = Join-Path $root 'apps\desktop\src-tauri\target\debug\gateway.exe'
if (-not (Test-Path $g)) { throw "Build the gateway first: cargo build --bin gateway (in apps\desktop\src-tauri)" }
$s = Join-Path ([IO.Path]::GetTempPath()) "str-parity-$(Get-Random)"
New-Item -ItemType Directory $s | Out-Null
$db = "$s\store.sqlite3"

function Post([string]$url, [hashtable]$headers, $body) {
  $file = "$s\body.json"; [IO.File]::WriteAllText($file, ($body | ConvertTo-Json -Depth 20 -Compress))
  $cargs = @('-s', '-D', "$s\headers.txt", '-X', 'POST', $url, '-H', 'content-type: application/json', '-H', 'anthropic-version: 2023-06-01')
  foreach ($k in $headers.Keys) { $cargs += @('-H', "${k}: $($headers[$k])") }
  $text = & curl.exe @cargs --data-binary "@$file"
  [pscustomobject]@{ Body = ($text -join "`n"); Headers = Get-Content "$s\headers.txt" }
}

# A system prompt long enough to be cached by every current model (over 4096 tokens).
$filler = (1..900 | ForEach-Object { "Rule $($_): answer in one short sentence." }) -join ' '
function Request([string]$nonce, [bool]$stream) {
  @{ model = $Model; max_tokens = 40; stream = $stream
     system = @(@{ type = 'text'; text = "Parity run $nonce. $filler"; cache_control = @{ type = 'ephemeral' } })
     messages = @(@{ role = 'user'; content = 'Say ready.' }) }
}
function Usage($response, [bool]$stream) {
  if (-not $stream) { return ($response.Body | ConvertFrom-Json).usage }
  # Streamed: input and cache counts are on message_start.
  $start = ($response.Body -split "`n" | Where-Object { $_ -like 'data: *"message_start"*' } | Select-Object -First 1)
  ($start.Substring(6) | ConvertFrom-Json).message.usage
}

$rows = @()
try {
  & $g --db $db add-connection --preset anthropic --label Anthropic --key-env ANTHROPIC_API_KEY --billing paid | Out-Null
  $token = ((& $g --db $db create-client --name Parity) | Select-Object -Last 1).Trim()
  $server = Start-Process -FilePath $g -ArgumentList @('--db', $db, 'serve', '--port', $Port) -PassThru -WindowStyle Hidden
  Start-Sleep -Seconds 1
  $arms = @(
    @{ name = 'direct'; url = 'https://api.anthropic.com/v1/messages'; headers = @{ 'x-api-key' = $env:ANTHROPIC_API_KEY } },
    @{ name = 'gateway'; url = "http://127.0.0.1:$Port/v1/messages"; headers = @{ 'x-api-key' = $token } }
  )
  foreach ($stream in $false, $true) {
    $nonce = Get-Random
    foreach ($arm in $arms) {
      $body = Request "$($arm.name)-$nonce" $stream
      if ($arm.name -eq 'gateway') { $body.model = "anthropic/$Model" }
      foreach ($turn in 1, 2) {
        $r = Post $arm.url $arm.headers $body
        $u = Usage $r $stream
        $rows += [pscustomobject]@{ arm = $arm.name; stream = $stream; turn = $turn; input = $u.input_tokens
          cache_write = $u.cache_creation_input_tokens; cache_read = $u.cache_read_input_tokens
          mode = (($r.Headers | Where-Object { $_ -like 'x-smarttaskrouter-mode:*' }) -replace '^[^:]+:\s*', '') }
      }
    }
  }
  $rows | Format-Table -AutoSize | Out-String | Write-Output
  "Ledger through the gateway:"
  & $g --db $db usage
} finally {
  if ($server) { Stop-Process -Id $server.Id -ErrorAction SilentlyContinue }
  & $g --db $db remove-connection --id anthropic | Out-Null
  Remove-Item $s -Recurse -Force -ErrorAction SilentlyContinue
}
