@echo off
REM ===========================================================================
REM  reproduce.bat  —  Appendix B reproduction package (Windows / cmd.exe)
REM
REM  Reproduces every number in the experiment report:
REM    * Go baseline build + tests
REM    * Rust port build + 94 tests
REM    * Differential testing (byte-identical equivalence) via `fc`
REM    * Determinism check (3 repeated Rust runs)
REM    * Output-quality + process metrics (LOC, unsafe, clippy, deps)
REM
REM  Run from the repository root:  reproduce.bat
REM  Requires: Go 1.22+, Rust/Cargo 1.82+ on PATH.
REM ===========================================================================
setlocal enabledelayedexpansion
set ROOT=%~dp0
cd /d "%ROOT%"
set OUT=%ROOT%repro_out
if not exist "%OUT%" mkdir "%OUT%"

echo.
echo ============================================================
echo [0/8] Toolchain versions
echo ============================================================
go version
cargo --version

echo.
echo ============================================================
echo [1/8] Go baseline: build + tests
echo   (go.mod module path must equal the import prefix
echo    github.com/skatsuta/monkey-interpreter)
echo ============================================================
go build -o "%OUT%\monkey_go.exe" .
if errorlevel 1 ( echo GO BUILD FAILED & goto :end )
go test ./...

echo.
echo ============================================================
echo [2/8] Rust port: build + full test suite (expect 94 passed)
echo ============================================================
cargo build --manifest-path rust\Cargo.toml
if errorlevel 1 ( echo CARGO BUILD FAILED & goto :end )
cargo test  --manifest-path rust\Cargo.toml

echo.
echo ============================================================
echo [3/8] Differential testing: Go vs Rust on prog.monkey
echo       (FC - Functional Correctness)
echo ============================================================
"%OUT%\monkey_go.exe" prog.monkey            > "%OUT%\out_go.txt"
rust\target\debug\monkey.exe prog.monkey     > "%OUT%\out_rust.txt"
fc "%OUT%\out_go.txt" "%OUT%\out_rust.txt"
if errorlevel 1 ( echo *** DIFFERENTIAL MISMATCH *** ) else ( echo IDENTICAL OUTPUT - Go and Rust agree )

echo.
echo ============================================================
echo [4/8] Determinism: 3 repeated Rust runs must be identical
echo ============================================================
rust\target\debug\monkey.exe prog.monkey > "%OUT%\det1.txt"
rust\target\debug\monkey.exe prog.monkey > "%OUT%\det2.txt"
rust\target\debug\monkey.exe prog.monkey > "%OUT%\det3.txt"
fc "%OUT%\det1.txt" "%OUT%\det2.txt" >nul && fc "%OUT%\det2.txt" "%OUT%\det3.txt" >nul
if errorlevel 1 ( echo *** NON-DETERMINISTIC *** ) else ( echo DETERMINISTIC - 3/3 runs identical )

echo.
echo ============================================================
echo [5/8] LOC metrics  (LOCD - LOC Delta, MT - Annotation Overhead)
echo ============================================================
REM NOTE: `Measure-Object -Line` ignores blank lines, so this reports NON-BLANK
REM lines. code-LOC = (non-blank) - (doc/comment). See report Section 5 / Appendix A.
echo Go impl (non-blank lines):
powershell -NoProfile -Command "(Get-ChildItem -Recurse -Filter *.go | Where-Object { $_.FullName -notlike '*\rust\*' -and $_.Name -notlike '*_test.go' } | Get-Content | Measure-Object -Line).Lines"
echo Go impl (doc/comment lines):
powershell -NoProfile -Command "(Get-ChildItem -Recurse -Filter *.go | Where-Object { $_.FullName -notlike '*\rust\*' -and $_.Name -notlike '*_test.go' } | Get-Content | Where-Object { $_ -match '^\s*//' } | Measure-Object).Count"
echo Rust src (non-blank lines):
powershell -NoProfile -Command "(Get-ChildItem rust\src -Recurse -Filter *.rs | Get-Content | Measure-Object -Line).Lines"
echo Rust src doc/comment lines:
powershell -NoProfile -Command "(Get-ChildItem rust\src -Recurse -Filter *.rs | Get-Content | Where-Object { $_ -match '^\s*(//|/\*|\*)' } | Measure-Object).Count"

echo.
echo ============================================================
echo [6/8] Memory Safety  (MS: UBR + Safe Transpilation Ratio)
echo ============================================================
echo unsafe blocks in rust\src (expect 0):
powershell -NoProfile -Command "(Get-ChildItem rust\src -Recurse -Filter *.rs | Select-String -Pattern '\bunsafe\b' | Measure-Object).Count"
echo function definitions in rust\src:
powershell -NoProfile -Command "(Get-ChildItem rust\src -Recurse -Filter *.rs | Select-String -Pattern '\bfn \w+' | Measure-Object).Count"

echo.
echo ============================================================
echo [7/8] Idiomacy  (ID: clippy density vs 21/KLOC baseline)
echo ============================================================
cargo clippy --manifest-path rust\Cargo.toml --all-targets

echo.
echo ============================================================
echo [8/8] Third-party dependencies  (TD)
echo ============================================================
echo Direct crate dependencies declared in Cargo.toml:
powershell -NoProfile -Command "$d = Select-String -Path rust\Cargo.toml -Pattern '^\s*[a-zA-Z0-9_-]+\s*=' | Where-Object { $_.Line -notmatch 'name|version|edition|description|license|path' }; if ($d) { $d.Count } else { 0 }"
echo (0 = std-library only)

echo.
echo ============================================================
echo Done. Per-run artifacts are in: %OUT%
echo ============================================================
:end
endlocal
