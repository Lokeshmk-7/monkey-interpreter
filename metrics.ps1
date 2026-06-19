<#
=====================================================================
 metrics.ps1  -  Reproduce the Section-5 metrics for the Rust port.

 Recomputes every AUTO-COMPUTABLE metric from the final artifact and the
 toolchain, and prints the two dual-audience buckets exactly as in the
 experiment report. Log-derived process metrics (AFR, CMS, TE-hours) are
 printed with their documented values and a note on how they were captured,
 because they come from the development trace, not the final code.

 Run from the repository root:
     powershell -ExecutionPolicy Bypass -File .\metrics.ps1
 Optional switches:
     -SkipTests         skip `cargo test` (FC test count)
     -SkipDifferential  skip the Go-vs-Rust differential build/run
     -SkipClippy        skip `cargo clippy` (ID)
     -Csv <path>        also write a CSV (default: metrics_report.csv)

 Requires: Rust/Cargo (+clippy), and Go (only for FC-differential + LOCD).
=====================================================================
#>
param(
  [switch]$SkipTests,
  [switch]$SkipDifferential,
  [switch]$SkipClippy,
  [string]$Csv = "metrics_report.csv"
)

# NOTE: must be "Continue", not "Stop": native tools (cargo/go) print progress to
# stderr, and under "Stop" PowerShell 5.1 turns those stderr lines into terminating
# errors even when the tool exits 0.
$ErrorActionPreference = "Continue"
$root      = $PSScriptRoot
$rustSrc   = Join-Path $root "rust\src"
$rustTests = Join-Path $root "rust\tests"
$cargo     = Join-Path $root "rust\Cargo.toml"
$rows      = @()   # for CSV: @{Bucket;Metric;Value;Source}

function Add-Row($bucket, $metric, $value, $source) {
  $script:rows += [pscustomobject]@{ Bucket=$bucket; Metric=$metric; Value=$value; Source=$source }
}

# ---------- LOC primitives -------------------------------------------------
# physical = all lines; nonblank = lines with non-whitespace;
# comment  = lines whose first non-space token starts a comment;
# code     = nonblank - comment.
function Measure-Loc {
  param([string[]]$Files, [string]$CommentRegex)
  $physical = 0; $nonblank = 0; $comment = 0
  foreach ($f in $Files) {
    $lines = Get-Content -LiteralPath $f
    $physical += $lines.Count
    $nonblank += ($lines | Where-Object { $_ -match '\S' }).Count
    $comment  += ($lines | Where-Object { $_ -match $CommentRegex }).Count
  }
  [pscustomobject]@{ Physical=$physical; NonBlank=$nonblank; Comment=$comment; Code=($nonblank-$comment) }
}

function Count-Matches {
  param([string[]]$Files, [string]$Pattern)
  # Use -Path (read file CONTENTS); piping $Files would search the path strings.
  $sum = (Select-String -Path $Files -Pattern $Pattern -AllMatches |
            ForEach-Object { $_.Matches.Count } | Measure-Object -Sum).Sum
  if ($null -eq $sum) { 0 } else { $sum }
}

$rustSrcFiles   = Get-ChildItem -Path $rustSrc   -Recurse -Filter *.rs | ForEach-Object FullName
$rustTestFiles  = Get-ChildItem -Path $rustTests -Recurse -Filter *.rs | ForEach-Object FullName
$goImplFiles    = Get-ChildItem -Path $root -Recurse -Filter *.go |
                    Where-Object { $_.FullName -notlike '*\rust\*' -and $_.Name -notlike '*_test.go' } |
                    ForEach-Object FullName

$rustCommentRe = '^\s*(//|/\*|\*)'
$goCommentRe   = '^\s*//'

$rustLoc = Measure-Loc -Files $rustSrcFiles  -CommentRegex $rustCommentRe
$goLoc   = Measure-Loc -Files $goImplFiles   -CommentRegex $goCommentRe
$testLoc = Measure-Loc -Files $rustTestFiles -CommentRegex $rustCommentRe

$codeKloc = [math]::Round($rustLoc.Code / 1000.0, 3)

Write-Host ""
Write-Host "==============================================================="
Write-Host " METRICS REPORT  -  Monkey interpreter (Rust port)"
Write-Host " Taxonomy: Class I (C0xM0), TCS ~3   |   $(Get-Date -Format s)"
Write-Host "==============================================================="

# =====================================================================
#  BUCKET A  -  TRANSPILED-CODE QUALITY (the product)
# =====================================================================
Write-Host ""
Write-Host "----- A. TRANSPILED-CODE QUALITY (product) --------------------"

# ---- FC : Functional Correctness ----
$fcTests = "skipped"
if (-not $SkipTests) {
  Write-Host "  [FC] running cargo test ..." -ForegroundColor DarkGray
  $testOut = & cargo test --manifest-path $cargo 2>&1
  $passed = 0; $failed = 0
  foreach ($m in ([regex]'test result: ok\. (\d+) passed; (\d+) failed').Matches($testOut -join "`n")) {
    $passed += [int]$m.Groups[1].Value; $failed += [int]$m.Groups[2].Value
  }
  $fcTests = "$passed passed / $failed failed"
}
Add-Row A "FC - cargo test" $fcTests "cargo test (this run)"

