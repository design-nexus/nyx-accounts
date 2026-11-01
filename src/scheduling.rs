//! RFC 6638 scheduling discovery. Credentials stay on the configured DAV origin.
use crate::remote::Remote;
use anyhow::{Context, Result};
use reqwest::Method;
use url::Url;
const C: &str = "urn:ietf:params:xml:ns:caldav";
fn trusted(base: &Url, href: &str) -> Result<Url> {
    let next = base.join(href)?;
    anyhow::ensure!(
        base.scheme() == "https"
            && next.origin() == base.origin()
            && next.username().is_empty()
            && next.password().is_none()
            && next.fragment().is_none(),
        "Untrusted DAV scheduling endpoint"
    );
    Ok(next)
}
fn success_property<'a, 'i>(
    xml: &'a roxmltree::Document<'i>,
    namespace: &str,
    name: &str,
) -> Option<roxmltree::Node<'a, 'i>> {
    xml.descendants().find(|node| {
        node.has_tag_name((namespace, name))
            && node
                .ancestors()
                .find(|p| p.has_tag_name(("DAV:", "propstat")))
                .is_some_and(|p| {
                    p.children()
                        .find(|n| n.has_tag_name(("DAV:", "status")))
                        .and_then(|n| n.text())
                        .is_some_and(|s| s.split_whitespace().nth(1) == Some("200"))
                })
    })
}
fn hrefs(node: roxmltree::Node<'_, '_>) -> Vec<String> {
    node.descendants()
        .filter(|n| n.has_tag_name(("DAV:", "href")))
        .filter_map(|n| n.text().map(str::to_owned))
        .collect()
}
fn valid_address(address: &str) -> bool {
    address.contains('@')
        && !address
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || ";:,?#<>\"".contains(c))
}
fn request(
    organizer: &str,
    from: &str,
    to: &str,
    stamp: &str,
    emails: &[String],
) -> Result<String> {
    anyhow::ensure!(
        valid_address(organizer)
            && !emails.is_empty()
            && emails.len() <= 64
            && emails.iter().all(|e| valid_address(e)),
        "Use valid calendar email addresses"
    );
    for date in [from, to, stamp] {
        anyhow::ensure!(
            date.len() == 16
                && date.as_bytes()[8] == b'T'
                && date.as_bytes()[15] == b'Z'
                && date
                    .bytes()
                    .enumerate()
                    .all(|(i, c)| i == 8 || i == 15 || c.is_ascii_digit()),
            "Invalid scheduling time"
        );
    }
    anyhow::ensure!(from < to, "Invalid free/busy range");
    let mut payload = format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Nyx//Online Accounts//EN\r\nMETHOD:REQUEST\r\nBEGIN:VFREEBUSY\r\nUID:{}\r\nDTSTAMP:{stamp}\r\nDTSTART:{from}\r\nDTEND:{to}\r\nORGANIZER:mailto:{organizer}\r\n",
        uuid::Uuid::new_v4()
    );
    for email in emails {
        payload.push_str(&format!("ATTENDEE:mailto:{email}\r\n"));
    }
    payload.push_str("END:VFREEBUSY\r\nEND:VCALENDAR\r\n");
    Ok(payload)
}
fn responses(text: &str, emails: &[String]) -> Result<Vec<(String, String)>> {
    let xml = roxmltree::Document::parse(text)?;
    anyhow::ensure!(
        xml.root_element().has_tag_name((C, "schedule-response")),
        "Invalid scheduling response"
    );
    let mut results = Vec::new();
    for response in xml
        .root_element()
        .children()
        .filter(|n| n.has_tag_name((C, "response")))
    {
        let recipient = response
            .children()
            .find(|n| n.has_tag_name((C, "recipient")))
            .and_then(|n| hrefs(n).into_iter().next())
            .context("Scheduling response has no recipient")?;
        let email = recipient
            .strip_prefix("mailto:")
            .context("Unsupported scheduling recipient")?;
        let Some(requested) = emails
            .iter()
            .find(|requested| requested.eq_ignore_ascii_case(email))
        else {
            anyhow::bail!("Unexpected scheduling recipient")
        };
        let status = response
            .children()
            .find(|n| n.has_tag_name((C, "request-status")))
            .and_then(|n| n.text())
            .unwrap_or("");
        anyhow::ensure!(
            status.starts_with("2."),
            "Free/busy is unavailable for a requested attendee"
        );
        let calendar = response
            .children()
            .find(|n| n.has_tag_name((C, "calendar-data")))
            .and_then(|n| n.text())
            .context("Scheduling response omitted availability data")?;
        anyhow::ensure!(
            !results.iter().any(|(e, _)| e == requested),
            "Duplicate scheduling recipient"
        );
        results.push((requested.clone(), calendar.to_owned()));
    }
    anyhow::ensure!(
        results.len() == emails.len(),
        "Scheduling response omitted a requested attendee"
    );
    Ok(results)
}
impl Remote {
    fn schedule_properties(&self, origin: &Url, start: &Url) -> Result<(String, Url)> {
        let mut current = start.clone();
        for _ in 0..5 {
            let response=self.request(Method::from_bytes(b"PROPFIND")?,&current).header("Depth","0").header("Content-Type","application/xml").body(r#"<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:current-user-principal/><c:schedule-outbox-URL/><c:calendar-user-address-set/></d:prop></d:propfind>"#).send().context("Scheduling discovery failed")?;
            if response.status().is_redirection() {
                current = trusted(
                    origin,
                    current
                        .join(
                            response
                                .headers()
                                .get("location")
                                .context("DAV redirect omitted its location")?
                                .to_str()?,
                        )?
                        .as_str(),
                )?;
                continue;
            }
            return Ok((self.text(response)?, current));
        }
        anyhow::bail!("Scheduling discovery exceeded limit")
    }
    pub fn dav_supports_scheduling(&self, endpoint: &str) -> Result<bool> {
        let origin = Url::parse(endpoint)?;
        trusted(&origin, endpoint)?;
        let mut current = origin.clone();
        for _ in 0..5 {
            let response = self
                .request(Method::OPTIONS, &current)
                .send()
                .context("Scheduling capability check failed")?;
            if response.status().is_redirection() {
                current = trusted(
                    &origin,
                    current
                        .join(
                            response
                                .headers()
                                .get("location")
                                .context("DAV redirect omitted its location")?
                                .to_str()?,
                        )?
                        .as_str(),
                )?;
                continue;
            }
            self.record_http_status(response.status());
            anyhow::ensure!(
                response.status().is_success(),
                "Scheduling capability check was refused"
            );
            return Ok(response
                .headers()
                .get_all("DAV")
                .iter()
                .filter_map(|h| h.to_str().ok())
                .flat_map(|h| h.split(','))
                .any(|value| value.trim() == "calendar-auto-schedule"));
        }
        anyhow::bail!("Scheduling capability discovery exceeded limit")
    }
    pub fn dav_free_busy(
        &self,
        endpoint: &str,
        from: &str,
        to: &str,
        stamp: &str,
        emails: &[String],
    ) -> Result<Vec<(String, String)>> {
        anyhow::ensure!(
            self.dav_supports_scheduling(endpoint)?,
            "This DAV server does not advertise automatic scheduling"
        );
        let origin = Url::parse(endpoint)?;
        let (text, base) = self.schedule_properties(&origin, &origin)?;
        let xml = roxmltree::Document::parse(&text)?;
        let principal = success_property(&xml, "DAV:", "current-user-principal")
            .and_then(|n| hrefs(n).into_iter().next())
            .context("DAV server did not identify its calendar principal")?;
        let principal = trusted(&origin, base.join(&principal)?.as_str())?;
        let (text, base) = self.schedule_properties(&origin, &principal)?;
        let xml = roxmltree::Document::parse(&text)?;
        let outbox = success_property(&xml, C, "schedule-outbox-URL")
            .and_then(|n| hrefs(n).into_iter().next())
            .context("This account has no scheduling outbox")?;
        let organizer = success_property(&xml, C, "calendar-user-address-set")
            .into_iter()
            .flat_map(hrefs)
            .find_map(|value| {
                value
                    .strip_prefix("mailto:")
                    .filter(|email| valid_address(email))
                    .map(str::to_owned)
            })
            .context("Calendar principal has no supported email address")?;
        let target = trusted(&origin, base.join(&outbox)?.as_str())?;
        let payload = request(&organizer, from, to, stamp, emails)?;
        let text = self.text(
            self.request(Method::POST, &target)
                .header("Content-Type", "text/calendar; charset=utf-8")
                .body(payload)
                .send()
                .context("Free/busy request failed")?,
        )?;
        responses(&text, emails)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scheduling_discovery_refuses_cross_origin_and_failed_properties() {
        let origin = Url::parse("https://dav.example.test/caldav/").unwrap();
        assert!(trusted(&origin, "https://other.example.test/outbox").is_err());
        assert!(trusted(&origin, "http://dav.example.test/outbox").is_err());
        let xml=roxmltree::Document::parse(r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response><d:propstat><d:prop><c:schedule-outbox-URL><d:href>/bad</d:href></c:schedule-outbox-URL></d:prop><d:status>HTTP/1.1 404 Not Found</d:status></d:propstat></d:response></d:multistatus>"#).unwrap();
        assert!(success_property(&xml, C, "schedule-outbox-URL").is_none());
    }
    #[test]
    fn freebusy_payload_refuses_ical_injection() {
        let emails = vec!["friend@example.test".into()];
        assert!(
            request(
                "owner@example.test",
                "20261030T120000Z",
                "20261030T130000Z",
                "20261030T110000Z",
                &emails
            )
            .unwrap()
            .contains("METHOD:REQUEST")
        );
        assert!(
            request(
                "owner@example.test\r\nATTENDEE:evil",
                "20261030T120000Z",
                "20261030T130000Z",
                "20261030T110000Z",
                &emails
            )
            .is_err()
        );
    }
    #[test]
    fn availability_response_does_not_treat_errors_as_free_time() {
        let xml = r#"<c:schedule-response xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:d="DAV:"><c:response><c:recipient><d:href>mailto:friend@example.test</d:href></c:recipient><c:request-status>2.0;Success</c:request-status><c:calendar-data>BEGIN:VCALENDAR</c:calendar-data></c:response></c:schedule-response>"#;
        let emails = vec!["friend@example.test".into()];
        assert_eq!(responses(xml, &emails).unwrap().len(), 1);
        assert!(
            responses(
                &xml.replace("2.0;Success", "3.7;Invalid Calendar User"),
                &emails
            )
            .is_err()
        );
        assert!(responses(xml, &["other@example.test".into()]).is_err());
    }
}
