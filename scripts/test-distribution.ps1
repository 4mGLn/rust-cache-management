# Native Windows offline installer fixture. CI runs this with an already built Rust executable.
param([Parameter(Mandatory=$true)][string]$Binary)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$root=Split-Path $PSScriptRoot -Parent
$temp=Join-Path ([IO.Path]::GetTempPath()) ('rcm-test-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path $temp
$global:FixtureAssets=Join-Path $temp 'assets'
function global:curl.exe {
    $out=$null; $uri=$null; $head=$false
    for ($i=0;$i -lt $args.Count;$i++) {
        switch ([string]$args[$i]) {
            '--output' { $i++; $out=[string]$args[$i] }
            '--head' { $head=$true }
            {$_ -in @('--write-out','--proto','--proto-redir','--retry','--connect-timeout','--max-time')} { $i++ }
            {$_ -in @('--fail','--silent','--show-error','--location')} { }
            {$_ -like 'https://*'} { $uri=[string]$args[$i] }
            default { throw 'Unexpected mock download argument' }
        }
    }
    if ($uri -notlike 'https://github.com/fixture/cache/releases/*') { throw 'Fixture refused unexpected network target' }
    if ($head) { $global:LASTEXITCODE=0; return 'https://github.com/fixture/cache/releases/tag/v0.3.0' }
    $relative=$uri.Substring('https://github.com/fixture/cache/releases/download/'.Length)
    Copy-Item -LiteralPath (Join-Path $global:FixtureAssets $relative) -Destination $out
    $global:LASTEXITCODE=0
}
try {
    foreach ($v in @('v0.2.0','v0.3.0')) {
        $dest=Join-Path $global:FixtureAssets $v
        & (Join-Path $PSScriptRoot 'package.ps1') -Version $v -Binary $Binary -OutputDirectory $dest | Out-Null
        $lines=@(Get-ChildItem -LiteralPath $dest -Filter '*.sha256' | ForEach-Object { [IO.File]::ReadAllText($_.FullName).Trim() })
        [IO.File]::WriteAllText((Join-Path $dest 'SHA256SUMS'),($lines -join "`n")+"`n",(New-Object Text.UTF8Encoding($false)))
    }
    $prefix=Join-Path $temp 'install with spaces'
    & (Join-Path $root 'install.ps1') -Repository fixture/cache -Prefix $prefix -Version v0.2.0 | Out-Null
    if (Test-Path -LiteralPath (Join-Path $prefix 'auto-update')) { throw 'Automatic update unexpectedly enabled' }
    & (Join-Path $prefix 'manage.ps1') -Action Status | Out-Null
    & (Join-Path $prefix 'bin/rust-cache-inspect.cmd') --version
    if ($LASTEXITCODE) { throw 'Installed native Windows launcher failed' }
    $cache=Join-Path $temp 'cache with spaces'; $null=New-Item -ItemType Directory -Path $cache
    [IO.File]::WriteAllText((Join-Path $cache 'artifact'),'hello',(New-Object Text.UTF8Encoding($false)))
    $diagnostic=& (Join-Path $prefix 'bin/rust-cache-inspect.cmd') --path $cache
    if ($LASTEXITCODE) { throw 'Installed native Windows scan failed' }
    $json=($diagnostic -join "`n") | ConvertFrom-Json
    if ($json.paths[0].logical_file_bytes -ne 5) { throw 'Installed scan byte count mismatch' }
    if ([IO.File]::ReadAllText((Join-Path $cache 'artifact')) -cne 'hello') { throw 'Diagnostic changed fixture' }
    & (Join-Path $prefix 'manage.ps1') -Action Update -Version v0.3.0 | Out-Null
    if ((Get-Content -Raw -LiteralPath (Join-Path $prefix 'current')).Trim() -cne 'v0.3.0-x86_64-pc-windows-msvc') { throw 'Update failed to activate' }
    & (Join-Path $prefix 'manage.ps1') -Action Rollback | Out-Null
    $skill=Join-Path $prefix 'releases/v0.2.0-x86_64-pc-windows-msvc/SKILL.md'
    Add-Content -LiteralPath $skill -Value 'user edit'
    $refused=$false
    try { & (Join-Path $prefix 'manage.ps1') -Action Update -Version v0.3.0 | Out-Null } catch { $refused=$true }
    if (-not $refused -or -not ([IO.File]::ReadAllText($skill).Contains('user edit'))) { throw 'Modified payload was not preserved' }
    Write-Output 'Native Windows installer fixture passed: package, install, launcher, read-only scan, update, rollback, local modifications.'
} finally {
    Remove-Item -LiteralPath $temp -Recurse -Force
    Remove-Item Function:\curl.exe
    Remove-Variable FixtureAssets -Scope Global
}
