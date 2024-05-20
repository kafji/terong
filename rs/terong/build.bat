@echo off

cargo build --release && ^
copy ..\target\release\terong-server.exe . && ^
copy ..\target\release\terong-client.exe .
