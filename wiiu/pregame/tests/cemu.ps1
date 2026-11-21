# Drives a fresh portable Cemu profile through its keyboard-to-VPAD mapping.
param(
    [Parameter(Mandatory)][string]$Cemu,
    [Parameter(Mandatory)][string]$Rpx,
    [Parameter(Mandatory)][string]$Output,
    [switch]$Production,
    [switch]$NativeMcp,
    [switch]$CodexInterop,
    [string]$EndpointAddress,
    [string]$PreparedResources,
    [string]$IsolatedSaves,
    [string]$IsolatedOptions,
    [switch]$Recompiler,
    [switch]$RestartMcp,
    [switch]$CaptureProfile,
    [string]$AgentHostname,
    [string]$GeminiBundle
)
$ErrorActionPreference = 'Stop'
$Cemu = (Resolve-Path -LiteralPath $Cemu).Path
$Rpx = (Resolve-Path -LiteralPath $Rpx).Path
$Output = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Output)
if (Test-Path -LiteralPath $Output) { throw 'Cemu output must be a new directory' }

$portable = Join-Path $Output 'portable'
New-Item -ItemType Directory "$portable/controllerProfiles", "$portable/mlc01", "$portable/sdcard" | Out-Null
Copy-Item -LiteralPath $Cemu -Destination "$Output/Cemu.exe"
if ($PreparedResources) {
    $cache = Join-Path $portable 'sdcard/wiiu/apps/HorizonWiiU/.local/horizon-client'
    New-Item -ItemType Directory -Path $cache -Force | Out-Null
    # The preparation is read-only. Keep saves and any mutable app data inside this profile.
    New-Item -ItemType Junction -Path "$cache/26.3" -Target (Resolve-Path -LiteralPath $PreparedResources).Path | Out-Null
}
if ($IsolatedSaves) {
    $app = Join-Path $portable 'sdcard/wiiu/apps/HorizonWiiU'
    New-Item -ItemType Directory -Path $app -Force | Out-Null
    Copy-Item -LiteralPath $IsolatedSaves -Destination "$app/saves" -Recurse
}
if ($IsolatedOptions) {
    $app = Join-Path $portable 'sdcard/wiiu/apps/HorizonWiiU'
    New-Item -ItemType Directory -Path $app -Force | Out-Null
    Copy-Item -LiteralPath $IsolatedOptions -Destination "$app/options.txt"
}
if ($CaptureProfile -or $AgentHostname) {
    $app = Join-Path $portable 'sdcard/wiiu/apps/HorizonWiiU'
    New-Item -ItemType Directory "$app/launcher" -Force | Out-Null
    if ($CaptureProfile) {
        New-Item -ItemType File "$app/gx2-profile.enable" | Out-Null
    }
    if ($AgentHostname) {
        Set-Content -LiteralPath "$app/launcher/agent-control-hostname.txt" -Value $AgentHostname -Encoding ascii
    }
}
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

