//! Comment moderation queue: approve / reject / delete each pending comment.

use dioxus::prelude::*;

use crate::components::button::ButtonVariant;
use crate::components::surface::{Badge, Panel, PanelPadding, PanelVariant};
use crate::components::text::PageTitle;
use crate::live::use_admin_live;
use crate::pages::widgets::list_states;
use crate::server::admin::{admin_list_comments, delete_comment, moderate_comment};

use super::{ActionButton, ActionFuture, AdminShell};

#[component]
pub fn AdminComments() -> Element {
    let mut comments = use_resource(move || async move { admin_list_comments(None).await });
    // This page is COMMENTS_MODERATE-gated, so subscribe to the admin stream and
    // refetch the queue whenever a comment is created/moderated/deleted anywhere
    // (a new pending comment, or another admin's action) — no manual refresh.
    let live = use_admin_live(true);
    use_effect(move || {
        let _ = (live.comment_tick)();
        comments.restart();
    });
    rsx! {
        AdminShell { active: "comments".to_string(),
            PageTitle { "Comment moderation" }
            {list_states!(comments, empty: "No comments.", list => rsx! {
                        div { class: "space-y-3",
                            for c in list {
                                Panel { key: "{c.id}", variant: PanelVariant::Outlined, padding: PanelPadding::Md,
                                    div { class: "flex items-center justify-between",
                                        div { class: "text-sm font-medium", "{c.display_name}" }
                                        Badge { "{c.status}" }
                                    }
                                    p { class: "mt-1 text-sm text-white/80", "{c.body}" }
                                    div { class: "mt-2 flex gap-3 text-xs",
                                        {
                                            let cid = c.id;
                                            rsx! {
                                                ActionButton {
                                                    label: "Approve".to_string(),
                                                    on_done: move |_| comments.restart(),
                                                    action: move |_| Box::pin(async move { moderate_comment(cid, "approved".to_string()).await }) as ActionFuture,
                                                }
                                                ActionButton {
                                                    label: "Reject".to_string(),
                                                    on_done: move |_| comments.restart(),
                                                    action: move |_| Box::pin(async move { moderate_comment(cid, "rejected".to_string()).await }) as ActionFuture,
                                                }
                                                ActionButton {
                                                    label: "Delete".to_string(),
                                                    variant: ButtonVariant::Destructive,
                                                    on_done: move |_| comments.restart(),
                                                    action: move |_| Box::pin(async move { delete_comment(cid).await }) as ActionFuture,
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
            })}
        }
    }
}