# ---- FC : differential (Go vs Rust) ----
$diffResult = "skipped"
if (-not $SkipDifferential -and (Get-Command go -ErrorAction SilentlyContinue)) {
  Write-Host "  [FC] differential Go vs Rust on prog.monkey ..." -ForegroundColor DarkGray
  try {
    $goExe = Join-Path $env:TEMP "monkey_go_metrics.exe"
    & go build -o $goExe $root 2>&1 | Out-Null
    & cargo build --quiet --manifest-path $cargo 2>&1 | Out-Null
    $rustExe = Join-Path $root "rust\target\debug\monkey.exe"
    $prog = Join-Path $root "prog.monkey"
    $go  = & $goExe   $prog 2>$null
    $rs  = & $rustExe $prog 2>$null
    $mismatch = if (($go -join "`n") -eq ($rs -join "`n")) { 0 } else { 1 }
    $diffResult = if ($mismatch -eq 0) { "IDENTICAL (0 mismatches)" } else { "MISMATCH" }
    Remove-Item $goExe -ErrorAction SilentlyContinue
  } catch { $diffResult = "error: $($_.Exception.Message)" }
}
Add-Row A "FC - differential" $diffResult "go build + rust + diff prog.monkey"

# ---- MS : Memory Safety (STR primary, UBR secondary) ----
$unsafeCount = Count-Matches -Files $rustSrcFiles -Pattern '\bunsafe\b'
$fnCount     = Count-Matches -Files $rustSrcFiles -Pattern '\bfn \w+'
$str = if ($rustLoc.Code -gt 0) { [math]::Round(100.0 * (1 - ($unsafeCount / $rustLoc.Code)), 2) } else { 100 }
$ubr = if ($fnCount -gt 0) { [math]::Round($unsafeCount / $fnCount, 3) } else { 0 }
Add-Row A "MS - STR (% safe)"  "$str %"  "1 - unsafe_lines/code_lines"
Add-Row A "MS - UBR"            $ubr      "unsafe_blocks/functions ($unsafeCount/$fnCount)"

# ---- MT : Maintainability (AO + CC proxy) ----
$ao = if ($rustLoc.Code -gt 0) { [math]::Round(100.0 * $rustLoc.Comment / $rustLoc.Code, 1) } else { 0 }
$decisionPattern = '\bif\b|\bwhile\b|\bfor\b| => |&&|\|\||\bmatch\b'
$decisionPoints  = Count-Matches -Files $rustSrcFiles -Pattern $decisionPattern
$ccProxy = if ($fnCount -gt 0) { [math]::Round(($decisionPoints / $fnCount) + 1, 1) } else { 0 }
Add-Row A "MT - Annotation Overhead" "$ao %" "comment_LOC/code_LOC ($($rustLoc.Comment)/$($rustLoc.Code))"
Add-Row A "MT - Cyclomatic (proxy)"  "$ccProxy avg" "decision_points/functions+1 ($decisionPoints/$fnCount)"

# ---- TD : Dependencies ----
$cargoText = Get-Content -LiteralPath $cargo -Raw
# count entries under [dependencies] (naive: lines of form `name = ...` after the header)
$depCount = 0
$inDeps = $false
foreach ($line in (Get-Content -LiteralPath $cargo)) {
  if ($line -match '^\s*\[dependencies\]')      { $inDeps = $true;  continue }
  if ($line -match '^\s*\[' -and $inDeps)       { $inDeps = $false }
  if ($inDeps -and $line -match '^\s*[A-Za-z0-9_-]+\s*=') { $depCount++ }
}
$dgs = "n/a"
if (Get-Command cargo -ErrorAction SilentlyContinue) {
  try {
    $tree = & cargo tree --manifest-path $cargo --edges normal 2>$null
    # transitive deps = total tree lines minus the root crate line
    $dgs = [math]::Max(0, ($tree | Where-Object { $_ -match '\S' }).Count - 1)
  } catch { $dgs = "n/a" }
}
$ec = if ($depCount -eq 0) { "100 % (0 deps - trivially complete)" } else { "review" }
Add-Row A "TD - Direct deps (DCD)"      $depCount "Cargo.toml [dependencies]"
Add-Row A "TD - Transitive (DGS)"       $dgs      "cargo tree"
Add-Row A "TD - Ecosystem coverage (EC)" $ec      "DCD=0 => complete"

