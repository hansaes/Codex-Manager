Set-Location 'D:\code\Codex-Manager\apps'
pnpm run dev:desktop *>&1 | Tee-Object -FilePath 'D:\code\Codex-Manager\.run-logs\teams-dev.log'
