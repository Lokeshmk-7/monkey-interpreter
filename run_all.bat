@echo off
echo =========================
echo Monkey Interpreter Runner
echo =========================

echo.
echo [1] Go version
go version

echo.
echo [2] Running Go tests
go test ./...

echo.
echo [3] Building Go binary
go build -o monkey_go.exe

echo.
echo [4] Rust version
cargo --version

echo.
echo [5] Running Rust tests
cargo test

echo.
echo [6] Building Rust binary (release)
cargo build --release

echo.
echo [7] LOC count (Rust)
for /r rust\src %%f in (*.rs) do type "%%f" >> tmp_rust.txt
find /c /v "" < tmp_rust.txt
del tmp_rust.txt

echo.
echo [8] LOC count (Go)
for /r . %%f in (*.go) do type "%%f" >> tmp_go.txt
find /c /v "" < tmp_go.txt
del tmp_go.txt

echo.
echo [9] Running differential test
echo (Make sure prog.monkey exists)

monkey_go.exe prog.monkey > go_out.txt
cargo run --release -- prog.monkey > rust_out.txt

fc go_out.txt rust_out.txt

echo.
echo [10] Determinism test
for /l %%i in (1,1,3) do (
    cargo run --release -- prog.monkey
)

echo.
echo Done.
pause