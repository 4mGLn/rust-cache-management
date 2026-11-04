# Live Windows smoke. Existing published bootstrap need not understand the new prefix variable in Source mode.
param([ValidateSet('Source','Published')][string]$Mode='Source',[string]$ExpectedVersion)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$root=Split-Path $PSScriptRoot -Parent
$temp=Join-Path ([IO.Path]::GetTempPath()) ('rcm-live-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path $temp
$previousInstallDir=$env:RUST_CACHE_INSTALL_DIR
$previousBinDir=$env:RUST_CACHE_BIN_DIR
$previousPath=$env:PATH
try {
    $latest=$null
    for ($attempt=0;$attempt -lt 12;$attempt++) {
        $latest=(Invoke-RestMethod -Uri 'https://api.github.com/repos/4mGLn/rust-cache-management/releases/latest' -TimeoutSec 30).tag_name
        if ($latest -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+$') { throw 'Invalid latest API version' }
        if (-not $ExpectedVersion -or $latest -ceq $ExpectedVersion) { break }
        if ($attempt -lt 11) { Start-Sleep -Seconds 5 }
    }
    if ($ExpectedVersion -and $latest -cne $ExpectedVersion) { throw 'Latest API does not match expected release' }
    $unicodeSuffix=([string][char]0xD55C)+([string][char]0xAE00)
    $env:RUST_CACHE_INSTALL_DIR=Join-Path $temp ("installed space's "+$unicodeSuffix)
    $env:RUST_CACHE_BIN_DIR=Join-Path $temp 'commands'
    $env:PATH=$env:RUST_CACHE_BIN_DIR+';'+$env:PATH
    function Invoke-Installer {
    if ($Mode -eq 'Published') {
        irm https://github.com/4mGLn/rust-cache-management/releases/latest/download/install.ps1 | iex
    } else {
        & (Join-Path $root 'install.ps1') -Prefix $env:RUST_CACHE_INSTALL_DIR
    }
    }
    Invoke-Installer
    $baseline=(Get-FileHash -LiteralPath (Join-Path $env:RUST_CACHE_INSTALL_DIR 'manage.ps1') -Algorithm SHA256).Hash
    Invoke-Installer
    if ((Get-FileHash -LiteralPath (Join-Path $env:RUST_CACHE_INSTALL_DIR 'manage.ps1') -Algorithm SHA256).Hash -cne $baseline) { throw 'Rerun rewrote manager' }
    foreach ($name in @('rust-cache-inspect.cmd','rust-cache-inspect.ps1','rust-cache-management.cmd','rust-cache-management.ps1')) {
        $file=Get-Item -LiteralPath (Join-Path $env:RUST_CACHE_BIN_DIR $name)
        if ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Expected regular command wrapper' }
    }
    rust-cache-management.cmd --help
    if ($LASTEXITCODE) { throw 'PATH alias help failed' }
    rust-cache-management.cmd
    if ($LASTEXITCODE) { throw 'PATH alias noargs failed' }
    rust-cache-management.cmd status
    if ($LASTEXITCODE) { throw 'PATH alias status failed' }
    $current=(Get-Content -Raw -LiteralPath (Join-Path $env:RUST_CACHE_INSTALL_DIR 'current')).Trim()
    if ($current -cne ($latest+'-x86_64-pc-windows-msvc')) { throw 'Activated version differs from actual latest API' }
    if (Test-Path -LiteralPath (Join-Path $env:RUST_CACHE_INSTALL_DIR 'auto-update')) { throw 'Automatic updates unexpectedly enabled' }
    $output=rust-cache-inspect.cmd --version
    if ($LASTEXITCODE -ne 0 -or ($output -join "`n") -cne ('rust-cache-inspect '+$latest.Substring(1))) { throw 'Installed executable version mismatch' }
    $cache=Join-Path $temp 'cache with spaces'
    $null=New-Item -ItemType Directory -Path $cache
    [IO.File]::WriteAllText((Join-Path $cache 'artifact'),'hello',(New-Object Text.UTF8Encoding($false)))
    $diagnostic=rust-cache-inspect.cmd --path $cache
    if ($LASTEXITCODE) { throw 'Live installed diagnostic failed' }
    $result=(($diagnostic -join "`n") | ConvertFrom-Json).paths[0]
    if ($result.logical_file_bytes -ne 5 -or $result.errors.Count -ne 0 -or -not $result.complete_scan) { throw 'Live diagnostic result mismatch' }
    if ([IO.File]::ReadAllText((Join-Path $cache 'artifact')) -cne 'hello') { throw 'Diagnostic changed fixture' }
    if ($Mode -eq 'Published') {
        & node (Join-Path $PSScriptRoot 'test-agent-setup.mjs') (Join-Path $env:RUST_CACHE_INSTALL_DIR "releases/$current/bin/rust-cache-inspect.exe") (Join-Path $env:RUST_CACHE_BIN_DIR 'rust-cache-management.ps1')
        if ($LASTEXITCODE) { throw 'Published packaged agent setup failed' }
    }
    Write-Output "Live latest Windows installer passed: mode=$Mode latest=$latest current=$current; actual scan5bytes, fixture preserved."
} finally {
    $env:RUST_CACHE_INSTALL_DIR=$previousInstallDir
    $env:RUST_CACHE_BIN_DIR=$previousBinDir
    $env:PATH=$previousPath
    Remove-Item -LiteralPath $temp -Recurse -Force
}
