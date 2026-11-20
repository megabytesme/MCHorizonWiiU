# MCP tool parity

The pinned schema comes from the actual tool registrations. The second
snapshot audits upstream main as of 20 November 2026. The machine-readable
inventory includes the exact descriptions and schemas, result contracts,
scopes, confirmations and original side-effect annotations. [Inventory](parity.json)

| Category | Disposition |
| --- | --- |
| 1 | Implement directly |
| 2 | Reuse the game implementation |
| 3 | Adapt to CafeOS |
| 4 | Builder-side operation |
| 5 | Deferred until a reviewed backend exists |
| 6 | Not applicable to Wii U |

## Pinned inventory

69fab597a6f0e8a6ce524d38d1b73e599e343034. 192 entries, 44 mapped to Wii U source.

| Tool | Official source | Availability | Scopes | Side effects | Wii U disposition |
| --- | --- | --- | --- | --- | --- |
| play | launcher/src/mcp.rs (play) | launcher-only | launcher.manage, game.control | mutating, network | 3. Queue a normal hosted launch of the selected installation or an existing local world. |
| server | launcher/src/mcp.rs (server) | app-wide | launcher.manage | mutating, network | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| server_command | launcher/src/mcp.rs (server_command) | app-wide | launcher.manage | mutating | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| server_log | launcher/src/mcp.rs (server_log) | app-wide | launcher.read | read-only | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| server_stop | launcher/src/mcp.rs (server_stop) | app-wide | launcher.manage | mutating | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| account_list | launcher/src/mcp.rs (account_list) | app-wide | launcher.read | read-only | 5. The separate accounts integration owns Microsoft and account tools. |
| account_sign_in | launcher/src/mcp.rs (account_sign_in) | app-wide | launcher.manage | mutating, network | 5. The separate accounts integration owns Microsoft and account tools. |
| versions | horizon-launcher versions | app-wide | launcher.read | read-only, network | 3. Installed Cafe build and preparation status. Desktop release queries are excluded. |
| install | horizon-launcher install | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| remove | horizon-launcher remove | app-wide | launcher.manage | mutating, destructive | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| prepare | horizon-launcher prepare | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| native_cache | horizon-launcher native-cache | app-wide | launcher.read | read-only | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| notes | horizon-launcher notes | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| links | horizon-launcher links | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| account_refresh | horizon-launcher account refresh | app-wide | launcher.manage | mutating, network | 5. The separate accounts integration owns Microsoft and account tools. |
| account_sign_out | horizon-launcher account sign-out | app-wide | launcher.manage | mutating, destructive | 5. The separate accounts integration owns Microsoft and account tools. |
| account_select | horizon-launcher account select | app-wide | launcher.manage | mutating | 5. The separate accounts integration owns Microsoft and account tools. |
| account_offline | horizon-launcher account offline | app-wide | launcher.manage | mutating | 5. The separate accounts integration owns Microsoft and account tools. |
| account_forget_offline | horizon-launcher account forget-offline | app-wide | launcher.manage | mutating, destructive | 5. The separate accounts integration owns Microsoft and account tools. |
| account_skin | horizon-launcher account skin | app-wide | launcher.manage | mutating, network | 5. The separate accounts integration owns Microsoft and account tools. |
| settings | horizon-launcher settings | app-wide | launcher.read | read-only, mutating | 1. Read effective Night settings. Desktop settings writes are excluded. |
| game_args | horizon-launcher game-args | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| share_log | horizon-launcher share-log | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| unshare_log | horizon-launcher unshare-log | app-wide | launcher.manage | mutating, destructive, network | 5. Night has no reviewed MCP dispatch for this command. |
| logs | horizon-launcher logs | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| update | horizon-launcher update | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| desktop_entry | horizon-launcher desktop-entry | app-wide | launcher.manage | mutating | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| data_status | horizon-launcher data status | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| data_allow | horizon-launcher data allow | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| data_deny | horizon-launcher data deny | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| data_show | horizon-launcher data show | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| data_delete | horizon-launcher data delete | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| data_send | horizon-launcher data send | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| data_bug | horizon-launcher data bug | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| data_benchmark | horizon-launcher data benchmark | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| instance_list | horizon-launcher instance list | app-wide | launcher.read | read-only | 3. Existing Night installations and their preparation status. |
| instance_create | horizon-launcher instance create | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_versions | horizon-launcher instance versions | app-wide | launcher.read | read-only, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| instance_java | horizon-launcher instance java | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| instance_loader | horizon-launcher instance loader | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| instance_rename | horizon-launcher instance rename | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_delete | horizon-launcher instance delete | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| instance_icon | horizon-launcher instance icon | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_group | horizon-launcher instance group | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_select | horizon-launcher instance select | app-wide | launcher.manage | mutating | 3. Persistently select a compatible installation. |
| instance_version | horizon-launcher instance version | app-wide | launcher.manage | mutating | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| instance_export | horizon-launcher instance export | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_launch | horizon-launcher instance launch | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| instance_shortcut | horizon-launcher instance shortcut | app-wide | launcher.manage | mutating | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| instance_import | horizon-launcher instance import | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_others | horizon-launcher instance others | app-wide | launcher.read | read-only | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| instance_copy_from | horizon-launcher instance copy-from | app-wide | launcher.manage | mutating | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| worlds_list | horizon-launcher worlds list | app-wide | launcher.read | read-only | 3. Validated saves for the selected instance with availability, name and ID. |
| worlds_backup | horizon-launcher worlds backup | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| worlds_copy | horizon-launcher worlds copy | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| worlds_delete | horizon-launcher worlds delete | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| worlds_datapack_list | horizon-launcher worlds datapack-list | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| worlds_datapack_add | horizon-launcher worlds datapack-add | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| worlds_datapack_remove | horizon-launcher worlds datapack-remove | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| packs_list | horizon-launcher packs list | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| packs_add | horizon-launcher packs add | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| packs_on | horizon-launcher packs on | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| packs_off | horizon-launcher packs off | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| packs_remove | horizon-launcher packs remove | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| java_list | horizon-launcher java list | app-wide | launcher.read | read-only | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| java_install | horizon-launcher java install | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| screenshots_list | horizon-launcher screenshots list | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| screenshots_delete | horizon-launcher screenshots delete | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| multiplayer_list | horizon-launcher multiplayer list | app-wide | launcher.read | read-only | 3. Night's shared saved-server list. |
| multiplayer_add | horizon-launcher multiplayer add | app-wide | launcher.manage | mutating | 3. Add to Night's shared saved-server list. No connection is made. |
| multiplayer_remove | horizon-launcher multiplayer remove | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| modpack_list | horizon-launcher modpack list | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| modpack_import | horizon-launcher modpack import | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| modpack_port | horizon-launcher modpack port | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| modpack_play | horizon-launcher modpack play | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| modpack_remove | horizon-launcher modpack remove | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| mods_list | horizon-launcher mods list | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| mods_search | horizon-launcher mods search | app-wide | launcher.read | read-only, network | 5. Night has no reviewed MCP dispatch for this command. |
| mods_check | horizon-launcher mods check | app-wide | launcher.read | read-only, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| mods_install | horizon-launcher mods install | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| mods_update | horizon-launcher mods update | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| mods_remove | horizon-launcher mods remove | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| mods_enable | horizon-launcher mods enable | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| mods_disable | horizon-launcher mods disable | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| rust_status | horizon-launcher rust status | app-wide | launcher.read | read-only | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| rust_install | horizon-launcher rust install | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| rust_remove | horizon-launcher rust remove | app-wide | launcher.manage | mutating, destructive | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| servers_list | horizon-launcher servers list | app-wide | launcher.read | read-only | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| servers_create | horizon-launcher servers create | app-wide | launcher.manage | mutating, network | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| servers_set_up | horizon-launcher servers set-up | app-wide | launcher.manage | mutating, network | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| servers_run | horizon-launcher servers run | app-wide | launcher.manage | mutating, network | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| servers_forget | horizon-launcher servers forget | app-wide | launcher.manage | mutating, destructive | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| benchmark | horizon-launcher benchmark | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| launch | horizon-ctl launch | gameplay | game.control | mutating | 6. The official launcher filters launch out. Use play. |
| game_wait_for | horizon-ctl wait-for | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_events | horizon-ctl events | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_input_log | horizon-ctl input-log | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_wait_ready | horizon-ctl wait-ready | gameplay | game.observe | read-only | 2. Poll original game status for up to 10 seconds per request. |
| game_logs | horizon-ctl logs | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_status | horizon-ctl status | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_hello | horizon-ctl hello | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_memory | horizon-ctl memory | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_worlds | horizon-ctl worlds | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_world_rename | horizon-ctl world-rename | gameplay | game.control | mutating, destructive | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_world_backup | horizon-ctl world-backup | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_world_delete | horizon-ctl world-delete | gameplay | game.control | mutating, destructive | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_open | horizon-ctl open | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_save | horizon-ctl save | gameplay | game.control | mutating | 2. Request the original game save. |
| game_quit | horizon-ctl quit | gameplay | game.control | mutating | 2. Request the original hosted save-and-return path. |
| game_screenshot | horizon-ctl screenshot | gameplay | game.observe | read-only | 2. Use the original Cafe TV encoder and return PNG content, bounded to 7 MiB of base64. |
| game_state | horizon-ctl state | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_looking_at | horizon-ctl looking-at | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_entities | horizon-ctl entities | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_block | horizon-ctl block | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_light | horizon-ctl light | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_blocks | horizon-ctl blocks | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_find_block | horizon-ctl find-block | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_inventory | horizon-ctl inventory | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_screen | horizon-ctl screen | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_messages | horizon-ctl messages | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_narration | horizon-ctl narration | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_move | horizon-ctl move | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_goto | horizon-ctl goto | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_jump | horizon-ctl jump | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_look | horizon-ctl look | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_turn | horizon-ctl turn | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_mouse_move | horizon-ctl mouse-move | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_look_at | horizon-ctl look-at | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_look_at_block | horizon-ctl look-at-block | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_look_at_entity | horizon-ctl look-at-entity | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_tp | horizon-ctl tp | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_emote | horizon-ctl emote | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_break | horizon-ctl break | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_place | horizon-ctl place | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_interact | horizon-ctl interact | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_use_item | horizon-ctl use-item | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_attack | horizon-ctl attack | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_select | horizon-ctl select | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_drop | horizon-ctl drop | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_key | horizon-ctl key | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_mouse | horizon-ctl mouse | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_interact_entity | horizon-ctl interact-entity | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_pad | horizon-ctl pad | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_release | horizon-ctl release | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_stop | horizon-ctl stop | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_open_inventory | horizon-ctl open-inventory | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_click_slot | horizon-ctl click-slot | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_close | horizon-ctl close | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_ui | horizon-ctl ui | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. |
| game_click | horizon-ctl click | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_set | horizon-ctl set | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_options_get | horizon-ctl options-get | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_options_set | horizon-ctl options-set | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_click_button | horizon-ctl click-button | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_click_at | horizon-ctl click-at | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_drag_at | horizon-ctl drag-at | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_scroll_at | horizon-ctl scroll-at | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_type | horizon-ctl type | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_chat | horizon-ctl chat | gameplay | game.control | mutating, network | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_command | horizon-ctl command | gameplay | game.control | mutating, network | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_respawn | horizon-ctl respawn | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_pause | horizon-ctl pause | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_resume | horizon-ctl resume | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. |
| game_options | horizon-ctl options | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_book | horizon-ctl book | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_signedit | horizon-ctl signedit | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_mount_menu | horizon-ctl mount-menu | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_connect | horizon-ctl connect | gameplay | game.control | mutating, network | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_disconnect | horizon-ctl disconnect | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_servers | horizon-ctl servers | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_lan | horizon-ctl lan | gameplay | game.control | mutating, network | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_friends | horizon-ctl friends | gameplay | game.control | mutating, network | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_give | horizon-ctl give | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_time | horizon-ctl time | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_setblock | horizon-ctl setblock | gameplay | game.control | mutating, destructive | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_summon | horizon-ctl summon | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_effect | horizon-ctl effect | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_effects | horizon-ctl effects | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_xp | horizon-ctl xp | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_enchant | horizon-ctl enchant | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_potion | horizon-ctl potion | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_fill | horizon-ctl fill | gameplay | game.control | mutating, destructive | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_container | horizon-ctl container | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_wait | horizon-ctl wait | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_bench | horizon-ctl bench | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_stats | horizon-ctl stats | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_record_start | horizon-ctl record-start | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_record_stop | horizon-ctl record-stop | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_record_status | horizon-ctl record-status | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_record_list | horizon-ctl record-list | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_record_replay | horizon-ctl record-replay | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_repro_export | horizon-ctl repro-export | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |

