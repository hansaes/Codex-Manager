Remove-Item Env:CC -ErrorAction SilentlyContinue
Remove-Item Env:CXX -ErrorAction SilentlyContinue
$env:TAURI_CONFIG = '{"identifier":"com.codexmanager.desktop.dev3015","productName":"CodexManager Dev 3015","build":{"devUrl":"http://127.0.0.1:3015"}}'
Write-Host "TAURI_CONFIG=$env:TAURI_CONFIG"
cargo run --manifest-path apps/src-tauri/Cargo.toml
