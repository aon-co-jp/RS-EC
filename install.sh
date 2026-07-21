#!/bin/sh
# RS-EC インストールスクリプト(AlmaLinux/Ubuntu/Debian/Fedora/RHEL等、
# systemdを使う主要Linuxディストリ共通)。
#
# 静的リンクされたmuslバイナリを使うため、ディストリ固有のライブラリ依存は
# 無い。root権限で実行すること。
#
# 使い方:
#   curl -fsSL https://github.com/aon-co-jp/RS-EC/releases/latest/download/rs-ec-linux-x86_64.tar.gz | tar xz
#   sudo ./install.sh

set -eu

BIN_SRC="$(dirname "$0")/rs-ec"
INSTALL_DIR="/usr/local/bin"
DATA_DIR="/var/lib/rs-ec"
SERVICE_FILE="/etc/systemd/system/rs-ec.service"

if [ "$(id -u)" -ne 0 ]; then
    echo "root権限で実行してください(例: sudo ./install.sh)" >&2
    exit 1
fi

if [ ! -f "$BIN_SRC" ]; then
    echo "rs-ec バイナリが見つかりません($BIN_SRC)。同梱のtar.gzを展開したディレクトリで実行してください。" >&2
    exit 1
fi

echo "==> バイナリを ${INSTALL_DIR}/rs-ec へ配置"
install -m 755 "$BIN_SRC" "${INSTALL_DIR}/rs-ec"

echo "==> データディレクトリを ${DATA_DIR} に作成"
mkdir -p "$DATA_DIR"

if [ ! -f "$SERVICE_FILE" ]; then
    echo "==> systemdサービスを作成(${SERVICE_FILE})"
    cat > "$SERVICE_FILE" << EOF
[Unit]
Description=RS-EC - EC-CUBE-equivalent Rust product catalog engine (no payment yet)
After=network.target

[Service]
Type=simple
WorkingDirectory=${DATA_DIR}
Environment=RSEC_DATA_DIR=${DATA_DIR}
Environment=RSEC_PORT=8102
# 管理者メール・SMTP設定は環境変数で指定すること(このファイルを直接
# 編集するか、/etc/systemd/system/rs-ec.service.d/override.confを
# 使うこと)。例:
#   Environment=RSEC_ADMIN_EMAIL=admin@example.com
#   Environment=RSEC_SMTP_HOST=smtp.example.com
ExecStart=${INSTALL_DIR}/rs-ec
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
else
    echo "==> 既存のsystemdサービスが見つかったため上書きしません(${SERVICE_FILE})"
fi

echo "==> 完了。次のコマンドで管理者メール等を設定してから起動してください:"
echo "    sudo systemctl edit rs-ec  # Environment=RSEC_ADMIN_EMAIL=... 等を追記"
echo "    sudo systemctl enable --now rs-ec"