## Reference inventory

e568ef530e10a524fa5dac1e8faaaa2a3c59762e. 194 entries, 44 mapped to Wii U source.

| Tool | Official source | Availability | Scopes | Side effects | Wii U disposition |
| --- | --- | --- | --- | --- | --- |
| play | launcher/src/mcp.rs (play) | launcher-only | launcher.manage, game.control | mutating, network | 3. Queue a normal hosted launch of the selected installation or an existing local world. Wii U advertises the pinned schema. |
| server | launcher/src/mcp.rs (server) | app-wide | launcher.manage | mutating, network | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| server_command | launcher/src/mcp.rs (server_command) | app-wide | launcher.manage | mutating | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| server_log | launcher/src/mcp.rs (server_log) | app-wide | launcher.read | read-only | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| server_stop | launcher/src/mcp.rs (server_stop) | app-wide | launcher.manage | mutating | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| account_list | launcher/src/mcp.rs (account_list) | app-wide | launcher.read | read-only | 5. The separate accounts integration owns Microsoft and account tools. |
| account_sign_in | launcher/src/mcp.rs (account_sign_in) | app-wide | launcher.manage | mutating, network | 5. The separate accounts integration owns Microsoft and account tools. |
| versions | horizon-launcher versions | app-wide | launcher.read | read-only, network | 3. Installed Cafe build and preparation status. Desktop release queries are excluded. Wii U advertises the pinned schema. |
| install | horizon-launcher install | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| remove | horizon-launcher remove | app-wide | launcher.manage | mutating, destructive | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| prepare | horizon-launcher prepare | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| storage | horizon-launcher storage | app-wide | launcher.read | read-only, mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| native_lock | horizon-launcher native-lock | app-wide | launcher.read | read-only | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| notes | horizon-launcher notes | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| links | horizon-launcher links | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| account_refresh | horizon-launcher account refresh | app-wide | launcher.manage | mutating, network | 5. The separate accounts integration owns Microsoft and account tools. |
| account_sign_out | horizon-launcher account sign-out | app-wide | launcher.manage | mutating, destructive | 5. The separate accounts integration owns Microsoft and account tools. |
| account_select | horizon-launcher account select | app-wide | launcher.manage | mutating | 5. The separate accounts integration owns Microsoft and account tools. |
| settings | horizon-launcher settings | app-wide | launcher.read | read-only, mutating | 1. Read effective Night settings. Desktop settings writes are excluded. Wii U advertises the pinned schema. |
| game_args | horizon-launcher game-args | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| share_log | horizon-launcher share-log | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| unshare_log | horizon-launcher unshare-log | app-wide | launcher.manage | mutating, destructive, network | 5. Night has no reviewed MCP dispatch for this command. |
| logs | horizon-launcher logs | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| update | horizon-launcher update | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| desktop_entry | horizon-launcher desktop-entry | app-wide | launcher.manage | mutating | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| bug_report | horizon-launcher bug-report | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| data_status | horizon-launcher data status | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| data_allow | horizon-launcher data allow | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| data_deny | horizon-launcher data deny | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| data_show | horizon-launcher data show | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| data_delete | horizon-launcher data delete | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| data_send | horizon-launcher data send | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| data_bug | horizon-launcher data bug | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| data_benchmark | horizon-launcher data benchmark | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| instance_list | horizon-launcher instance list | app-wide | launcher.read | read-only | 3. Existing Night installations and their preparation status. Wii U advertises the pinned schema. |
| instance_create | horizon-launcher instance create | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_versions | horizon-launcher instance versions | app-wide | launcher.read | read-only, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| instance_java | horizon-launcher instance java | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| instance_loader | horizon-launcher instance loader | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| instance_rename | horizon-launcher instance rename | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_delete | horizon-launcher instance delete | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| instance_icon | horizon-launcher instance icon | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_group | horizon-launcher instance group | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_select | horizon-launcher instance select | app-wide | launcher.manage | mutating | 3. Persistently select a compatible installation. Wii U advertises the pinned schema. |
| instance_version | horizon-launcher instance version | app-wide | launcher.manage | mutating | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| instance_export | horizon-launcher instance export | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_launch | horizon-launcher instance launch | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| instance_shortcut | horizon-launcher instance shortcut | app-wide | launcher.manage | mutating | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| instance_import | horizon-launcher instance import | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| instance_others | horizon-launcher instance others | app-wide | launcher.read | read-only | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| instance_copy_from | horizon-launcher instance copy-from | app-wide | launcher.manage | mutating | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| worlds_list | horizon-launcher worlds list | app-wide | launcher.read | read-only | 3. Validated saves for the selected instance with availability, name and ID. Wii U advertises the pinned schema. |
| worlds_backup | horizon-launcher worlds backup | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| worlds_copy | horizon-launcher worlds copy | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| worlds_delete | horizon-launcher worlds delete | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| worlds_datapack_list | horizon-launcher worlds datapack-list | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| worlds_datapack_add | horizon-launcher worlds datapack-add | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| worlds_datapack_remove | horizon-launcher worlds datapack-remove | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| packs_list | horizon-launcher packs list | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| packs_add | horizon-launcher packs add | app-wide | launcher.manage | mutating, network | 5. Night has no reviewed MCP dispatch for this command. |
| packs_on | horizon-launcher packs on | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| packs_off | horizon-launcher packs off | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| packs_remove | horizon-launcher packs remove | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| java_list | horizon-launcher java list | app-wide | launcher.read | read-only | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| java_install | horizon-launcher java install | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| screenshots_list | horizon-launcher screenshots list | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| screenshots_delete | horizon-launcher screenshots delete | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| multiplayer_list | horizon-launcher multiplayer list | app-wide | launcher.read | read-only | 3. Night's shared saved-server list. Wii U advertises the pinned schema. |
| multiplayer_add | horizon-launcher multiplayer add | app-wide | launcher.manage | mutating | 3. Add to Night's shared saved-server list. No connection is made. Wii U advertises the pinned schema. |
| multiplayer_remove | horizon-launcher multiplayer remove | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| modpack_list | horizon-launcher modpack list | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| modpack_import | horizon-launcher modpack import | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| modpack_port | horizon-launcher modpack port | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| modpack_play | horizon-launcher modpack play | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| modpack_remove | horizon-launcher modpack remove | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| mods_list | horizon-launcher mods list | app-wide | launcher.read | read-only | 5. Night has no reviewed MCP dispatch for this command. |
| mods_search | horizon-launcher mods search | app-wide | launcher.read | read-only, network | 5. Night has no reviewed MCP dispatch for this command. |
| mods_check | horizon-launcher mods check | app-wide | launcher.read | read-only, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| mods_install | horizon-launcher mods install | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| mods_update | horizon-launcher mods update | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| mods_remove | horizon-launcher mods remove | app-wide | launcher.manage | mutating, destructive | 5. Night has no reviewed MCP dispatch for this command. |
| mods_enable | horizon-launcher mods enable | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| mods_disable | horizon-launcher mods disable | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| rust_status | horizon-launcher rust status | app-wide | launcher.read | read-only | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| rust_install | horizon-launcher rust install | app-wide | launcher.manage | mutating, network | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| rust_remove | horizon-launcher rust remove | app-wide | launcher.manage | mutating, destructive | 4. Uses the pinned desktop builder, preparation, or installation tooling. |
| servers_list | horizon-launcher servers list | app-wide | launcher.read | read-only | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| servers_create | horizon-launcher servers create | app-wide | launcher.manage | mutating, network | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| servers_set_up | horizon-launcher servers set-up | app-wide | launcher.manage | mutating, network | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| servers_run | horizon-launcher servers run | app-wide | launcher.manage | mutating, network | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| servers_forget | horizon-launcher servers forget | app-wide | launcher.manage | mutating, destructive | 6. Desktop integration or dedicated-server hosting is unavailable on CafeOS. |
| benchmark | horizon-launcher benchmark | app-wide | launcher.manage | mutating | 5. Night has no reviewed MCP dispatch for this command. |
| launch | horizon-ctl launch | gameplay | game.control | mutating | 6. The official launcher filters launch out. Use play. |
| game_wait_for | horizon-ctl wait-for | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_events | horizon-ctl events | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_input_log | horizon-ctl input-log | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_wait_ready | horizon-ctl wait-ready | gameplay | game.observe | read-only | 2. Poll original game status for up to 10 seconds per request. Wii U advertises the pinned schema. |
| game_logs | horizon-ctl logs | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_status | horizon-ctl status | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_hello | horizon-ctl hello | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_memory | horizon-ctl memory | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_worlds | horizon-ctl worlds | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_world_rename | horizon-ctl world-rename | gameplay | game.control | mutating, destructive | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_world_backup | horizon-ctl world-backup | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_world_delete | horizon-ctl world-delete | gameplay | game.control | mutating, destructive | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_open | horizon-ctl open | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_save | horizon-ctl save | gameplay | game.control | mutating | 2. Request the original game save. Wii U advertises the pinned schema. |
| game_quit | horizon-ctl quit | gameplay | game.control | mutating | 2. Request the original hosted save-and-return path. Wii U advertises the pinned schema. |
| game_screenshot | horizon-ctl screenshot | gameplay | game.observe | read-only | 2. Use the original Cafe TV encoder and return PNG content, bounded to 7 MiB of base64. Wii U advertises the pinned schema. |
| game_state | horizon-ctl state | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_looking_at | horizon-ctl looking-at | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_entities | horizon-ctl entities | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_block | horizon-ctl block | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_light | horizon-ctl light | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_blocks | horizon-ctl blocks | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_find_block | horizon-ctl find-block | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_inventory | horizon-ctl inventory | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_screen | horizon-ctl screen | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_messages | horizon-ctl messages | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_narration | horizon-ctl narration | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_move | horizon-ctl move | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_goto | horizon-ctl goto | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_jump | horizon-ctl jump | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_look | horizon-ctl look | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_turn | horizon-ctl turn | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_mouse_move | horizon-ctl mouse-move | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_look_at | horizon-ctl look-at | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_look_at_block | horizon-ctl look-at-block | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_look_at_entity | horizon-ctl look-at-entity | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_tp | horizon-ctl tp | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_emote | horizon-ctl emote | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_break | horizon-ctl break | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_place | horizon-ctl place | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_interact | horizon-ctl interact | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_use_item | horizon-ctl use-item | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_attack | horizon-ctl attack | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_select | horizon-ctl select | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_drop | horizon-ctl drop | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_hunt | horizon-ctl hunt | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_collect | horizon-ctl collect | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_key | horizon-ctl key | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_mouse | horizon-ctl mouse | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_interact_entity | horizon-ctl interact-entity | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_pad | horizon-ctl pad | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_release | horizon-ctl release | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_stop | horizon-ctl stop | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_open_inventory | horizon-ctl open-inventory | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_click_slot | horizon-ctl click-slot | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_close | horizon-ctl close | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_ui | horizon-ctl ui | gameplay | game.observe | read-only | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_click | horizon-ctl click | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_set | horizon-ctl set | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_options_get | horizon-ctl options-get | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_options_set | horizon-ctl options-set | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_click_button | horizon-ctl click-button | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_click_at | horizon-ctl click-at | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_drag_at | horizon-ctl drag-at | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_scroll_at | horizon-ctl scroll-at | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_type | horizon-ctl type | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_chat | horizon-ctl chat | gameplay | game.control | mutating, network | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_command | horizon-ctl command | gameplay | game.control | mutating, network | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_respawn | horizon-ctl respawn | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_pause | horizon-ctl pause | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_resume | horizon-ctl resume | gameplay | game.control | mutating | 2. Direct dispatch through the original gameplay control handler. Wii U advertises the pinned schema. |
| game_options | horizon-ctl options | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_book | horizon-ctl book | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_signedit | horizon-ctl signedit | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_mount_menu | horizon-ctl mount-menu | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_connect | horizon-ctl connect | gameplay | game.control | mutating, network | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_disconnect | horizon-ctl disconnect | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_servers | horizon-ctl servers | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_lan | horizon-ctl lan | gameplay | game.control | mutating, network | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_friends | horizon-ctl friends | gameplay | game.control | mutating, network | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_give | horizon-ctl give | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_time | horizon-ctl time | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_setblock | horizon-ctl setblock | gameplay | game.control | mutating, destructive | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_summon | horizon-ctl summon | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_effect | horizon-ctl effect | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_effects | horizon-ctl effects | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_xp | horizon-ctl xp | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_enchant | horizon-ctl enchant | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_potion | horizon-ctl potion | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_fill | horizon-ctl fill | gameplay | game.control | mutating, destructive | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_container | horizon-ctl container | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_wait | horizon-ctl wait | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_bench | horizon-ctl bench | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_stats | horizon-ctl stats | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_profile | horizon-ctl profile | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_record_start | horizon-ctl record-start | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_record_stop | horizon-ctl record-stop | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_record_status | horizon-ctl record-status | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_record_list | horizon-ctl record-list | gameplay | game.observe | read-only | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_record_replay | horizon-ctl record-replay | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
| game_repro_export | horizon-ctl repro-export | gameplay | game.control | mutating | 5. No reviewed Wii U MCP adapter. The original game and diagnostic transport retain their own capabilities. |
