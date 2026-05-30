//! Newsletter subscription with double opt-in. `subscribe` records a pending
//! (unconfirmed) row and emails a confirmation link; `confirm_subscription`
//! flips the `confirmed` flag when that link is followed.

use dioxus::prelude::*;

#[cfg(feature = "server")]
use crate::db::subscribers::{
    confirm_subscriber_db, get_subscriber_by_email_db, has_recent_token_db,
    rotate_subscriber_token_db, upsert_subscriber_db,
};
#[cfg(feature = "server")]
use crate::server::{looks_like_email, sfe, DbExtension, MailExtension};

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

#[post("/api/subscribe", db: DbExtension, mail: MailExtension)]
pub async fn subscribe(email: String) -> Result<()> {
    let email = email.trim().to_lowercase();
    if !looks_like_email(&email) {
        return Err(ServerFnError::new("Please enter a valid email address.").into());
    }

    upsert_subscriber_db(&db.0, &email).await.map_err(sfe)?;

    let (sub_id, confirmed) = get_subscriber_by_email_db(&db.0, &email)
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
    if has_recent_token_db(&db.0, sub_id, RESEND_COOLDOWN)
        .await
        .map_err(sfe)?
    {
        return Ok(());
    }

    let token = rotate_subscriber_token_db(&db.0, sub_id)
        .await
        .map_err(sfe)?;

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
        tracing::warn!(target: "mail", "failed to send subscription confirmation: {err}");
    }

    Ok(())
}

/// Consume a confirmation token, marking the subscriber confirmed. Returns
/// `true` when an unexpired token matched, `false` when it was unknown or older
/// than [`TOKEN_TTL`].
#[post("/api/subscribe/confirm", db: DbExtension)]
pub async fn confirm_subscription(token: String) -> Result<bool> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Ok(false);
    }
    Ok(confirm_subscriber_db(&db.0, &token, TOKEN_TTL)
        .await
        .map_err(sfe)?)
}
