# RS-EC インストールスクリプト(Windows / Windows Server 共通)。
#
# 使い方(管理者権限のPowerShellで):
#   Invoke-WebRequest -Uri "https://github.com/aon-co-jp/RS-EC/releases/latest/download/rs-ec-windows-x86_64.zip" -OutFile rs-ec.zip
#   Expand-Archive rs-ec.zip -DestinationPath rs-ec
#   cd rs-ec
#   .\install.ps1

#Requires -RunAsAdministrator

$ErrorActionPreference = "Stop"

$InstallDir = "C:\Program Files\RS-EC"
$DataDir = "C:\ProgramData\RS-EC\data"
$ServiceName = "RSEC"

Write-Host "==> インストール先: $InstallDir"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null

$BinSrc = Join-Path $PSScriptRoot "rs-ec.exe"
if (-not (Test-Path $BinSrc)) {
    Write-Error "rs-ec.exe が見つかりません($BinSrc)。zipを展開したディレクトリで実行してください。"
    exit 1
}
Copy-Item $BinSrc -Destination $InstallDir -Force

$existing = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if ($existing) {
    Write-Host "==> 既存のWindowsサービスが見つかったため、バイナリのみ更新しました(再起動は行いません)"
    Write-Host "    手動で再起動する場合: Restart-Service $ServiceName"
} else {
    Write-Host "==> Windowsサービスとして登録($ServiceName)"
    Write-Host "    管理者メール・SMTP設定は環境変数で指定する必要があります。"
    Write-Host "    例(サービス登録前に環境変数を設定する場合、システム環境変数として設定してください):"
    Write-Host "      [Environment]::SetEnvironmentVariable('RSEC_ADMIN_EMAIL', 'admin@example.com', 'Machine')"
    Write-Host "      [Environment]::SetEnvironmentVariable('RSEC_DATA_DIR', '$DataDir', 'Machine')"
    Write-Host "      [Environment]::SetEnvironmentVariable('RSEC_PORT', '8102', 'Machine')"
    Write-Host ""
    Write-Host "    環境変数を設定した後、以下でサービス登録・起動してください:"
    Write-Host "      New-Service -Name $ServiceName -BinaryPathName '$InstallDir\rs-ec.exe' -DisplayName 'RS-EC' -StartupType Automatic"
    Write-Host "      Start-Service $ServiceName"
}

Write-Host "==> 完了。"
