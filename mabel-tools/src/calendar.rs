use std::sync::Arc;

use google_calendar3::{api::Event, CalendarHub};
use rmcp::{model::*, tool, tool_handler, tool_router};
use serde_json::json;
use tokio::sync::Mutex;
