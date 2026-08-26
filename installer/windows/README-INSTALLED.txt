RS-EC — installed / インストール完了

------------------------------------------------------------------
English
------------------------------------------------------------------
RS-EC has been installed. By default it runs as a normal foreground
program (via the Start Menu shortcut / desktop launch).

To run it as a background Windows Service instead (auto-start on
boot), open PowerShell AS ADMINISTRATOR and run:
    cd "<install dir>"
    .\install-service.ps1
This will register and start the "RSEC" service.

------------------------------------------------------------------
日本語
------------------------------------------------------------------
RS-ECのインストールが完了しました。既定では通常のフォアグラウンド
プログラムとして動作します(スタートメニュー/デスクトップから起動)。

バックグラウンドのWindowsサービスとして常駐させたい場合(起動時に
自動実行)は、管理者権限でPowerShellを開き、以下を実行してください:
    cd "<インストール先>"
    .\install-service.ps1
「RSEC」サービスとして登録・起動されます。
