//! Follows the built spec's handles and verifies ordinary production
//! code.

use std::time::{Duration, SystemTime};

use provenance_sdk::verify;
use rust_sdk_example::provenance_spec::share_links;
use rust_sdk_example::share_links::create_share_link;

const THIRTY_DAYS: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[test]
fn share_link_expiry() -> anyhow::Result<()> {
    let document = share_links()?;
    let expiry = document.handles().requirement("sharing")?.rule("expiry")?;
    verify(&expiry, "share-link-expiry", || {
        let now = SystemTime::now();
        let link = create_share_link(now);
        if link.expires_at > now + THIRTY_DAYS {
            return Err("share link expires after the 30-day limit".to_string());
        }
        Ok(())
    })
}
