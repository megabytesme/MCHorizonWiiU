# Prepare a single-use entropy pool for an isolated Cemu test profile.
# Run before Cemu starts. This is not a physical Wii U entropy source.
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$PortableDirectory)

$ErrorActionPreference = 'Stop'
$root = (Get-Item -LiteralPath $PortableDirectory -Force).FullName
$drive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($root))
if ($drive.DriveFormat -ne 'NTFS') { throw 'The isolated Cemu profile must be on NTFS.' }
$components = @('sdcard', 'wiiu', 'apps', 'HorizonWiiU')
$allowed = @([Security.Principal.WindowsIdentity]::GetCurrent().User.Value, 'S-1-5-18', 'S-1-5-32-544')
$trustedOwners = $allowed + @('S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464') # TrustedInstaller
function Assert-PrivateDirectory([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force
    if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Each profile path component must be a real directory, not a reparse point.'
    }
    $acl = Get-Acl -LiteralPath $item.FullName
    if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin $trustedOwners) {
        throw 'An untrusted principal owns a profile directory.'
    }
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
    if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin $trustedOwners) {
        throw 'An untrusted principal owns a profile ancestor.'
    }
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
$pool = Join-Path $app 'cemu-entropy.pool'
$claim = Join-Path $app 'cemu-entropy.claimed'
if ((Test-Path -LiteralPath $pool) -or (Test-Path -LiteralPath $claim)) {
    throw 'A pool or claim already exists; do not reuse it.'
}
$bytes = [byte[]]::new(1048576)
$rng = [Security.Cryptography.RandomNumberGenerator]::Create()
try {
    $rng.GetBytes($bytes)
    $file = [IO.FileStream]::new($pool, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write,
        [IO.FileShare]::None, 4096, [IO.FileOptions]::WriteThrough)
    try { $file.Write($bytes, 0, $bytes.Length); $file.Flush($true) } finally { $file.Dispose() }
    $acl = Get-Acl -LiteralPath $pool
    foreach ($rule in $acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])) {
        if ($rule.AccessControlType -eq 'Allow' -and $rule.IdentityReference.Value -notin $allowed) {
            throw 'The pool did not inherit private permissions; discard the profile.'
        }
    }
    if ($acl.GetOwner([Security.Principal.SecurityIdentifier]).Value -notin $allowed) {
        throw 'The pool has an unexpected owner; discard the profile.'
    }
} finally {
    $rng.Dispose()
    [Array]::Clear($bytes, 0, $bytes.Length)
}
Write-Output 'Created a fresh private, single-use entropy pool.'
