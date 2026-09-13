//! Discussion lists return the complete native records in their stored order.
use super::scoped_list::scoped_list;

scoped_list!(ListThreads, "list-threads", Thread, list_threads);
scoped_list!(ListMessages, "list-messages", Message, list_messages);
