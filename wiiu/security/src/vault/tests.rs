use super::*;
use std::time::Instant;

const PIN: &[u8] = b"sample passphrase";
const NEW_PIN: &[u8] = b"replacement passphrase";
const TOKEN: &[u8] = b"my synthetic refresh credential";
const KEY: &str = "refresh";
const DOMAIN: &str = "microsoft/0123456789abcdef0123456789abcdef";

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let mut id = [0; 16];
        getrandom::fill(&mut id).unwrap();
        let path = std::env::temp_dir().join(format!(
            "horizon-vault-test-{:032x}",
            u128::from_le_bytes(id)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn protected(vault: &mut Vault, pin: &[u8]) {
    vault
        .set_protection(Protection::PinProtected { new_pin: pin })
        .unwrap();
}

fn store(vault: &mut Vault) {
    vault.put(KEY, Secret::new(TOKEN.to_vec())).unwrap();
}

fn expect_token(vault: &mut Vault) {
    assert_eq!(vault.get(KEY).unwrap().unwrap().expose(), TOKEN);
}

fn active_file(vault: &Vault) -> PathBuf {
    vault.directory.join(SLOTS[vault.active.unwrap().0])
}

#[test]
fn protected_restart_lock_wrong_pin_and_corruption() {
    let scratch = Scratch::new();
    let mut vault = Vault::open(&scratch.0, DOMAIN).unwrap();
    assert_eq!(vault.status().mode, StorageMode::PinProtected);
    assert_eq!(vault.get(KEY).unwrap_err(), VaultError::SetupRequired);
    let start = Instant::now();
    protected(&mut vault, PIN);
    eprintln!(
        "Argon2id setup on host with {} KiB: {:?}",
        format::MEMORY_KIB,
        start.elapsed()
    );
    store(&mut vault);
    let path = active_file(&vault);
    let on_disk = fs::read(&path).unwrap();
    assert!(!on_disk.windows(TOKEN.len()).any(|s| s == TOKEN));
    drop(vault);

    let mut vault = Vault::open(&scratch.0, DOMAIN).unwrap();
    assert_eq!(vault.get(KEY).unwrap_err(), VaultError::Locked);
    assert_eq!(
        vault.unlock(b"incorrect PIN"),
        Err(VaultError::AuthenticationFailed)
    );
    vault.unlock(PIN).unwrap();
    expect_token(&mut vault);
    vault.lock();
    assert_eq!(vault.get(KEY).unwrap_err(), VaultError::Locked);
    let mut corrupted = on_disk;
    corrupted[70] ^= 1;
    fs::write(path, corrupted).unwrap();
    assert!(matches!(
        Vault::open(&scratch.0, DOMAIN),
        Err(VaultError::Corrupt)
    ));
}

#[test]
fn migration_changes_salt_and_updates_change_nonce() {
    let scratch = Scratch::new();
    let mut vault = Vault::open(&scratch.0, DOMAIN).unwrap();
    protected(&mut vault, PIN);
    store(&mut vault);
    let first = fs::read(active_file(&vault)).unwrap();
    store(&mut vault);
    let next = fs::read(active_file(&vault)).unwrap();
    assert_eq!(first[30..46], next[30..46]);
    assert_ne!(first[46..70], next[46..70]);

    protected(&mut vault, NEW_PIN);
    let changed = fs::read(active_file(&vault)).unwrap();
    assert_ne!(changed[30..46], next[30..46]);
    vault.lock();
    assert_eq!(vault.unlock(PIN), Err(VaultError::AuthenticationFailed));
    vault.unlock(NEW_PIN).unwrap();
    expect_token(&mut vault);
    vault
        .set_protection(Protection::ConfirmedUnprotected)
        .unwrap();
    drop(vault);

    let mut vault = Vault::open(&scratch.0, DOMAIN).unwrap();
    assert_eq!(vault.status().mode, StorageMode::Unprotected);
    expect_token(&mut vault);
    protected(&mut vault, PIN);
    assert!(!vault.status().cleanup_pending);
    for entry in fs::read_dir(&vault.directory).unwrap() {
        let bytes = fs::read(entry.unwrap().path()).unwrap();
        assert!(!bytes.windows(TOKEN.len()).any(|s| s == TOKEN));
    }
    drop(vault);
    let mut vault = Vault::open(&scratch.0, DOMAIN).unwrap();
    assert_eq!(vault.get(KEY).unwrap_err(), VaultError::Locked);
    vault.unlock(PIN).unwrap();
    expect_token(&mut vault);
}

#[test]
fn interrupted_migration_and_commit_recovery() {
    let scratch = Scratch::new();
    let mut vault = Vault::open(&scratch.0, DOMAIN).unwrap();
    vault
        .set_protection(Protection::ConfirmedUnprotected)
        .unwrap();
    store(&mut vault);
    let previous = fs::read(active_file(&vault)).unwrap();
    let pending = vault.directory.join(STAGED);
    fs::write(&pending, b"interrupted replacement").unwrap();
    drop(vault);
    let mut vault = Vault::open(&scratch.0, DOMAIN).unwrap();
    expect_token(&mut vault);
    assert!(!pending.exists());

    // An occupied pending path causes staging to fail before the old slot
    // is replaced. The destination is a directory, so removal also fails.
    fs::create_dir(&pending).unwrap();
    assert_eq!(
        vault.set_protection(Protection::PinProtected { new_pin: PIN }),
        Err(VaultError::StorageUnavailable)
    );
    assert_eq!(vault.status().mode, StorageMode::Unprotected);
    expect_token(&mut vault);
    fs::remove_dir(&pending).unwrap();
    protected(&mut vault, PIN);
    fs::create_dir(&pending).unwrap();
    assert_eq!(
        vault.set_protection(Protection::PinProtected { new_pin: NEW_PIN }),
        Err(VaultError::StorageUnavailable)
    );
    fs::remove_dir(&pending).unwrap();

    // Power loss after the commit rename and before the old-slot removal.
    let predecessor = vault.directory.join(SLOTS[vault.active.unwrap().0 ^ 1]);
    fs::write(&predecessor, previous).unwrap();
    drop(vault);
    let mut vault = Vault::open(&scratch.0, DOMAIN).unwrap();
    assert!(vault.status().cleanup_pending);
    assert_eq!(vault.unlock(NEW_PIN), Err(VaultError::AuthenticationFailed));
    vault.unlock(PIN).unwrap();
    assert!(!predecessor.exists());
    expect_token(&mut vault);
}

#[test]
fn profile_isolation_session_only_and_removal() {
    let scratch = Scratch::new();
    let mut vault = Vault::open(&scratch.0, DOMAIN).unwrap();
    vault
        .set_protection(Protection::ConfirmedUnprotected)
        .unwrap();
    store(&mut vault);
    let mut other = Vault::open(&scratch.0, "microsoft/another_profile").unwrap();
    other.set_protection(Protection::SessionOnly).unwrap();
    assert!(other.get(KEY).unwrap().is_none());
    store(&mut other);
    expect_token(&mut other);
    assert!(!other.directory.exists());
    other.lock();
    assert!(other.get(KEY).unwrap().is_none());
    expect_token(&mut vault);
    assert_eq!(
        format!("{:?}", vault.get(KEY).unwrap().unwrap()),
        "Secret([redacted])"
    );
    vault.remove(KEY).unwrap();
    assert!(Vault::open(&scratch.0, DOMAIN)
        .unwrap()
        .get(KEY)
        .unwrap()
        .is_none());
    store(&mut vault);
    vault.set_protection(Protection::SessionOnly).unwrap();
    expect_token(&mut vault);
    assert!(Vault::open(&scratch.0, DOMAIN)
        .unwrap()
        .get(KEY)
        .unwrap()
        .is_none());
}

#[test]
fn a_locked_account_can_be_forgotten_without_disclosing_its_secret() {
    let scratch = Scratch::new();
    let mut vault = Vault::open(&scratch.0, DOMAIN).unwrap();
    protected(&mut vault, PIN);
    store(&mut vault);
    let credential = active_file(&vault);
    vault.lock();
    vault.erase_domain().unwrap();
    assert!(!credential.exists());
    let mut reopened = Vault::open(&scratch.0, DOMAIN).unwrap();
    assert_eq!(reopened.status().mode, StorageMode::SessionOnly);
    assert!(reopened.get(KEY).unwrap().is_none());
}
