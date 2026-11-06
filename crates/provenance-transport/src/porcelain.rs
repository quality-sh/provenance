//! Host adapters for shared Porcelain capabilities.

pub mod api_port;
pub(crate) mod authoring;
mod discussion_port;
mod get_port;
mod host;
mod parent_replacement;
mod search_port;
pub use api_port::HostApiPort;
pub use authoring::{replacement_call, target_call, Action, ActionError, TargetRoute};
pub use discussion_port::HostDiscussionPort;
pub use get_port::HostGetPort;
pub use host::HostPorcelain;
pub use parent_replacement::{ParentReplacement, Replaced, ReplacementError};
pub use search_port::HostSearchPort;

use provenance_contract::Definition;
use provenance_core::NodeType;

impl HostPorcelain<'_> {
    /// The host grants what the get capability reads.
    pub fn get_is_available(&self) -> bool {
        get_port::is_available(self.host)
    }

    /// The host grants what the search capability reads.
    pub fn search_is_available(&self) -> bool {
        search_port::is_available(self.host)
    }

    /// The host grants what the api capability needs.
    pub fn api_is_available(&self) -> bool {
        api_port::is_available(self.host)
    }

    /// The host can register a client thread for review changes.
    pub fn notify_is_available(&self) -> bool {
        self.host.notify_port().is_some() && self.get_is_available()
    }

    /// The host grants what one discussion action needs.
    pub fn discussion_is_available(&self, action: Action) -> bool {
        discussion_port::is_available(self.host, action)
    }

    /// The registered operations that this host can run for one target-first action.
    pub fn executable_target_definitions(
        &self,
        action: Action,
    ) -> Vec<(NodeType, &'static Definition)> {
        self.host.executable_target_definitions(action)
    }
}
