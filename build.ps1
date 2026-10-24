# Builds dist\KOTOR Modern Gamesave Editor.exe beside this file: one file, no installer, no other files.
#
#   powershell -ExecutionPolicy Bypass -File build.ps1
#
# Needs Rust (rustup, stable). Everything else is in this folder, so it builds on its own,
# build.rs says where the fonts are looked for. Linux and macOS build
# with build.sh beside this file.
$ErrorActionPreference = "Stop"

$here = $PSScriptRoot
$dist = Join-Path $here "dist"
$exe = Join-Path $dist "KOTOR Modern Gamesave Editor.exe"

Push-Location $here
try {
    # The tests build in their own folder with their own profile (Cargo.toml, [profile.tests]),
    # at the same time as the program, on the cores its final single-threaded pass leaves idle.
    $tests = Start-Job -ScriptBlock {
        param($dir)
        Set-Location $dir
        $env:CARGO_TARGET_DIR = Join-Path $dir "build\target-tests"
        cargo test --profile tests 2>&1
        "EXIT $LASTEXITCODE"
    } -ArgumentList $here
    cargo build --release
    $built = $LASTEXITCODE
    $out = Receive-Job -Job $tests -Wait -AutoRemoveJob
    $out | Where-Object { "$_" -match "test result|FAILED|panicked|^error" } | ForEach-Object { Write-Host $_ }
    if ($built -ne 0) { throw "the editor did not build" }
    if (-not ($out | Where-Object { "$_" -eq "EXIT 0" })) { throw "the tests failed" }
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Force -Path $dist | Out-Null
Copy-Item -LiteralPath (Join-Path $here "build\target\release\kotor-modern-gamesave-editor.exe") -Destination $exe -Force
$item = Get-Item -LiteralPath $exe
$hash = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
Write-Host ("{0}  {1:n0} bytes  SHA-256 {2}" -f $item.FullName, $item.Length, $hash)