# ---- ID : Idiomacy (clippy density) ----
$idDensity = "skipped"
if (-not $SkipClippy -and (Get-Command cargo -ErrorAction SilentlyContinue)) {
  Write-Host "  [ID] running cargo clippy --all-targets ..." -ForegroundColor DarkGray
  $clip = & cargo clippy --manifest-path $cargo --all-targets 2>&1
  $warn = ($clip | Select-String -Pattern '^warning: ' |
             Where-Object { $_.Line -notmatch 'generated \d+ warning' }).Count
  $idDensity = "$warn warnings ; $([math]::Round($warn / [math]::Max($codeKloc,0.001),2)) /KLOC (baseline 21/KLOC)"
}
Add-Row A "ID - Clippy lint density" $idDensity "cargo clippy / KLOC"

# ---- LOCD : LOC Delta ----
$locdCode = if ($goLoc.Code -gt 0) { [math]::Round($rustLoc.Code / $goLoc.Code, 3) } else { "n/a" }
$locdPhys = if ($goLoc.Physical -gt 0) { [math]::Round($rustLoc.Physical / $goLoc.Physical, 3) } else { "n/a" }
Add-Row A "LOCD - LOC delta (code)"     "$($locdCode)x"  "rust_code/go_code ($($rustLoc.Code)/$($goLoc.Code))"
Add-Row A "LOCD - LOC delta (physical)" "$($locdPhys)x"  "rust_phys/go_phys ($($rustLoc.Physical)/$($goLoc.Physical))"

# echo the computed Bucket-A values to the console
$rows | Where-Object Bucket -eq 'A' | Format-Table Metric,Value -AutoSize | Out-String | Write-Host

# =====================================================================
#  BUCKET B  -  TRANSPILATION-PROCESS QUALITY (the process)
# =====================================================================
Write-Host ""
Write-Host "----- B. TRANSPILATION-PROCESS QUALITY (process) --------------"

# ---- TE : effort by phase (LOC objective; hours/iterations from log) ----
Write-Host "  [TE] per-file code LOC (objective part; hours from manual log):"
$phaseTable = @()
foreach ($f in ($rustSrcFiles | Sort-Object)) {
  $loc = Measure-Loc -Files @($f) -CommentRegex $rustCommentRe
  $rel = $f.Replace($root + '\','')
  $phaseTable += [pscustomobject]@{ File=$rel; Code=$loc.Code; Comment=$loc.Comment }
}
$phaseTable | Format-Table -AutoSize | Out-String | Write-Host
Add-Row B "TE - total src code LOC" $rustLoc.Code "sum of per-file code LOC"
Add-Row B "TE - iterations / hours" "from dev log (Appendix A)" "manual daily log"

# ---- AFR : log-derived (optional git estimate) ----
$afrDoc = "0.44 % (9 transpiler-side lines / $($rustLoc.Code) code LOC) - from dev trace"
$afrGit = $null
if (Get-Command git -ErrorAction SilentlyContinue) {
  $tag = (& git -C $root tag --list "green-baseline" 2>$null)
  if ($tag) {
    $stat = & git -C $root diff --numstat green-baseline -- rust/src 2>$null
    $changed = ($stat | ForEach-Object { ($_ -split '\t')[0] } | Where-Object { $_ -match '^\d+$' } | Measure-Object -Sum).Sum
    $afrGit = "$([math]::Round(100.0*$changed/[math]::Max($rustLoc.Code,1),2)) % (git: $changed lines changed in rust/src since tag 'green-baseline')"
  }
}
Add-Row B "AFR (documented)" $afrDoc "development trace"
if ($afrGit) { Add-Row B "AFR (git estimate)" $afrGit "git diff since 'green-baseline' tag" }
Write-Host "  [AFR] $afrDoc"
if ($afrGit) { Write-Host "  [AFR] $afrGit" } else {
  Write-Host "  [AFR] (to auto-estimate: tag the first green build `git tag green-baseline`, then re-run)" -ForegroundColor DarkGray
}

# ---- CMS : manual classification (documented) ----
$cms = "mappable 100 % (22/22) ; weighted 0.76 - see construct_mappability.md"
Add-Row B "CMS (manual classification)" $cms "analyst judgement (construct table)"
Write-Host "  [CMS] $cms"

# ---- TME : toolchain migration ----
$cargoLines = (Get-Content -LiteralPath $cargo | Where-Object { $_ -match '\S' }).Count
$goMod = Join-Path $root "go.mod"
$goModLines = if (Test-Path $goMod) { (Get-Content -LiteralPath $goMod | Where-Object { $_ -match '\S' }).Count } else { 0 }
$tme = "Cargo.toml $cargoLines lines <- go.mod $goModLines lines ; deps ported = 0"
Add-Row B "TME - toolchain migration" $tme "manifest line counts"
Write-Host "  [TME] $tme"

# ---- supporting: test LOC ----
Add-Row "info" "Rust test LOC (physical/code)" "$($testLoc.Physical)/$($testLoc.Code)" "tests/"

# =====================================================================
#  CSV + summary
# =====================================================================
if ($Csv) {
  $rows | Export-Csv -LiteralPath (Join-Path $root $Csv) -NoTypeInformation -Encoding UTF8
  Write-Host ""
  Write-Host "CSV written to: $Csv"
}
Write-Host ""
Write-Host "Done."
