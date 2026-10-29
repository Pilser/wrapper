//! HTTP request handling, body helpers, multipart and streaming stubs.

pub mod body;
pub mod handler;
pub mod multipart;
pub mod static_assets;
pub mod streaming;

pub use body::{full_response, read_body, text_response};
pub use handler::HttpHandler;
pub use multipart::{boundary_of, MultipartField};
pub use streaming::{collect_streamed, StreamedChunk};
