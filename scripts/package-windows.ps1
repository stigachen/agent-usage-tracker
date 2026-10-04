$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = Split-Path -Parent $PSScriptRoot
$config = Get-Content -LiteralPath "$root/src-tauri/tauri.conf.json" -Raw | ConvertFrom-Json
$binary = Join-Path $root "src-tauri/target/x86_64-pc-windows-msvc/release/agent-usage-tracker.exe"
$directory = Join-Path $root "src-tauri/target/windows-portable"
$archive = Join-Path $root "src-tauri/target/Agent.Usage_$($config.version)_windows_x64_portable.zip"

if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    throw "Build the Windows executable first: pnpm tauri build --target x86_64-pc-windows-msvc --no-bundle"
}

# Stage only the executable and user instructions, never build caches or old installers.
if (Test-Path -LiteralPath $directory) {
    Remove-Item -LiteralPath $directory -Recurse -Force
}
New-Item -ItemType Directory -Path $directory | Out-Null
Copy-Item -LiteralPath $binary -Destination "$directory/Agent Usage.exe"
Copy-Item -LiteralPath "$root/docs/WINDOWS-PORTABLE.txt" -Destination "$directory/README.txt"
Compress-Archive -Path "$directory/*" -DestinationPath $archive -Force

if ($env:GITHUB_OUTPUT) {
    "directory=$directory" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
    "archive=$archive" | Out-File -FilePath $env:GITHUB_OUTPUT -Encoding utf8 -Append
}
Write-Host "Portable Windows package: $archive"
