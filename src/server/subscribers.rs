//! Newsletter subscription with double opt-in. `subscribe` records a pending
//! (unconfirmed) row and emails a confirmation link; `confirm_subscription`
//! flips the `confirmed` flag when that link is followed.

use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::server::{looks_like_email, sfe, DbExtension, MailExtension};

#[post("/api/subscribe", db: DbExtension, mail: MailExtension)]
pub async fn subscribe(email: String) -> Result<()> {
    let email = email.trim().to_lowercase();
    if !looks_like_email(&email) {
        return Err(ServerFnError::new("Please enter a valid email address.").into());
    }

    // Upsert the subscriber row (idempotent on the unique email).
    sqlx::query("INSERT OR IGNORE INTO subscribers (email) VALUES (?)")
        .bind(&email)
        .execute(&db.0)
        .await
        .map_err(sfe)?;

    let (sub_id, confirmed): (i64, i64) =
        sqlx::query_as("SELECT id, confirmed FROM subscribers WHERE email = ?")
            .bind(&email)
            .fetch_one(&db.0)
            .await
            .map_err(sfe)?;

    // Already confirmed — nothing to do, and we don't re-mail.
    if confirmed != 0 {
        return Ok(());
    }

    // Anti-abuse: this endpoint is anonymous, so without a gate anyone could call
    // it on a loop to relay confirmation mail to an arbitrary address (an email
    // bomb). If a pending token was issued for this subscriber within the cooldown
    // window, silently succeed without minting a new token or re-mailing. The
    // caller can't tell a fresh row from a throttled one, so this leaks nothing.
    let recently_mailed: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM subscriber_tokens \
         WHERE subscriber_id = ? AND created_at >= datetime('now', ?) LIMIT 1",
    )
    .bind(sub_id)
    .bind(RESEND_COOLDOWN)
    .fetch_optional(&db.0)
    .await
    .map_err(sfe)?;
    if recently_mailed.is_some() {
        return Ok(());
    }

    // Issue a fresh token (SQLite's randomblob avoids pulling in a rand crate),
    // replacing any earlier pending token for this subscriber. The delete+insert
    // run in one transaction so a concurrent caller can't observe (or mail) a
    // half-rotated state where the old token is gone but the new one isn't in yet.
    let token: String = sqlx::query_scalar("SELECT lower(hex(randomblob(16)))")
        .fetch_one(&db.0)
        .await
        .map_err(sfe)?;
    let mut tx = db.0.begin().await.map_err(sfe)?;
    sqlx::query("DELETE FROM subscriber_tokens WHERE subscriber_id = ?")
        .bind(sub_id)
        .execute(&mut *tx)
        .await
        .map_err(sfe)?;
    sqlx::query("INSERT INTO subscriber_tokens (token, subscriber_id) VALUES (?, ?)")
        .bind(&token)
        .bind(sub_id)
        .execute(&mut *tx)
        .await
        .map_err(sfe)?;
    tx.commit().await.map_err(sfe)?;

    // Send the confirmation email. A flaky mailer must not fail the request —
    // mirror arium's auth flows and just log on error.
    let link = format!("{}/subscribe/confirm?token={token}", mail.0.base_url());
    let subject = "Confirm your subscription";
    let text = format!(
        "Thanks for subscribing!\n\nPlease confirm your email by visiting:\n{link}\n\n\
         If you didn't request this, you can ignore this message."
    );
    let html = format!(
        "<p>Thanks for subscribing!</p>\
         <p>Please confirm your email by clicking the link below:</p>\
         <p><a href=\"{link}\">Confirm subscription</a></p>\
         <p style=\"color:#888\">If you didn't request this, you can ignore this message.</p>"
    );
    if let Err(err) = mail.0.send(&email, subject, &text, Some(&html)).await {
        eprintln!("[mail] WARN: failed to send subscription confirmation: {err}");
    }

    Ok(())
}

/// How long a subscription-confirmation token stays valid. Past this window a
/// token is treated as expired (`confirm_subscription` returns `false`), so a
/// link that leaks or sits unused can't be redeemed indefinitely.
#[cfg(feature = "server")]
const TOKEN_TTL: &str = "-7 days";

/// Minimum gap between confirmation-email re-sends for one (unconfirmed)
/// subscriber. A second `subscribe` within this window is accepted but neither
/// re-tokenizes nor re-mails — the throttle that turns the anonymous endpoint
/// from an email-relay into a no-op under repeat calls.
#[cfg(feature = "server")]
const RESEND_COOLDOWN: &str = "-5 minutes";

/// Consume a confirmation token, marking the subscriber confirmed. Returns
/// `true` when an unexpired token matched, `false` when it was unknown or older
/// than [`TOKEN_TTL`].
#[post("/api/subscribe/confirm", db: DbExtension)]
pub async fn confirm_subscription(token: String) -> Result<bool> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Ok(false);
    }

    let sub_id: Option<i64> = sqlx::query_scalar(
        "SELECT subscriber_id FROM subscriber_tokens \
         WHERE token = ? AND created_at >= datetime('now', ?)",
    )
    .bind(&token)
    .bind(TOKEN_TTL)
    .fetch_optional(&db.0)
    .await
    .map_err(sfe)?;

    let Some(sub_id) = sub_id else {
        return Ok(false);
    };

    // Flip the flag and consume every token for this subscriber in one
    // transaction: a crash between the two used to leave a confirmed subscriber
    // with a live token still redeemable.
    let mut tx = db.0.begin().await.map_err(sfe)?;
    sqlx::query("UPDATE subscribers SET confirmed = 1 WHERE id = ?")
        .bind(sub_id)
        .execute(&mut *tx)
        .await
        .map_err(sfe)?;
    sqlx::query("DELETE FROM subscriber_tokens WHERE subscriber_id = ?")
        .bind(sub_id)
        .execute(&mut *tx)
        .await
        .map_err(sfe)?;
    tx.commit().await.map_err(sfe)?;

    Ok(true)
}
