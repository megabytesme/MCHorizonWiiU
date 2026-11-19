# Agent Control on physical Wii U

## Pair a development PC

1. Connect the Wii U and the PC to the same trusted private LAN. Launch Horizon and wait for the title menu.
2. On the Wii U, open **Horizon Settings → AI Agent**. With **No AI** off, enable **Agent Control**. The page shows the bound IPv4 address and port and a 32-character pairing code. Keep this page out of photographs, recordings and screen sharing while the code is visible.
3. On the PC, use a `horizon-ctl` binary built from this version of Horizon. In PowerShell, enter the displayed code at the masked prompt. Replace the example endpoint with the one on the Wii U.

   ```powershell
   $ctl = 'C:\path\to\horizon-ctl.exe'
   $endpoint = '192.168.0.25:37121'
   $code = Read-Host 'Wii U pairing code' -AsSecureString
   $buffer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($code)
   try {
       $env:HORIZON_CONTROL_PAIRING = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($buffer)
   } finally {
       [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($buffer)
       $code.Dispose()
   }
   & $ctl --remote $endpoint state
   & $ctl --remote $endpoint pad status
   ```

4. Confirm that `state` reports the Wii U's current screen and `pad status` returns controller state. These are read-only checks. `screenshot tv.png` and `screenshot-gamepad gamepad.png` write to the PC when frames are available. During gameplay, `state` also reports position and world information. Use `horizon-ctl help` for commands and platform-specific availability.
5. Turn off **Agent Control**, turn on **No AI**, or exit the game to close the connection. For a new pairing, enable Agent Control again and use the new endpoint and code. Clear the shell's copy with `Remove-Item Env:HORIZON_CONTROL_PAIRING`.

Each native launch requires local authorization. Start with a read-only state check before sending gameplay input. Only one PC connection is handled at a time. Close an active `horizon-ctl mcp` session before connecting a second client.

## Configure Codex or another MCP client

Codex uses `horizon-ctl` as a **local stdio** MCP server. Add this entry to your local Codex `config.toml` (on Windows, `%USERPROFILE%\.codex\config.toml`). Use the absolute path to the executable and the currently displayed address and port.

```toml
[mcp_servers.horizon_wiiu]
command = 'C:\path\to\horizon-ctl.exe'
args = ['--remote', '192.168.0.25:37121', 'mcp']
env_vars = ['HORIZON_CONTROL_PAIRING']
```

Launch `codex` from the PowerShell session where you entered the code. For Codex Desktop or an IDE extension, the local MCP process must inherit that variable from the host that starts it. Restart that host with the variable set, then check `/mcp` and call `state`. In other MCP clients, configure the same command, arguments and environment forwarding. Keep the pairing code out of command arguments and MCP configuration files. [Codex MCP settings](https://developers.openai.com/codex/mcp)

## Choose a fixed address or port

By default Horizon queries the console's assigned private IPv4 address and lets the network stack select a free port. To override the endpoint, put a `control-lan.json` file beside the RPX in the app's game directory. This file contains only the listen address and port. Set `listen` to a private IPv4 address assigned to the console and use port `0` if you want the network stack to choose the port. Apply changes by disabling and re-enabling Agent Control.

```json
{"listen":"192.168.0.25:37121"}
```

An invalid address, unavailable port or entropy failure leaves the listener closed. Check the Wii U connection and the optional file, then try enabling Agent Control again.

## Transport and limitations

Cafe generates a fresh 128-bit code from the production system random source on each activation and retains it in memory. The LAN connection uses Horizon's authenticated ChaCha20-Poly1305 framing with fresh session randoms, directional keys and authenticated key confirmation before forwarding commands to the existing server. It provides encryption and integrity. A party that later obtains the same pairing code can decrypt a recorded session from that activation. A party that can reach the port can also occupy the connection temporarily. Keep the service on a trusted private LAN and leave router port forwarding off.

The Wii U shows the code locally for pairing. The SD file, ordinary logs and copied connection details contain no code. Keep the PC environment private to the local account and trusted processes. Turning off Agent Control revokes the current code. Desktop's local socket and Cemu's opt-in loopback development transport are unchanged.
