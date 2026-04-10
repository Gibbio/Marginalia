use crate::frontend::{BackendCapabilities, FrontendEvent, FrontendRequest, FrontendResponse};

pub trait FrontendGateway {
    fn capabilities(&self) -> BackendCapabilities;
    fn execute_command(&mut self, request: FrontendRequest) -> FrontendResponse;
    fn execute_query(&mut self, request: FrontendRequest) -> FrontendResponse;
    fn recent_events(&self, limit: usize) -> Vec<FrontendEvent>;
}
