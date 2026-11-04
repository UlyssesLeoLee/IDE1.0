@echo off
title IDE1.0 Editor
set IDE_SHELL_WEB_NO_AUTH=1
echo Starting IDE1.0 editor server...
start "" http://127.0.0.1:8123/editor
"E:\IDE1.0\target\debug\ide-shell-web.exe"
