# User-local versioned installation; no services, tasks, or ExecutionPolicy changes.
param(
    [ValidateSet('Install','Update','Rollback','AutoOn','AutoOff','Status','Run')]
    [string]$Action = 'Install',
    [string]$Repository = '4mGLn/rust-cache-management',
    [string]$Prefix,
    [string]$Version,
    [switch]$AutoUpdate,
    [string[]]$DiagnosticArguments = @()
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Assert-PlainPath([string]$Path) {
    if ($Path -notmatch '^[A-Za-z]:[\\/]' -or $Path -match '(^|[\\/])\.\.?([\\/]|$)') { throw 'Use an absolute prefix without dot components.' }
    $p = [IO.Path]::GetFullPath($Path)
    while ($p) {
        try {
            $attributes=[IO.File]::GetAttributes($p)
            if ($attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Reparse path refused: $p" }
        } catch [IO.FileNotFoundException] { } catch [IO.DirectoryNotFoundException] { }
        $parent = [IO.Directory]::GetParent($p)
        if ($null -eq $parent) { break }
        $p = $parent.FullName
    }
}
function Hash([string]$Path) { (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
function Write-Text([string]$Path,[string]$Text) { [IO.File]::WriteAllText($Path,$Text + "`n",(New-Object Text.UTF8Encoding($false))) }
function Read-Plain([string]$Path) {
    Assert-PlainPath $Path
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "Missing file: $Path" }
    [IO.File]::ReadAllText($Path).Trim()
}
function Payload-Names {
    @('AGENTS-snippet.md','CHANGELOG.md','CONTRIBUTING.md','LICENSE','README.md','SKILL.md','THIRD-PARTY-NOTICES','VERSION',"bin/$script:Binary",'install.ps1','install.sh','references/defaults.md','references/distribution.md','references/inspection.md','references/policy.md') | Sort-Object
}
function Read-Pointer([string]$Name) {
    $value = Read-Plain (Join-Path $Prefix $Name)
    if ($value -notmatch ('^v[0-9]+\.[0-9]+\.[0-9]+-' + [regex]::Escape($Target) + '$')) { throw "Invalid $Name pointer" }
    $value
}
function Verify-Payload([string]$dir,[string]$Baseline) {
    Assert-PlainPath $dir
    $manifest = Join-Path $dir 'MANIFEST.sha256'
    Assert-PlainPath $manifest
    if ((Hash $manifest) -ne $Baseline) { throw 'Installed manifest changed; update refused.' }
    $lines = [IO.File]::ReadAllLines($manifest)
    $names = @()
    foreach ($line in $lines) {
        if ($line -notmatch '^([0-9a-f]{64})  ([A-Za-z0-9._/-]+)$') { throw 'Invalid manifest line' }
        $names += $Matches[2]
        $file = Join-Path $dir $Matches[2]
        Assert-PlainPath $file
        if ((Hash $file) -ne $Matches[1]) { throw "Modified payload preserved; update refused: $file" }
    }
    if ((($names | Sort-Object) -join "`n") -cne ((Payload-Names) -join "`n")) { throw 'Manifest differs from fixed payload allowlist.' }
}
function Verify-Release([string]$Release) {
    Verify-Payload (Join-Path $Prefix "releases/$Release") (Read-Plain (Join-Path $Prefix "records/$Release"))
}
function Verify-Manager {
    foreach ($name in @('manage.ps1','bin/rust-cache-inspect.ps1','bin/rust-cache-inspect.cmd')) {
        $path = Join-Path $Prefix $name
        Assert-PlainPath $path
        if ((Hash $path) -ne (Read-Plain ($path+'.sha256'))) { throw "Modified manager preserved; update refused: $name" }
    }
}
function Activate([string]$Release) {
    if (Test-Path -LiteralPath (Join-Path $Prefix 'current')) {
        Write-Text (Join-Path $Stage 'previous') (Read-Pointer 'current')
        Move-Item -Force -LiteralPath (Join-Path $Stage 'previous') -Destination (Join-Path $Prefix 'previous')
    }
    Write-Text (Join-Path $Stage 'current') $Release
    Move-Item -Force -LiteralPath (Join-Path $Stage 'current') -Destination (Join-Path $Prefix 'current')
}
function Download([string]$Uri,[string]$OutFile) {
    & curl.exe --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --retry 2 --connect-timeout 15 --max-time 180 --output $OutFile $Uri
    if ($LASTEXITCODE -ne 0) { throw 'HTTPS release download failed.' }
}
$Target = 'x86_64-pc-windows-msvc'
$script:Binary = 'rust-cache-inspect.exe'
if ($env:OS -ne 'Windows_NT' -or $env:PROCESSOR_ARCHITECTURE -notin @('AMD64','x86') -or -not [Environment]::Is64BitOperatingSystem) { throw 'This installer supports Windows x86_64 only.' }
if (-not $Prefix) {
    if ([IO.Path]::GetFileName($PSCommandPath) -eq 'manage.ps1') { $Prefix=$PSScriptRoot }
    else { $Prefix=Join-Path $env:LOCALAPPDATA 'rust-cache-management' }
}
Assert-PlainPath $Prefix
$Prefix=[IO.Path]::GetFullPath($Prefix).TrimEnd('\','/')
if ($Prefix -match '^[A-Za-z]:$') { throw 'Use a dedicated installation directory, not a volume root.' }
if ($Version -and $Version -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+$') { throw 'Version must be vN.N.N' }
if ($Action -eq 'Install') {
    if ($Repository -notmatch '^[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*$') { throw 'Invalid repository; expected OWNER/REPO.' }
    if (Test-Path -LiteralPath $Prefix) { throw 'Prefix exists; use its manager or choose a new prefix.' }
    $null = New-Item -ItemType Directory -Path $Prefix
} else {
    $Repository=Read-Plain (Join-Path $Prefix 'repository')
    if ($Repository -notmatch '^[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*$') { throw 'Invalid repository record' }
}
foreach ($name in @('releases','records','bin','current','previous','repository','auto-update','last-check','manage.ps1')) { Assert-PlainPath (Join-Path $Prefix $name) }
if ($Action -eq 'Run') {
    if (Test-Path -LiteralPath (Join-Path $Prefix 'auto-update')) {
        try {
            [long]$last=0
            if (Test-Path -LiteralPath (Join-Path $Prefix 'last-check')) { [void][long]::TryParse((Read-Plain (Join-Path $Prefix 'last-check')),[ref]$last) }
            $now=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
            if ($last -gt $now -or $now-$last -ge 86400) {
                $state=Join-Path $Prefix ('.check-'+[guid]::NewGuid().ToString('N'))
                Write-Text $state ([string]$now)
                Move-Item -Force -LiteralPath $state -Destination (Join-Path $Prefix 'last-check')
                & $PSCommandPath -Action Update -Prefix $Prefix
                if ($LASTEXITCODE -ne 0) { Write-Warning 'Update failed; retained diagnostic release will run.' }
            }
        } catch { Write-Warning ('Update check failed; retained diagnostic release will run. '+$_.Exception.Message) }
    }
    $current=Read-Pointer 'current'
    $exe=Join-Path $Prefix "releases/$current/bin/$script:Binary"
    Assert-PlainPath $exe
    & $exe @DiagnosticArguments
    exit $LASTEXITCODE
}
$lock=Join-Path $Prefix '.lock'
Assert-PlainPath $lock
if (Test-Path -LiteralPath $lock) { throw 'Another manager is running or a stale .lock needs inspection.' }
$null=New-Item -ItemType Directory -Path $lock -ErrorAction Stop
$Stage=Join-Path $Prefix ('.stage-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path $Stage
try {
    if ($Action -ne 'Install') { Verify-Manager; $current=Read-Pointer 'current'; Verify-Release $current }
    switch ($Action) {
        'Status' { Write-Output "Repository: $Repository`nCurrent release: $current`nSkill: $Prefix/releases/$current/SKILL.md"; Write-Output ('Auto update: '+(Test-Path -LiteralPath (Join-Path $Prefix 'auto-update'))); return }
        'AutoOn' { Write-Text (Join-Path $Stage 'auto-update') 'enabled'; Move-Item -Force -LiteralPath (Join-Path $Stage 'auto-update') -Destination (Join-Path $Prefix 'auto-update'); Write-Output 'Enabled: launcher checks at most daily; no service or scheduled task.'; return }
        'AutoOff' { Remove-Item -Force -ErrorAction SilentlyContinue -LiteralPath (Join-Path $Prefix 'auto-update'); Write-Output 'Automatic updates disabled.'; return }
        'Rollback' { $previous=Read-Pointer 'previous'; Verify-Release $previous; Activate $previous; Write-Output "Activated retained release $previous"; return }
    }
    if (-not (Get-Command curl.exe -ErrorAction SilentlyContinue)) { throw 'Windows built-in curl.exe is required.' }
    if (-not (Get-Command tar.exe -ErrorAction SilentlyContinue)) { throw 'Windows built-in tar.exe is required.' }
    if (-not $Version) {
        $uri=& curl.exe --fail --silent --show-error --head --location --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 60 --output NUL --write-out '%{url_effective}' "https://github.com/$Repository/releases/latest"
        if ($LASTEXITCODE -ne 0) { throw 'Latest stable release lookup failed.' }
        $Version=([string]$uri).TrimEnd('/').Split('/')[-1]
        if ($Version -notmatch '^v[0-9]+\.[0-9]+\.[0-9]+$') { throw 'Latest stable release could not be resolved.' }
    }
    $release="$Version-$Target"
    if ($Action -eq 'Update') {
        $oldVersion=$current.Substring(1,$current.Length-$Target.Length-2)
        if ([version]$Version.Substring(1) -eq [version]$oldVersion) { Write-Output "Already at $Version"; return }
        if ([version]$Version.Substring(1) -lt [version]$oldVersion) { throw 'Update must advance version; use rollback for a retained release.' }
    }
    $releaseDir=Join-Path $Prefix "releases/$release"
    $record=Join-Path $Prefix "records/$release"
    if ((Test-Path -LiteralPath $releaseDir) -or (Test-Path -LiteralPath $record)) {
        Verify-Release $release
        Activate $release
        Write-Output "Activated verified retained release $release"
        return
    }
    $asset="rust-cache-management-$Version-$Target.tar.gz"
    $base="https://github.com/$Repository/releases/download/$Version"
    Download "$base/SHA256SUMS" (Join-Path $Stage 'SHA256SUMS')
    Download "$base/$asset" (Join-Path $Stage 'bundle.tar.gz')
    $checks=@([IO.File]::ReadAllLines((Join-Path $Stage 'SHA256SUMS')) | Where-Object { $_ -cmatch ('^[0-9a-f]{64}  '+[regex]::Escape($asset)+'$') })
    if ($checks.Count -ne 1 -or (Hash (Join-Path $Stage 'bundle.tar.gz')) -ne $checks[0].Substring(0,64)) { throw 'Missing, ambiguous, or mismatched release checksum.' }
    $members=@(& tar.exe -tzf (Join-Path $Stage 'bundle.tar.gz'))
    if ($LASTEXITCODE -ne 0) { throw 'Archive listing failed.' }
    $expected=@((Payload-Names) + 'MANIFEST.sha256' | Sort-Object)
    if ((($members | Sort-Object) -join "`n") -cne ($expected -join "`n")) { throw 'Archive differs from fixed payload allowlist.' }
    $payload=Join-Path $Stage 'payload'
    $null=New-Item -ItemType Directory -Path (Join-Path $payload 'bin') -Force
    $null=New-Item -ItemType Directory -Path (Join-Path $payload 'references') -Force
    # Redirect binary stdout with native file handles; PowerShell 5 redirection corrupts binary bytes.
    foreach ($member in $expected) {
        $psi=New-Object Diagnostics.ProcessStartInfo
        $psi.FileName=(Get-Command tar.exe).Source
        $psi.Arguments='-xOzf "'+(Join-Path $Stage 'bundle.tar.gz')+'" "'+$member+'"'
        $psi.UseShellExecute=$false; $psi.RedirectStandardOutput=$true
        $proc=New-Object Diagnostics.Process; $proc.StartInfo=$psi
        $null=$proc.Start()
        $stream=[IO.File]::Create((Join-Path $payload $member))
        try { $proc.StandardOutput.BaseStream.CopyTo($stream) } finally { $stream.Dispose() }
        $proc.WaitForExit()
        if ($proc.ExitCode -ne 0) { throw "Archive member read failed: $member" }
        $proc.Dispose()
    }
    if ((Read-Plain (Join-Path $payload 'VERSION')) -cne $Version) { throw 'Payload version mismatch' }
    foreach ($dir in @('releases','records','bin')) { $null=New-Item -ItemType Directory -Path (Join-Path $Prefix $dir) -Force }
    $manifestHash=Hash (Join-Path $payload 'MANIFEST.sha256')
    Verify-Payload $payload $manifestHash
    Write-Text (Join-Path $Stage 'record') $manifestHash
    Move-Item -LiteralPath (Join-Path $Stage 'record') -Destination $record
    Move-Item -LiteralPath $payload -Destination $releaseDir
    Verify-Release $release
    if ($Action -eq 'Install') {
        Copy-Item -LiteralPath (Join-Path $releaseDir 'install.ps1') -Destination (Join-Path $Prefix 'manage.ps1')
        $launcher=@'
$ErrorActionPreference='Stop'
& "$PSScriptRoot/../manage.ps1" -Action Run -DiagnosticArguments $args
exit $LASTEXITCODE
'@
        Write-Text (Join-Path $Prefix 'bin/rust-cache-inspect.ps1') $launcher
        Write-Text (Join-Path $Prefix 'bin/rust-cache-inspect.cmd') '@powershell.exe -NoLogo -NoProfile -File "%~dp0rust-cache-inspect.ps1" %*'
        foreach ($name in @('manage.ps1','bin/rust-cache-inspect.ps1','bin/rust-cache-inspect.cmd')) { Write-Text (Join-Path $Prefix ($name+'.sha256')) (Hash (Join-Path $Prefix $name)) }
        Write-Text (Join-Path $Prefix 'repository') $Repository
    }
    Activate $release
    if ($AutoUpdate) { Write-Text (Join-Path $Prefix 'auto-update') 'enabled' }
    Write-Output "Installed $release`nLauncher: $Prefix/bin/rust-cache-inspect.cmd`nSkill: $releaseDir/SKILL.md"
} finally {
    if (Test-Path -LiteralPath $Stage) { Remove-Item -LiteralPath $Stage -Recurse -Force }
    if (Test-Path -LiteralPath $lock) { Remove-Item -LiteralPath $lock -Force }
    if ($Action -eq 'Install' -and -not (Test-Path -LiteralPath (Join-Path $Prefix 'manage.ps1'))) {
        foreach ($path in @((Join-Path $Prefix 'bin'),(Join-Path $Prefix 'records'),(Join-Path $Prefix 'releases'),$Prefix)) {
            if ((Test-Path -LiteralPath $path -PathType Container) -and @(Get-ChildItem -Force -LiteralPath $path).Count -eq 0) { Remove-Item -LiteralPath $path -Force }
        }
    }
}
