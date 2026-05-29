//! Auth pages — thin wrappers over arium's drop-in UI: /login, /register,
//! /forgot-password, /auth/reset, /auth/verify (FullBleedLayout), plus /account
//! (HolyGrailLayout).
//!
//! `LoginPanel` carries its own sign-in / sign-up toggle, so /login and /register
//! render the same panel; the route split is for clean URLs and deep links.

use dioxus::prelude::*;

use arium_dioxus::server::*;
use arium_dioxus::ui::{
    use_oauth_providers, use_permissions, AccountSettings, ForgotPassword, LoginPanel, LoginSubmit,
    MfaChallenge, MfaSetup, RequireAuth, ResetPassword, SubmitKind, VerifyEmail,
};
use arium_dioxus::{friendly_server_error, LoginOutcome};

use crate::layouts::{FullBleedLayout, HolyGrailLayout};
use crate::Route;

#[component]
fn AuthPanel() -> Element {
    let perms = use_permissions();
    let providers = use_oauth_providers();
    let mut auth_error = use_signal(String::new);
    // When a password login returns `MfaRequired`, the session is
    // half-authenticated and we swap the panel for the TOTP challenge.
    let mut mfa_pending = use_signal(|| false);
    let nav = navigator();

    let on_submit = move |submission: LoginSubmit| {
        auth_error.set(String::new());
        let LoginSubmit {
            kind,
            email,
            password,
            remember,
        } = submission;
        spawn(async move {
            let result = match kind {
                SubmitKind::SignIn => login_with_password(email, password, remember).await,
                SubmitKind::SignUp => register_with_password(email, password).await,
            };
            match result {
                Ok(LoginOutcome::LoggedIn) => {
                    perms.refresh();
                    nav.push(Route::HomePage);
                }
                Ok(LoginOutcome::EmailUnverified) => {
                    auth_error.set("Check your inbox to verify your email, then sign in.".into())
                }
                Ok(LoginOutcome::MfaRequired) => mfa_pending.set(true),
                Err(e) => auth_error.set(friendly_server_error(e)),
            }
        });
    };

    rsx! {
        FullBleedLayout {
            div { class: "flex min-h-screen items-center justify-center p-4",
                div { class: "w-full max-w-md",
                    if mfa_pending() {
                        MfaChallenge {
                            on_logged_in: move |_| {
                                perms.refresh();
                                nav.push(Route::HomePage);
                            },
                            on_cancel: move |_| {
                                spawn(async move {
                                    let _ = cancel_mfa_challenge().await;
                                    mfa_pending.set(false);
                                });
                            },
                        }
                    } else {
                        LoginPanel {
                            providers: providers.clone(),
                            forgot_href: "/forgot-password",
                            error: {
                                let e = auth_error();
                                if e.is_empty() { None } else { Some(e) }
                            },
                            on_submit,
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub fn LoginPage() -> Element {
    rsx! { AuthPanel {} }
}

#[component]
pub fn RegisterPage() -> Element {
    rsx! { AuthPanel {} }
}

#[component]
pub fn ForgotPasswordPage() -> Element {
    rsx! {
        FullBleedLayout {
            div { class: "flex min-h-screen items-center justify-center p-4",
                div { class: "w-full max-w-md", ForgotPassword {} }
            }
        }
    }
}

#[component]
pub fn ResetPasswordPage(token: String) -> Element {
    rsx! {
        FullBleedLayout {
            div { class: "flex min-h-screen items-center justify-center p-4",
                div { class: "w-full max-w-md", ResetPassword { token } }
            }
        }
    }
}

#[component]
pub fn VerifyEmailPage(token: String) -> Element {
    rsx! {
        FullBleedLayout {
            div { class: "flex min-h-screen items-center justify-center p-4",
                div { class: "w-full max-w-md", VerifyEmail { token } }
            }
        }
    }
}

/// Authenticated self-service account page: arium's `AccountSettings` (display
/// name, password change, linked providers, delete account) plus the two-factor
/// `MfaSetup` panel, in site chrome. Unauthenticated visitors are redirected to
/// the login page.
#[component]
pub fn AccountPage() -> Element {
    rsx! {
        HolyGrailLayout {
            RequireAuth { redirect_to: "/login".to_string(),
                div { class: "mx-auto flex w-full max-w-2xl flex-col gap-6",
                    h1 { class: "text-2xl font-semibold tracking-tight", "Your account" }
                    AccountSettings {}
                    MfaSetup { embedded: true }
                }
            }
        }
    }
}
