# Drives a fresh portable Cemu profile through its keyboard-to-VPAD mapping.
param(
    [Parameter(Mandatory)][string]$Cemu,
    [Parameter(Mandatory)][string]$Rpx,
    [Parameter(Mandatory)][string]$Output,
    [switch]$Production
)
$ErrorActionPreference = 'Stop'
$Cemu = (Resolve-Path -LiteralPath $Cemu).Path
$Rpx = (Resolve-Path -LiteralPath $Rpx).Path
$Output = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Output)
if (Test-Path -LiteralPath $Output) { throw 'Cemu output must be a new directory' }

$portable = Join-Path $Output 'portable'
New-Item -ItemType Directory "$portable/controllerProfiles", "$portable/mlc01", "$portable/sdcard" | Out-Null
Copy-Item -LiteralPath $Cemu -Destination "$Output/Cemu.exe"
foreach ($item in 'resources','gameProfiles') {
    $source = Join-Path (Split-Path $Cemu) $item
    if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination $Output -Recurse }
}
$Rpx = (Copy-Item -LiteralPath $Rpx -Destination "$Output/horizon_wiiu_pregame.rpx" -PassThru).FullName

@'
<content>
    <logflag>131072</logflag>
    <check_update>false</check_update>
    <gp_download>false</gp_download>
    <use_discord_presence>false</use_discord_presence>
    <save_screenshot>true</save_screenshot>
    <open_pad>true</open_pad>
    <Graphic><api>1</api><AsyncCompile>false</AsyncCompile></Graphic>
</content>
'@ | Set-Content -LiteralPath "$portable/settings.xml" -Encoding ascii
@'
<emulated_controller>
  <type>Wii U GamePad</type>
  <controller>
    <api>Keyboard</api>
    <uuid>Keyboard</uuid>
    <display_name>Keyboard</display_name>
    <mappings>
      <entry><mapping>1</mapping><button>90</button></entry>
      <entry><mapping>2</mapping><button>88</button></entry>
      <entry><mapping>11</mapping><button>87</button></entry>
      <entry><mapping>12</mapping><button>83</button></entry>
    </mappings>
  </controller>
</emulated_controller>
'@ | Set-Content -LiteralPath "$portable/controllerProfiles/controller0.xml" -Encoding ascii

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class PregameCemuInput {
    public delegate bool WindowCallback(IntPtr window, IntPtr context);
    [DllImport("user32.dll")]
    public static extern bool EnumChildWindows(IntPtr window, WindowCallback callback, IntPtr context);
    [DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr window, uint message, IntPtr key, IntPtr flags);
    [DllImport("user32.dll")]
    public static extern uint MapVirtualKey(uint key, uint type);

    public static IntPtr[] Windows(IntPtr parent) {
        var windows = new List<IntPtr> { parent };
        EnumChildWindows(parent, (child, context) => { windows.Add(child); return true; }, IntPtr.Zero);
        return windows.ToArray();
    }
}
'@

function Send-Key([int]$Key) {
    $program.Refresh()
    if ($program.HasExited -or $program.MainWindowHandle -eq [IntPtr]::Zero) {
        throw 'Cemu has no application window'
    }
    $windows = [PregameCemuInput]::Windows($program.MainWindowHandle)
    $scan = [long]([PregameCemuInput]::MapVirtualKey($Key, 0)) -shl 16
    foreach ($window in $windows) {
        [void][PregameCemuInput]::PostMessage($window, 0x100, [IntPtr]$Key, [IntPtr]($scan -bor 1))
    }
    Start-Sleep -Milliseconds 160
    foreach ($window in $windows) {
        [void][PregameCemuInput]::PostMessage($window, 0x101, [IntPtr]$Key, [IntPtr]($scan -bor [long]3221225473))
    }
    Start-Sleep -Milliseconds 150
}

