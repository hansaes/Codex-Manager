$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$files = @(
  (Join-Path $repoRoot 'docker/Dockerfile.service'),
  (Join-Path $repoRoot 'docker/Dockerfile.service.release')
)

$failed = $false
foreach ($file in $files) {
  $content = Get-Content -Raw $file
  if ($content -notmatch 'apt-get install -y --no-install-recommends[\s\\`\r\n]+ca-certificates wget curl(?:\s|\\|\r|\n)') {
    Write-Error "expected runtime dependencies to include curl in $file"
    $failed = $true
  }
}

if ($failed) {
  exit 1
}

Write-Host 'OK: service Dockerfiles include curl in runtime dependencies.'
