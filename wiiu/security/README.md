# Horizon Night secret storage

`horizon-wiiu-security` provides `SecretStore` for one local PIN domain.
MCP uses a single domain under the app's `private/mcp` directory. It can
use separate names such as `https_private_key` and `pairing_secret` in
that domain. Accounts use one domain per Minecraft UUID under
`private/microsoft`. Give each service its own store and keep the store,
PIN prompts, and returned secret bytes out of remote dispatch.

- Call `status()` during service startup. An uninitialized vault defaults to
  PIN protection. Ask the local player to set a PIN before the first `put`.
- For a locked vault, ask the player locally and call `unlock`. Keep the
  store alive across launcher and gameplay transitions. `lock` drops the
  in-memory key and credentials. The caller should also drop any secret
  it has already retrieved.
- Change a PIN or storage policy with `set_protection` on an unlocked
  store. Use `ConfirmedUnprotected` only after local, explicit confirmation.
  Tell the player that copies of the SD card can reveal authentication tokens
  or server private keys, that these may permit account or MCP impersonation,
  and that the app applies no encryption in this mode. PIN protection can be
  enabled later. Warn that offline guessing of short PINs bypasses local
  attempt limits and offer a passphrase.
- `Secret` owns its bytes and zeroes them on drop. Use `expose()` only when
  handing bytes directly to the service that needs them. An account token
  must never be accessible to MCP callers.
- `remove` deletes a named credential. SD wear leveling and backups limit
  physical erasure. Check `cleanup_pending` for an older copy the app could
  not retire. Report errors without serializing secret values.
- Instantiate and share only one writable store per domain in the app.
  Call through its owner or serialize access with a mutex.

Use `Vault::open(&app_root.join("private"), "mcp")` and the `SecretStore`
methods above. Import the crate as a path dependency. Cafe builds must
enable the existing custom getrandom backend and run
`horizon_wiiu_entropy::link()` before opening any random-dependent service.
The app owns the PIN prompt. Pass its UTF-8 bytes to the vault and clear
the keyboard buffer after use. After a successful unlock, keep the
`Vault` in the shared app state across game sessions.

## On-disk format

Version 1 uses RustCrypto Argon2id v1.3 with 19,456 KiB of working memory,
two passes, one lane and a 32-byte key. Each protection setup or PIN change
gets an independent random 16-byte salt. Each protected write gets a fresh
24-byte nonce for XChaCha20-Poly1305. The AEAD authenticates the domain and
header, including the format version, policy, generation and KDF parameters.
The file carries the detached 16-byte tag and a separate SHA-256 checksum
for detecting accidental damage before requesting a PIN. A checksum can be
recomputed by somebody with SD access. If authenticated decryption fails,
the app treats wrong PIN and unauthenticated modification alike.

Cafe writes `vault.pending`, flushes the file, reads it back and verifies
the new representation before renaming into an absent slot. A pending file
is ignored after an interruption. A valid adjacent committed generation
takes precedence over its predecessor. Invalid committed files are reported
as damaged to avoid silently rolling back a PIN change or recovering a
removed token. All stages of protected-to-protected migration remain
encrypted on disk. A `SessionOnly` transition leaves at most an empty
generation marker. An explicit whole-domain sign-out may call
`Vault::erase_domain()` to replace even a locked account's vault with that
marker without recovering its contents.

Cafe's filesystem does not expose directory fsync, so the journal relies
on the synchronous rename and unlink operations for metadata. Removable
media can fail between writes. Check `cleanup_pending` if an older copy
could not be retired, especially after turning PIN protection on. SD
controllers and copies of the card can retain old credentials even after
successful removal. A protected vault relies on the PIN or passphrase and
has no hardware-backed key or anti-rollback counter. Prefer a long
passphrase because someone who copies the card can try PINs offline.

RustCrypto's scratch blocks, derived keys and owned secret buffers are
zeroized. The host release test measured about 56 ms for setup including
file I/O on Ben-Desktop. Cafe uses the repository's guarded NSS entropy
adapter. Physical Wii U timing, entropy and power-loss behavior require
hardware validation. [Argon2 specification](https://www.rfc-editor.org/rfc/rfc9106) ·
[RustCrypto AEAD implementation](https://github.com/RustCrypto/AEADs) ·
[Cafe entropy adapter](../entropy/README.md)
