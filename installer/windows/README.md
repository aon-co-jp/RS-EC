# このフォルダについて / About this folder

`rs-ec.iss` はこのインストーラーを**作るための** [Inno Setup](https://jrsoftware.org/isinfo.php)
ビルドスクリプトです。`RS-EC-installer.exe` はこのフォルダ内に実体として置いてあります
(ユーザー指示により、v0.1.0でローカルビルドしたバイナリを直接コミット)。

**⬇ 今すぐダウンロード**: [RS-EC-installer.exe](RS-EC-installer.exe)

**正直な開示**: このファイルはビルド成果物であり、`rs-ec.iss`やソースコードを変更しても
自動的には更新されません(手動での再ビルド・再コミットが必要)。CIの`release.yml`は
Windows向けバイナリのzip(`rs-ec-windows-x86_64.zip`)をGitHub Releasesへ公開しますが、
この`.exe`インストーラー自体は自動ビルドしていないため、現時点でこのインストーラーを
自動ビルド・公開するCIは無く、このファイルが唯一の配布経路です。

---

`rs-ec.iss` is the [Inno Setup](https://jrsoftware.org/isinfo.php) script used to **build**
this installer. `RS-EC-installer.exe` is committed directly in this folder as the actual
binary (built locally from v0.1.0, per user request).

**⬇ Download now**: [RS-EC-installer.exe](RS-EC-installer.exe)

**Honest disclosure**: this file is a build artifact — it does not update automatically when
`rs-ec.iss` or the source code changes (it must be manually rebuilt and re-committed). The
`release.yml` CI workflow publishes a Windows binary zip (`rs-ec-windows-x86_64.zip`) to
GitHub Releases, but it does not build this `.exe` installer itself, so there is currently no
CI that auto-builds or publishes this installer — this file is the only distribution channel
for it.
