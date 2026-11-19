# Prepare the original control endpoint for an isolated Cemu development profile.
# Run before Cemu starts. This is not a physical Wii U pairing mechanism.
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$PortableDirectory)

$ErrorActionPreference = 'Stop'
$root = (Get-Item -LiteralPath $PortableDirectory -Force).FullName
$drive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($root))
if ($drive.DriveFormat -ne 'NTFS') { throw 'The isolated Cemu profile must be on NTFS.' }
$components = @('sdcard', 'wiiu', 'apps', 'HorizonWiiU')
$allowed = @([Security.Principal.WindowsIdentity]::GetCurrent().User.Value, 'S-1-5-18', 'S-1-5-32-544')
function Assert-PrivateDirectory([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Each profile path component must be a real directory, not a reparse point.'
    }
    $acl = Get-Acl -LiteralPath $item.FullName
    foreach ($rule in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
        if ($rule.AccessControlType -eq 'Allow' -and $rule.IdentityReference.Value -notin $allowed) {
            throw 'The profile and its parent must allow access only to the current user, SYSTEM, and Administrators.'
        }
    }
}
# Ancestors can be readable, but another account must not be able to remove
# or take ownership of an existing path component and replace the profile.
$ancestor = Split-Path -Parent (Split-Path -Parent $root)
$replaceRights = [Security.AccessControl.FileSystemRights]::Delete -bor
    [Security.AccessControl.FileSystemRights]::DeleteSubdirectoriesAndFiles -bor
    [Security.AccessControl.FileSystemRights]::ChangePermissions -bor
    [Security.AccessControl.FileSystemRights]::TakeOwnership
while ($ancestor) {
    $item = Get-Item -LiteralPath $ancestor -Force
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw 'Profile ancestors must not contain reparse points.'
    }
    $acl = Get-Acl -LiteralPath $ancestor
    foreach ($rule in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
        if ($rule.AccessControlType -eq 'Allow' -and $rule.IdentityReference.Value -notin $allowed -and
            -not ($rule.PropagationFlags -band [Security.AccessControl.PropagationFlags]::InheritOnly) -and
            ($rule.FileSystemRights -band $replaceRights)) {
            throw 'An untrusted principal can replace a profile ancestor.'
        }
    }
    $next = Split-Path -Parent $ancestor
    if ($next -eq $ancestor) { break }
    $ancestor = $next
}
Assert-PrivateDirectory (Split-Path -Parent $root)
Assert-PrivateDirectory $root
$app = $root
foreach ($part in $components) {
    $app = Join-Path $app $part
    Assert-PrivateDirectory $app
}
$path = Join-Path $app 'control-cemu.json'
if (Test-Path -LiteralPath $path) { throw 'Endpoint already exists; stop Cemu and use a fresh endpoint.' }
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
try { $port = ([Net.IPEndPoint]$listener.LocalEndpoint).Port } finally { $listener.Stop() }
$random = [byte[]]::new(16)
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
try { $rng.GetBytes($random) } finally { $rng.Dispose() }
$token = ([BitConverter]::ToString($random)).Replace('-', '').ToLowerInvariant()
$content = '{"transport":"tcp","port":' + $port + ',"token":"' + $token + '"}'
$bytes = [Text.UTF8Encoding]::new($false).GetBytes($content)
$created = $false
try {
    $file = [IO.FileStream]::new($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write,
        [IO.FileShare]::None, 4096, [IO.FileOptions]::WriteThrough)
    $created = $true
    try { $file.Write($bytes, 0, $bytes.Length); $file.Flush($true) } finally { $file.Dispose() }
    $acl = Get-Acl -LiteralPath $path
    foreach ($rule in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
        if ($rule.AccessControlType -eq 'Allow' -and $rule.IdentityReference.Value -notin $allowed) {
            throw 'The endpoint did not inherit private permissions.'
        }
    }
    $owner = $acl.GetOwner([Security.Principal.SecurityIdentifier]).Value
    if ($owner -notin $allowed) { throw 'The endpoint has an unexpected owner.' }
} catch {
    if ($created) { [IO.File]::Delete($path) }
    throw
} finally {
    $token = $null
    $content = $null
    [Array]::Clear($random, 0, $random.Length)
    [Array]::Clear($bytes, 0, $bytes.Length)
}
Write-Output "Cemu control endpoint created: $path"
Write-Output "Loopback port: $port (not reserved; verify the Cemu listener PID before connecting)"
