//! The adapter's own files beside each other: the persisted update offset
//! the poll intake acknowledges by, and the webhook intake's secret token.
//!
//! The offset, per decision 0014: the state file holds the next offset to
//! send — the highest acknowledged update id plus one — written after a
//! batch's messages are ingested. A crash between ingest and write
//! redelivers, and the duplicates are the accepted outcome; the file makes
//! that redelivery window explicit and testable. The webhook intake never
//! touches it: there the response code is the acknowledgement.
//!
//! The secret token (2026-08-29) is the adapter's own — generated here at
//! the first webhook start, kept in a file beside the offset with owner-only
//! permissions, and reused thereafter. A generated secret that cannot be
//! kept refuses the start instead of running on an unpersisted one: the
//! registered secret and the kept one must be the same across a restart, or
//! every delivery the platform is still retrying is discarded at the door.
//! No human carries it: it enters no configuration, no log line and no error
//! text, which the type below keeps true by having no display form and a
//! redacting debug one.

use std::io::Read;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The stored offset, or `None` when the file is absent, empty or
/// malformed — all three read as absent by decision, with the two
/// abnormal cases logged so an operator can see the redelivery coming.
pub(crate) fn read(path: &Path) -> Option<i64> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(%error, "the state file did not read; treating it as absent");
            return None;
        }
    };
    let trimmed = content.trim();
    if trimmed.is_empty() {
        tracing::warn!("the state file is empty; treating it as absent");
        return None;
    }
    if let Ok(offset) = trimmed.parse() {
        Some(offset)
    } else {
        tracing::warn!("the state file is malformed; treating it as absent");
        None
    }
}

/// The suffix a sidecar carries while it is being written, before it is
/// renamed over the file it replaces. One name for both files this module
/// persists, because both take the same write-then-rename.
const SIDECAR_SUFFIX: &str = ".next";

/// The sidecar one file is written through.
fn sidecar_of(path: &Path) -> PathBuf {
    let mut sidecar = path.as_os_str().to_owned();
    sidecar.push(SIDECAR_SUFFIX);
    PathBuf::from(sidecar)
}

/// Persist the next offset: written whole to a sibling file, then renamed
/// over the state file. The rename keeps a process crash mid-write from
/// leaving a torn file — the old offset survives, and a torn file would
/// read as absent and widen the redelivery window for no reason. Power
/// loss is not defended: nothing is fsynced, because the worst a lost
/// write yields is the accepted duplicate window, the exact outcome an
/// fsync would be paying to avoid.
pub(crate) fn write(path: &Path, next_offset: i64) -> std::io::Result<()> {
    let sidecar = sidecar_of(path);
    std::fs::write(&sidecar, next_offset.to_string())?;
    std::fs::rename(&sidecar, path)
}

/// The characters a generated secret is drawn from — exactly the platform's
/// permitted alphabet for the token, and exactly sixty-four of them, so one
/// byte's low six bits index it without bias.
const SECRET_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";

/// How many characters a generated secret carries: the platform accepts one
/// to two hundred fifty-six, and sixty-four of this alphabet is three
/// hundred eighty-four bits of randomness, far past what anyone could search.
const SECRET_LENGTH: usize = 64;

/// The mode the secret file is kept at: readable and writable by its owner,
/// by nobody else.
const OWNER_ONLY: u32 = 0o600;

/// The mode bits [`OWNER_ONLY`] is compared against, so a set-group-id or
/// sticky bit elsewhere in the mode word does not read as a permission.
const PERMISSION_BITS: u32 = 0o777;

/// Where the operating system's randomness is read from.
const RANDOM_SOURCE: &str = "/dev/urandom";

/// The suffix the secret file carries beside the state file.
const SECRET_SUFFIX: &str = ".secret";

/// The webhook's secret token: the value the registration hands the platform
/// and every delivery must carry back. It has no display form and its debug
/// form is redacted, so no format string anywhere can spill it; the two
/// places that genuinely need the characters ask for them by name.
pub(crate) struct SecretToken(String);

impl SecretToken {
    /// The token's characters — the registration's parameter and nothing
    /// else. Named so every use of the raw value is greppable.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }

    /// Whether an offered token is this one, compared over its whole length
    /// in time that does not depend on how far the two agree: the door
    /// answers strangers, and a comparison that stopped at the first
    /// difference would answer them with a measurement.
    ///
    /// A length that differs returns at once, and that early return leaks
    /// nothing: every token this adapter generates is exactly
    /// [`SECRET_LENGTH`] characters, a constant of the source, so the length
    /// is public and the timing of a length mismatch tells an observer only
    /// what the source already says.
    pub(crate) fn matches(&self, offered: &str) -> bool {
        let known = self.0.as_bytes();
        let offered = offered.as_bytes();
        if known.len() != offered.len() {
            return false;
        }
        known
            .iter()
            .zip(offered)
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            })
            == 0
    }

    /// The text with any occurrence of the token replaced — the same second
    /// protection the client applies to the bot token, for the one error
    /// text that carries a platform answer about a registration.
    pub(crate) fn scrubbed(&self, text: &str) -> String {
        text.replace(&self.0, "[redacted]")
    }
}

