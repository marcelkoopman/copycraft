#![cfg(target_os = "macos")]

use objc2_foundation::{NSMutableURLRequest, NSString, NSURL, NSURLConnection};

const DOCUMENT_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";

/// GET without cookies. Returns the body when the load succeeds.
pub fn get(url: &str) -> Option<Vec<u8>> {
    load(url, "Copycraft", None)
}

/// GET a preview image with a browser user agent and no byte cap.
pub fn get_asset(url: &str) -> Option<Vec<u8>> {
    load(url, DOCUMENT_AGENT, None)
}

/// GET the start of a page. Asks for the first 256KB so the HTML head can be
/// read without downloading the rest of the document.
pub fn get_document(url: &str) -> Option<Vec<u8>> {
    const MAX: usize = 256 * 1024;
    let ranged = load(url, DOCUMENT_AGENT, Some("bytes=0-262143"));
    let bytes = ranged
        .filter(|bytes| bytes.len() > 64)
        .or_else(|| load(url, DOCUMENT_AGENT, None))?;
    let end = bytes.len().min(MAX);
    Some(bytes[..end].to_vec())
}

fn load(url: &str, agent: &str, range: Option<&str>) -> Option<Vec<u8>> {
    let nsurl = NSURL::URLWithString(&NSString::from_str(url))?;
    let request = NSMutableURLRequest::requestWithURL(&nsurl);
    request.setHTTPShouldHandleCookies(false);
    request.setTimeoutInterval(8.0);
    request.setValue_forHTTPHeaderField(
        Some(&NSString::from_str(agent)),
        &NSString::from_str("User-Agent"),
    );
    if let Some(range) = range {
        request.setValue_forHTTPHeaderField(
            Some(&NSString::from_str(range)),
            &NSString::from_str("Range"),
        );
    }
    let data = send(&request).ok()?;
    Some(data.to_vec())
}

#[allow(deprecated)]
fn send(
    request: &NSMutableURLRequest,
) -> Result<
    objc2::rc::Retained<objc2_foundation::NSData>,
    objc2::rc::Retained<objc2_foundation::NSError>,
> {
    NSURLConnection::sendSynchronousRequest_returningResponse_error(request, None)
}
