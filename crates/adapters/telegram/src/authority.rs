//! Authority resolution per decision 0015: the sender's standing translates
//! from the chat's member status — `creator` to admin, `administrator` to
//! moderator, everyone else to member — resolved from a per-chat
//! administrator list and cached with a short time-to-live.
//!
//! A failed list fetch is the caller's transient failure: authority is never
//! silently defaulted into the ledger, so no fallback lives here.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use assistant_core::Authority;

use crate::client::{BotClient, ClientError};

/// How long one chat's fetched administrator list stays authoritative. Long
/// enough to keep a busy group from refetching per message against a
/// rate-limited API, short enough that a promotion takes effect within a minute.
const ADMIN_CACHE_TTL: Duration = Duration::from_mins(1);

/// One chat's cached administrator standings, stamped at fetch.
struct CachedChat {
    fetched: Instant,
    /// Administrators only, by sender id; absence means member.
    standings: HashMap<i64, Authority>,
}

/// The per-chat administrator cache.
pub(crate) struct AdminCache {
    chats: HashMap<i64, CachedChat>,
}

impl AdminCache {
    pub(crate) fn new() -> Self {
        Self {
            chats: HashMap::new(),
        }
    }

    /// One sender's standing in one chat, from the cache or a fresh fetch.
    ///
    /// # Errors
    ///
    /// [`ClientError`] when the list fetch fails; the caller treats that as
    /// the message's own transient failure.
    pub(crate) async fn authority_for(
        &mut self,
        client: &BotClient,
        chat_id: i64,
        sender_id: i64,
    ) -> Result<Authority, ClientError> {
        let fresh_enough = self
            .chats
            .get(&chat_id)
            .is_some_and(|cached| cached.fetched.elapsed() < ADMIN_CACHE_TTL);
        if !fresh_enough {
            let members = client.chat_administrators(chat_id).await?;
            let standings = members
                .iter()
                .filter_map(|member| {
                    let standing = match member.status.as_str() {
                        "creator" => Authority::Admin,
                        "administrator" => Authority::Moderator,
                        // A status outside the two elevated ones carries no
                        // elevated standing; the absence below means member.
                        _ => return None,
                    };
                    Some((member.user.id, standing))
                })
                .collect();
            self.chats.insert(
                chat_id,
                CachedChat {
                    fetched: Instant::now(),
                    standings,
                },
            );
        }
        let standing = self
            .chats
            .get(&chat_id)
            .and_then(|cached| cached.standings.get(&sender_id))
            .copied();
        Ok(standing.unwrap_or(Authority::Member))
    }
}
