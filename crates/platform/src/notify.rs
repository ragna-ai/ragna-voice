//! User-facing notifications (Notification Center).
//!
//! Permission is asked the first time there is something to say, not at launch: most
//! users never see a notification, because Insertion usually works.

/// Posts a notification. Repeated posts with the same `title` replace each other
/// instead of piling up. Never put a Transcript in it (`AGENTS.md` §2). Returns at once;
/// failures are logged, since the clipboard still holds anything the user needs.
pub fn post(title: &str, body: &str) {
    imp::post(title, body);
}

#[cfg(target_os = "macos")]
mod imp {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNUserNotificationCenter,
    };

    pub fn post(title: &str, body: &str) {
        // Unbundled (`cargo run`), Notification Center throws and aborts the process.
        if crate::app::bundle_id().is_none() {
            log::info!("not bundled: not posting {title:?}");
            return;
        }
        let (title, body) = (title.to_owned(), body.to_owned());
        let center = UNUserNotificationCenter::currentNotificationCenter();
        // Shows the system prompt the first time; afterwards answers at once.
        let handler = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
            if !granted.as_bool() {
                log::info!("notifications are off; not posting {title:?}");
                return;
            }
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(&title));
            content.setBody(&NSString::from_str(&body));
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &NSString::from_str(&title),
                &content,
                None,
            );
            UNUserNotificationCenter::currentNotificationCenter()
                .addNotificationRequest_withCompletionHandler(&request, None);
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &handler,
        );
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn post(title: &str, _body: &str) {
        log::info!("notification: {title}");
    }
}

#[cfg(test)]
mod tests {
    /// Test binaries are not bundled, like `cargo run -p ragna-voice`. Notification
    /// Center throws an Objective-C exception there, which aborts the process.
    #[test]
    fn posting_unbundled_does_not_abort() {
        super::post("RAGNA Voice test", "");
    }
}