function Wait-Step([string]$Text, [int]$Count = 1, [int]$Seconds = 18) {
    $deadline = [DateTime]::UtcNow.AddSeconds($Seconds)
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
    if (!$Recompiler) { $arguments = @('--force-interpreter') + $arguments }
    $program = Start-Process -FilePath "$Output/Cemu.exe" -WorkingDirectory $Output -WindowStyle Hidden -ArgumentList $arguments -PassThru

    if ($NativeMcp) {
        if ($PreparedResources) {
            Wait-Step 'frame.home.play'
            Send-Key 83
        } else {
            Wait-Step 'frame.home.profiles'
        }
        foreach ($step in 1..4) { Send-Key 83 }
        Wait-Step 'frame.home.agent'
        Send-Key 90
        Wait-Step 'frame.local.form'
        Save-Frame 'agent-control'
        Send-Key 90
        Send-Key 83
        Send-Key 90
        Save-Frame 'no-pin-consent'
        Send-Key 90
        Save-Frame 'agent-control-initialized'

        $ca = Join-Path $portable 'sdcard/wiiu/apps/HorizonWiiU/launcher/agent-control-ca.pem'
        $listener = @(Get-NetTCPConnection -State Listen -LocalPort 8443 -OwningProcess $program.Id -ErrorAction SilentlyContinue)
        $fingerprint = if (Test-Path -LiteralPath $ca) {
            $der = [Convert]::FromBase64String(((Get-Content -LiteralPath $ca | Where-Object { $_ -notlike '-----*' }) -join ''))
            [Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($der))
        }
        if (!$EndpointAddress -and $listener.Count -eq 1) {
            $EndpointAddress = $listener[0].LocalAddress
        }
        $http = $null
        if ($EndpointAddress -and $fingerprint) {
            $url = 'https://{0}:8443/.well-known/oauth-protected-resource/mcp' -f $EndpointAddress
            $meta = & python -B -c 'import ssl,sys,urllib.request; print(urllib.request.urlopen(sys.argv[1],context=ssl.create_default_context(cafile=sys.argv[2]),timeout=8).status)' $url $ca
            if ($LASTEXITCODE -eq 0) { $http = $meta }
            if ($http -eq '200' -and $CodexInterop) {
                $origin = 'https://{0}:8443' -f $EndpointAddress
                $cliFixture = Join-Path $PSScriptRoot '../../../tools/mcp_codex_cemu_test.py'
                $state = Join-Path $Output 'scratch/codex'
                [string[]]$extra = if ($PreparedResources) { '--game' }
                python -B $cliFixture $origin $ca ($program.MainWindowHandle.ToInt64()) $state @extra
                if ($LASTEXITCODE -ne 0) { throw 'Real Codex MCP interoperability failed' }
            }
            if ($http -eq '200' -and $GeminiBundle) {
                $origin = 'https://{0}:8443' -f $EndpointAddress
                $cliFixture = Join-Path $PSScriptRoot '../../../tools/mcp_gemini_cemu_test.py'
                $state = Join-Path $Output 'scratch/gemini'
                [string[]]$extra = if ($PreparedResources) { '--game' } elseif ($RestartMcp) { '--game-grant' }
                python -B $cliFixture $GeminiBundle $origin $ca ($program.MainWindowHandle.ToInt64()) $state @extra
                if ($LASTEXITCODE -ne 0) { throw 'Real Gemini MCP interoperability failed' }
            }
        }
        if (($CodexInterop -or $GeminiBundle) -and $http -ne '200') {
            throw 'The Cemu HTTPS endpoint did not serve OAuth resource metadata'
        }
        if ($PreparedResources -and $http -eq '200' -and ($CodexInterop -or $GeminiBundle)) {
            if ($IsolatedSaves) { Wait-Step 'hosted save barrier completed' 1 90 }
            Wait-Step 'frame.home.play' 2 90
            foreach ($step in 1..6) { Send-Key 83 }
        } else {
            Send-Key 88
            Wait-Step 'frame.home.agent' 2
            Send-Key 83
        }
        Wait-Step 'frame.home.close'
        Send-Key 90
        Wait-Step 'launcher.closed'
        $restarted = $false
        $pinRestart = $false
        $revoked = $false
        $noAi = $false
        if ($RestartMcp) {
            if ((!$CodexInterop -and !$GeminiBundle) -or $http -ne '200' -or $PreparedResources) {
                throw 'Restart acceptance requires a successful launcher-only MCP client run'
            }
            function Restart-Guest([string]$Phase) {
                Wait-Step 'procui.closed'
                if (!$script:program.HasExited) { $script:program.Kill(); $script:program.WaitForExit() }
                Move-Item -LiteralPath $log -Destination "$Output/before-$Phase.log"
                $script:program = Start-Process -FilePath "$Output/Cemu.exe" -WorkingDirectory $Output -WindowStyle Hidden -ArgumentList $arguments -PassThru
                Wait-Step 'frame.home.profiles'
            }
            function Open-AgentPage {
                foreach ($step in 1..4) { Send-Key 83 }
                Wait-Step 'frame.home.agent'
                Send-Key 90
                Wait-Step 'frame.local.form'
            }
            function Exit-AgentPage {
                Send-Key 88
                Wait-Step 'frame.home.agent' 2
                Send-Key 83
                Wait-Step 'frame.home.close'
                Send-Key 90
                Wait-Step 'launcher.closed'
            }
            function Request-Metadata {
                $result = & python -B -c 'import ssl,sys,urllib.request
try:
    print(urllib.request.urlopen(sys.argv[1],context=ssl.create_default_context(cafile=sys.argv[2]),timeout=4).status)
except OSError:
    print("unavailable")' $url $ca
                return $result
            }
            function Enter-FixturePin {
                # A six-digit test-only PIN. No secret is logged or staged.
                foreach ($step in 1..6) { Send-Key 90 }
                Send-Key 83
                Send-Key 83
                Send-Key 90
            }
            function Verify-Resume {
                if ($GeminiBundle) {
                    python -B $cliFixture $GeminiBundle $origin $ca ($program.MainWindowHandle.ToInt64()) $state --resume --refresh --game-grant
                } else {
                    python -B $cliFixture $origin $ca ($program.MainWindowHandle.ToInt64()) $state --resume
                }
                if ($LASTEXITCODE -ne 0) { throw 'The genuine MCP client could not reuse authorization' }
            }
            Restart-Guest 'automatic-restart'
            if ((Request-Metadata) -ne '200') { throw 'No-PIN service did not restart automatically' }
            Verify-Resume
            $restarted = $true
            Open-AgentPage
            foreach ($step in 1..4) { Send-Key 83 }
            Send-Key 90
            foreach ($step in 1..4) { Send-Key 83 }
            Send-Key 90
            Send-Key 90
            Enter-FixturePin
            Save-Frame 'pin-mode-selected'
            Exit-AgentPage
            Restart-Guest 'pin-restart'
            if ((Request-Metadata) -eq '200') { throw 'PIN-protected service listened before local unlock' }
            Open-AgentPage
            Send-Key 90
            Enter-FixturePin
            $deadline = [DateTime]::UtcNow.AddSeconds(60)
            do {
                if ((Request-Metadata) -eq '200') { break }
                Start-Sleep -Milliseconds 250
            } while ([DateTime]::UtcNow -lt $deadline)
            if ((Request-Metadata) -ne '200') { throw 'PIN-protected service did not start after local unlock' }
            Verify-Resume
            $pinRestart = $true
            Save-Frame 'pin-mode-restarted'
            if ($GeminiBundle) {
                $oldGrant = Join-Path $state '.gemini/mcp-oauth-tokens.json'
                function Request-CredentialStatus([bool]$Refresh) {
                    & python -B -c 'import json,pathlib,ssl,sys,urllib.error,urllib.parse,urllib.request
saved,=json.loads(pathlib.Path(sys.argv[1]).read_bytes())
if sys.argv[3].lower()=="true":
    fields={"grant_type":"refresh_token","client_id":saved["clientId"],"refresh_token":saved["token"]["refreshToken"]}
    request=urllib.request.Request(saved["tokenUrl"],urllib.parse.urlencode(fields).encode(),{"Content-Type":"application/x-www-form-urlencoded"})
else:
    body=b"""{"jsonrpc":"2.0","id":1,"method":"tools/list"}"""
    request=urllib.request.Request(saved["mcpServerUrl"],body,{"Content-Type":"application/json","Authorization":"Bearer "+saved["token"]["accessToken"]})
try:
    with urllib.request.urlopen(request,context=ssl.create_default_context(cafile=sys.argv[2]),timeout=8) as response:
        print(response.status)
except urllib.error.HTTPError as error:
    print(error.code)' $oldGrant $ca $Refresh.ToString()
                }
                foreach ($step in 1..4) { Send-Key 83 }
                Send-Key 90 # Agent Control settings
                foreach ($step in 1..3) { Send-Key 83 }
                Send-Key 90 # No AI / Emergency disable
                if ((Request-CredentialStatus $false) -ne '401') { throw 'No AI did not reject the existing MCP grant' }
                if ((Request-CredentialStatus $true) -ne '403') { throw 'No AI did not reject OAuth refresh' }
                $noAi = $true
                Send-Key 90 # Re-enable locally for the independent revocation case
                Send-Key 88 # Back to Agent Control
                Verify-Resume
                foreach ($step in 1..3) { Send-Key 83 }
                Send-Key 90 # Trusted clients
                Send-Key 90 # The sole Gemini entry
                Send-Key 90 # Revoke
                if ((Request-CredentialStatus $false) -ne '401') { throw 'Revoked MCP access did not return 401' }
                if ((Request-CredentialStatus $true) -ne '400') { throw 'Revoked MCP refresh did not return 400' }
                $revoked = $true
                Send-Key 88 # Back to Agent Control
            }
            Exit-AgentPage
        }
        $evidence = [ordered]@{
            kind = 'wiiu-native-mcp-cemu-acceptance'
            rpx_sha256 = (Get-FileHash -LiteralPath $Rpx).Hash.ToLowerInvariant()
            emulator_sha256 = (Get-FileHash -LiteralPath $Cemu).Hash.ToLowerInvariant()
            ca_exported = [bool]$fingerprint
            listener = $listener.Count -eq 1 -or $http -eq '200'
            protected_resource_metadata_status = $http
            codex_interop = $http -eq '200' -and $CodexInterop.IsPresent
            gemini_interop = $http -eq '200' -and [bool]$GeminiBundle
            hosted_game_round_trip = [bool]$PreparedResources -and $http -eq '200' -and ($CodexInterop -or $GeminiBundle)
            no_pin_restart = $restarted
            pin_restart_unlock = $pinRestart
            revoked_access_rejected = $revoked
            no_ai_refused_access = $noAi
            launcher_closed = $true
        }
        $evidence | ConvertTo-Json | Set-Content -LiteralPath "$Output/cemu-mcp-acceptance.json" -Encoding ascii
        $evidence | ConvertTo-Json
        return
    }
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
