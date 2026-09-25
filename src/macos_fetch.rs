#![cfg(target_os = "macos")]

use objc2_foundation::{NSMutableURLRequest, NSString, NSURL, NSURLConnection};

const DOCUMENT_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";

/// GET without cookies. Returns the body when the load succeeds.
pub fn get(url: &str) -> Option<Vec<u8>> {
    load(url, "Copycraft")
}

/// GET a page or its preview image with a browser user agent so the site
/// returns the HTML head instead of a bot challenge.
pub fn get_document(url: &str) -> Option<Vec<u8>> {
    load(url, DOCUMENT_AGENT)
}

fn load(url: &str, agent: &str) -> Option<Vec<u8>> {
    let nsurl = NSURL::URLWithString(&NSString::from_str(url))?;
    let request = NSMutableURLRequest::requestWithURL(&nsurl);
    request.setHTTPShouldHandleCookies(false);
    request.setTimeoutInterval(8.0);
    request.setValue_forHTTPHeaderField(
        Some(&NSString::from_str(agent)),
        &NSString::from_str("User-Agent"),
    );
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
