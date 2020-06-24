use libsip::{Header, SipMessage};

pub fn set_to_branch(msg: &mut SipMessage, branch: &str) {
    if let SipMessage::Response {
        ref mut headers, ..
    } = msg
    {
        let h = headers
            .0
            .iter_mut()
            .find(|h| if let Header::To(_) = h { true } else { false });
        if let Some(Header::To(h)) = h {
            if h.params.contains_key("branch") {
                h.params.remove("branch");
            }
            h.params.insert("branch".to_string(), branch.to_string());
        }
    }
}
