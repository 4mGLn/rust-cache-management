param([Parameter(Mandatory=$true)][string]$Version,[Parameter(Mandatory=$true)][string]$Binary,[string]$OutputDirectory='dist')
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
if ($Version -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+$') { throw 'Version must be vN.N.N' }
$root=Split-Path $PSScriptRoot -Parent
$null=New-Item -ItemType Directory -Force -Path $OutputDirectory
$out=(Resolve-Path -LiteralPath $OutputDirectory).Path
$stage=Join-Path $out ('.package-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path (Join-Path $stage 'bin') -Force
$null=New-Item -ItemType Directory -Path (Join-Path $stage 'references') -Force
$utf8=New-Object Text.UTF8Encoding($false)
try {
    Copy-Item -LiteralPath $Binary -Destination (Join-Path $stage 'bin/rust-cache-inspect.exe')
    $names=@('AGENTS-snippet.md','CHANGELOG.md','CONTRIBUTING.md','LICENSE','README.md','SKILL.md','THIRD-PARTY-NOTICES','install.sh','install.ps1','references/defaults.md','references/distribution.md','references/inspection.md','references/policy.md')
    foreach ($name in $names) { Copy-Item -LiteralPath (Join-Path $root $name) -Destination (Join-Path $stage $name) }
    [IO.File]::WriteAllText((Join-Path $stage 'VERSION'),$Version+"`n",$utf8)
    $names=@(($names + 'VERSION' + 'bin/rust-cache-inspect.exe') | Sort-Object)
    $lines=@(foreach ($name in $names) { (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $stage $name)).Hash.ToLowerInvariant()+'  '+$name })
    [IO.File]::WriteAllText((Join-Path $stage 'MANIFEST.sha256'),($lines -join "`n")+"`n",$utf8)
    $names += 'MANIFEST.sha256'
    $asset="rust-cache-management-$Version-x86_64-pc-windows-msvc.tar.gz"
    foreach ($name in @($asset,($asset+'.sha256'))) {
        if ($null -ne (Get-Item -Force -ErrorAction SilentlyContinue -LiteralPath (Join-Path $out $name))) { throw 'Refusing to overwrite bundle or checksum, including reparse paths.' }
    }
    & tar.exe -czf (Join-Path $out $asset) -C $stage @names
    if ($LASTEXITCODE -ne 0) { throw 'tar failed' }
    $hash=(Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $out $asset)).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $out ($asset+'.sha256')),$hash+'  '+$asset+"`n",$utf8)
    Write-Output (Join-Path $out $asset)
} finally { Remove-Item -Recurse -Force -LiteralPath $stage }
