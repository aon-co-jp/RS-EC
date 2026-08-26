; RS-EC Windowsインストーラー(Inno Setup)。
;
; ユーザー指示「パワーシェルでインストールする関連リポジトリは全て、
; リポジトリ名-installer.exeに統一して」+「パワーシェル版とリポジトリ名
; -installer.exeの二種類をご用意して」への対応。既存の`install.ps1`
; (管理者権限のPowerShellで手動実行する方式)はそのまま維持し、これは
; それに加わる第二の選択肢——GUIでダブルクリックするだけのインストーラー。
;
; 正直な開示: 既存install.ps1は`C:\Program Files\RS-EC`への配置を
; 前提に`#Requires -RunAsAdministrator`としているが、実際に行うのは
; ファイルコピーとサービス登録コマンドの**印字のみ**(自動登録はしない)
; ——管理者権限が必須な操作ではない。本インストーラーは
; `PrivilegesRequired=lowest`とし、既定のインストール先を
; `{autopf}`(非管理者時は自動的にユーザー単位の書き込み可能な場所
; 〈%LOCALAPPDATA%\Programs〉に切り替わる、Inno Setupの標準機能)にする
; ことで、管理者権限のPowerShellを起動する手間そのものを無くした。
; Windowsサービスとして常駐させたい場合は、引き続き
; `install-service.ps1`(既存install.ps1と同一内容、同梱)を管理者権限で
; 実行する必要がある——これは変更していない。
;
; ビルド方法: リポジトリルートで`cargo build --release --bin rs-ec`を
; 実行した後、このディレクトリで`ISCC.exe rs-ec.iss`を実行する。

#define MyAppName "RS-EC"
#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-local-build"
#endif
#define MyAppPublisher "aon-co-jp"
#define MyAppURL "https://github.com/aon-co-jp/RS-EC"
#define MyAppExeName "rs-ec.exe"

[Setup]
PrivilegesRequired=lowest
AppId={{A1B2C3D4-2222-4A5B-8C6D-2E7F9A0B1C2D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2
SolidCompression=yes
OutputDir=dist
OutputBaseFilename=RS-EC-installer
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes

[Languages]
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "..\..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\install.ps1"; DestDir: "{app}"; Flags: ignoreversion; DestName: "install-service.ps1"
Source: "README-INSTALLED.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"

[Run]
; 既定ではフォアグラウンド実行として起動する(サービス化はオプション、
; install-service.ps1を管理者権限で別途実行する必要がある——README-
; INSTALLED.txt参照)。
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{app}"