impl std::fmt::Debug for SecretToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Where the secret lives: beside the state file, under its name plus
/// [`SECRET_SUFFIX`] — one path derived from one configured path, so a
/// deployment names its state directory once.
pub(crate) fn secret_path(state_file: &Path) -> PathBuf {
    let mut path = state_file.as_os_str().to_owned();
    path.push(SECRET_SUFFIX);
    PathBuf::from(path)
}

/// The webhook's secret: the kept one where the file holds a usable token,
/// a freshly generated and persisted one otherwise.
///
/// A kept token that cannot be used — unreadable, empty, or outside the
/// permitted alphabet — is replaced with a structural warning, and a file
/// found at wider permissions is corrected on read. Regeneration always
/// converges, because every webhook start registers the token this returns.
///
/// # Errors
///
/// The two failures that refuse the start. The operating system's randomness
/// could not be read, so there is no token to register and no door to open;
/// or the generated token did not persist, which is refused instead of
/// carried: a start that registered a secret it could not write would come
/// back after any restart with a different one, and every delivery the
/// platform was still retrying under the old secret would be discarded at
/// the door as a stranger's.
pub(crate) fn webhook_secret(state_file: &Path) -> std::io::Result<SecretToken> {
    let path = secret_path(state_file);
    if let Some(kept) = read_secret(&path) {
        return Ok(kept);
    }
    let fresh = generate_secret()?;
    write_secret(&path, &fresh)?;
    Ok(fresh)
}

/// The kept secret, or `None` when the file holds nothing usable — every
/// abnormal case warned about, so an operator sees the replacement coming.
fn read_secret(path: &Path) -> Option<SecretToken> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(%error, "the webhook secret did not read; a fresh one is generated");
            return None;
        }
    };
    let kept = content.trim();
    if kept.is_empty() {
        tracing::warn!("the webhook secret file is empty; a fresh one is generated");
        return None;
    }
    if kept.len() != SECRET_LENGTH || !kept.bytes().all(|byte| SECRET_ALPHABET.contains(&byte)) {
        tracing::warn!(
            "the kept webhook secret is not one this adapter can use; a fresh one is generated"
        );
        return None;
    }
    correct_permissions(path);
    Some(SecretToken(kept.to_owned()))
}

/// Bring a kept secret's file back to owner-only, so a file that was created
/// or copied wider does not stay that way.
fn correct_permissions(path: &Path) {
    let mode = match std::fs::metadata(path) {
        Ok(metadata) => metadata.permissions().mode(),
        Err(error) => {
            tracing::warn!(%error, "the webhook secret file's permissions did not read");
            return;
        }
    };
    if mode & PERMISSION_BITS == OWNER_ONLY {
        return;
    }
    match std::fs::set_permissions(path, std::fs::Permissions::from_mode(OWNER_ONLY)) {
        Ok(()) => tracing::warn!("the webhook secret file was readable past its owner; corrected"),
        Err(error) => {
            tracing::warn!(%error, "the webhook secret file's permissions did not correct");
        }
    }
}

/// A fresh token: [`SECRET_LENGTH`] characters of [`SECRET_ALPHABET`], each
/// indexed by the low six bits of one byte of the operating system's
/// randomness — an unbiased draw, because the alphabet is exactly as long as
/// six bits can name.
fn generate_secret() -> std::io::Result<SecretToken> {
    let mut bytes = [0_u8; SECRET_LENGTH];
    std::fs::File::open(RANDOM_SOURCE)?.read_exact(&mut bytes)?;
    let token = bytes
        .iter()
        .map(|byte| char::from(SECRET_ALPHABET[usize::from(byte & 0x3F)]))
        .collect();
    Ok(SecretToken(token))
}

/// Persist the secret owner-only, written whole to a sidecar and renamed
/// over the secret file — the same write-then-rename the offset beside it
/// takes, and for the same reason: a process that died mid-write would
/// otherwise leave half a token, which the next start would read as
/// unusable and replace, breaking delivery authentication for every retry
/// still carrying the old one.
///
/// The sidecar is created at the mode and has the mode set again before the
/// rename, so an existing wider sidecar is narrowed instead of inherited and
/// the secret is never readable past its owner for even one moment. The
/// rename carries the mode with it.
///
/// The content is synced before the rename, which the offset beside it does
/// not do: a lost offset costs the duplicate window that decision already
/// accepts, while a lost secret costs the authentication of every delivery
/// the platform is still retrying.
fn write_secret(path: &Path, secret: &SecretToken) -> std::io::Result<()> {
    use std::io::Write;

    let sidecar = sidecar_of(path);
    let mut file = std::fs::File::options()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(OWNER_ONLY)
        .open(&sidecar)?;
    file.write_all(secret.expose().as_bytes())?;
    file.sync_all()?;
    std::fs::set_permissions(&sidecar, std::fs::Permissions::from_mode(OWNER_ONLY))?;
    std::fs::rename(&sidecar, path)
}
