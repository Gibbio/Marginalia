#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendQueryName {
    GetAppSnapshot,
    GetBackendCapabilities,
    GetDocumentView,
    GetDoctorReport,
    GetSessionSnapshot,
    ListNotes,
    ListDocuments,
    SearchDocuments,
    SearchNotes,
}