function Wait-Step([string]$Text, [int]$Count = 1) {
    $deadline = [DateTime]::UtcNow.AddSeconds(18)
    do {
        if (Test-Path -LiteralPath $log) {
            $observations = @(Select-String -LiteralPath $log -SimpleMatch $Text | Where-Object { $_.Line.EndsWith($Text) })
            if ($observations.Count -ge $Count) { return }
        }
        if ($program.HasExited) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Cemu did not reach '$Text' ($Count). See $log"
}

function Test-CompletePng([string]$Path) {
    # Cemu creates both filenames before it finishes encoding their contents.
    $stream = $null
    try {
        $stream = [IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite')
        $trailer = New-Object byte[] 12
        if ($stream.Length -lt $trailer.Length) { return $false }
        [void]$stream.Seek(-$trailer.Length, [IO.SeekOrigin]::End)
        if ($stream.Read($trailer, 0, $trailer.Length) -ne $trailer.Length) { return $false }
        return [BitConverter]::ToString($trailer) -eq '00-00-00-00-49-45-4E-44-AE-42-60-82'
    } catch [IO.IOException] {
        return $false
    } finally {
        if ($stream) { $stream.Dispose() }
    }
}

function Save-Frame([string]$State) {
    $screenshotDir = Join-Path $portable 'screenshots'
    $existing = @(Get-ChildItem -LiteralPath $screenshotDir -Filter '*.png' -ErrorAction SilentlyContinue |
        ForEach-Object Name)
    Send-Key 123 # F12
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $screens = @(Get-ChildItem -LiteralPath $screenshotDir -Filter '*.png' -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -notin $existing -and (Test-CompletePng $_.FullName) })
        if ($screens.Count -eq 2) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($screens.Count -ne 2) { throw "Missing TV or GamePad screenshot for $State" }
    foreach ($screen in $screens) {
        $target = if ($screen.BaseName.EndsWith('_GamePad')) { 'gamepad' } else { 'tv' }
        Copy-Item -LiteralPath $screen.FullName -Destination "$Output/$State-$target.png"
    }
    Start-Sleep -Milliseconds 1000
}

$prior = @{}
foreach ($name in 'HORIZON_HOME','HORIZON_LAUNCHER_HOME','XDG_DATA_HOME','XDG_CONFIG_HOME',
    'HORIZON_LAUNCHER_TRASH','HORIZON_LAUNCHER_CLIPBOARD','HORIZON_LAUNCHER_DESKTOP',
    'HORIZON_NO_AUDIO','HORIZON_LAUNCHER_NO_SELF_UPDATE') {
    $prior[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
    $value = '1'
    if ($name -notin 'HORIZON_NO_AUDIO','HORIZON_LAUNCHER_NO_SELF_UPDATE') {
        $value = (New-Item -ItemType Directory "$Output/scratch/$name").FullName
    }
    [Environment]::SetEnvironmentVariable($name, $value, 'Process')
}
$log = Join-Path $portable 'log.txt'
$program = $null
try {
    $arguments = @('-g', ('"{0}"' -f $Rpx))
    $program = Start-Process -FilePath "$Output/Cemu.exe" -WorkingDirectory $Output -WindowStyle Hidden -ArgumentList $arguments -PassThru

    if ($Production) {
        Wait-Step 'frame.home.profiles'
        Save-Frame 'home'
    } else {
        Wait-Step 'frame.home.play'
        Save-Frame 'home'
        Send-Key 83 # D-pad Down
        Wait-Step 'frame.home.profiles'
    }
    Send-Key 90 # A
    Wait-Step 'frame.profiles'
    Send-Key 83
    Send-Key 90
    Wait-Step 'frame.profiles.guest_selected'
    Save-Frame 'profiles'
    Send-Key 88 # B
    Wait-Step 'frame.home.profiles' 2

    Send-Key 83
    Wait-Step 'frame.home.versions'
    Send-Key 90
    Wait-Step 'frame.versions'
    Save-Frame 'versions'
    Send-Key 88
    Wait-Step 'frame.home.versions' 2

    Send-Key 83
    Wait-Step 'frame.home.settings'
    Send-Key 90
    Wait-Step 'frame.settings'
    Save-Frame 'settings'
    Send-Key 90
    Wait-Step 'frame.settings.swapped'
    Send-Key 88 # B now selects, so restore normal A/B.
    Wait-Step 'frame.settings' 2
    Send-Key 88
    Wait-Step 'frame.home.settings' 2

    Send-Key 83
    Wait-Step 'frame.home.agent'
    Send-Key 90
    Wait-Step 'frame.agent'
    Save-Frame 'agent'
    Send-Key 88
    Wait-Step 'frame.home.agent' 2
    if ($Production) {
        Send-Key 83 # D-pad Down from Agent Control.
    } else {
        foreach ($step in 1..4) { Send-Key 87 } # D-pad Up
        Wait-Step 'frame.home.play' 2

        Send-Key 90
        Wait-Step 'frame.loading'
        Save-Frame 'loading'
        Wait-Step 'frame.session'
        Save-Frame 'session'
        Send-Key 88
        Wait-Step 'frame.home.play' 3

        Send-Key 90
        Wait-Step 'frame.loading' 2
        Send-Key 88
        Wait-Step 'frame.home.play' 4

        foreach ($step in 1..3) { Send-Key 83 }
        Wait-Step 'frame.home.settings' 3
        Send-Key 90
        Wait-Step 'frame.settings' 3
        Send-Key 83
        Send-Key 83
        Send-Key 90
        Wait-Step 'frame.startup_error'
        Save-Frame 'error'
        Send-Key 88
        Wait-Step 'frame.home.play' 5

        Send-Key 90
        Wait-Step 'frame.session' 2
        Send-Key 88
        Wait-Step 'frame.home.play' 6

        Send-Key 87 # Up wraps to Close Horizon.
    }
    Wait-Step 'frame.home.close'
    Send-Key 90
    Wait-Step 'procui.closed'
    Wait-Step 'coreinit.exit(0)'

    $actual = @(Select-String -LiteralPath $log -SimpleMatch 'Horizon pregame: ' |
        ForEach-Object { ($_.Line -split 'Horizon pregame: ', 2)[1] })
    $lifecycle = @($actual | Where-Object {
        $_ -match '^(session\.|frame\.(loading|session|startup_error)(\.|$)|launcher\.closed$|view\.released$|gx2\.closed$|procui\.closed$)'
    })
    $saveLifecycle = @(
        'frame.loading', 'session.allocated', 'session.ready', 'frame.session',
        'session.save_requested', 'frame.session.saving', 'session.saved',
        'session.stop_requested', 'frame.session.stopping', 'session.quiescent', 'session.released'
    )
    $abortLifecycle = @(
        'frame.loading', 'session.allocated', 'session.stop_requested',
        'frame.loading.stopping', 'session.quiescent', 'session.released'
    )
    $expected = @('launcher.closed', 'view.released', 'gx2.closed', 'procui.closed')
    if (!$Production) {
        $expected = $saveLifecycle + $abortLifecycle + $abortLifecycle + @('frame.startup_error') + $saveLifecycle + $expected
    }
    if (($lifecycle -join ',') -cne ($expected -join ',')) {
        throw "Unexpected lifecycle markers. See $log"
    }
    $captures = Get-ChildItem -LiteralPath $Output -Filter '*.png' | Sort-Object Name
    $evidence = [ordered]@{
        schema = 1
        kind = 'wiiu-pregame-cemu-acceptance'
        result = 'passed'
        fixture = !$Production.IsPresent
        rpx_sha256 = (Get-FileHash -LiteralPath $Rpx).Hash.ToLowerInvariant()
        emulator_sha256 = (Get-FileHash -LiteralPath $Cemu).Hash.ToLowerInvariant()
        emulator_settings = @(Select-String -LiteralPath $log -Pattern 'CPU-Mode:|Async compile:|Using Vulkan' |
            ForEach-Object { ($_.Line -split '\] ', 2)[1] })
        guest_exit = 0
        markers = $actual
        metrics = @(Select-String -LiteralPath $log -SimpleMatch "Horizon pregame metrics" | ForEach-Object { ($_.Line -split "Horizon pregame metrics: ", 2)[1] })
        captures = @($captures | ForEach-Object {
            @{ file = $_.Name; sha256 = (Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant() }
        })
    }
    $evidence | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath "$Output/cemu-acceptance.json" -Encoding ascii
    if ($Production) {
        Write-Output 'Cemu production navigation and guest exit passed.'
    } else {
        Write-Output 'Cemu VPAD navigation, save and drain, cancellation, failed startup, repeat launch, and guest exit passed.'
    }
} finally {
    if ($program -and !$program.HasExited) { $program.Kill(); $program.WaitForExit() }
    foreach ($name in $prior.Keys) {
        [Environment]::SetEnvironmentVariable($name, $prior[$name], 'Process')
    }
}
