# 本机没有 MSVC：用 rustup 的 GNU 工具链 + 自带 dlltool 编译
$env:PATH = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-gnu\lib\rustlib\x86_64-pc-windows-gnu\bin\self-contained;" + $env:PATH
Set-Location $PSScriptRoot
cargo +stable-x86_64-pc-windows-gnu build --release
