//! Auth pages — thin wrappers over arium's drop-in UI, placed in FullBleedLayout
//! per the PLAN's /login, /register, /forgot-password, /auth/reset, /auth/verify.
//!
//! `LoginPanel` carries its own sign-in / sign-up toggle, so /login and /register
//! render the same panel; the route split is for clean URLs and deep links.

use dioxus::prelude::*;

use arium_dioxus::server::*;
use arium_dioxus::ui::{
    use_oauth_providers, use_permissions, ForgotPassword, LoginPanel, LoginSubmit, ResetPassword,
    SubmitKind, VerifyEmail,
};
use arium_dioxus::{friendly_server_error, LoginOutcome};

use crate::layouts::FullBleedLayout;
use crate::Route;

#[component]
fn AuthPanel() -> Element {
    let perms = use_permissions();
    let providers = use_oauth_providers();
    let mut auth_error = use_signal(String::new);
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
                Ok(LoginOutcome::MfaRequired) => auth_error
                    .set("Two-factor required — finish sign-in from the account page.".into()),
                Err(e) => auth_error.set(friendly_server_error(e)),
            }
        });
    };

    rsx! {
        FullBleedLayout {
            div { class: "flex min-h-screen items-center justify-center p-4",
                div { class: "w-full max-w-md",
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
