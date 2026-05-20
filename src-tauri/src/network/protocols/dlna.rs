use crate::store::network_connection_store::NetworkConnectionRecord;
use roxmltree::Document;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::time;
use url::Url;

const SSDP_MULTICAST_ADDR: &str = "239.255.255.250:1900";
const SSDP_ST_MEDIA_SERVER: &str = "urn:schemas-upnp-org:device:MediaServer:1";

#[derive(Clone)]
pub struct DlnaDevice {
    pub usn: String,
    pub location: String,
    pub friendly_name: Option<String>,
    pub server: Option<String>,
    pub st: String,
}

#[derive(Clone)]
pub struct DlnaBrowseEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
}

pub struct DlnaBrowseResult {
    pub path: String,
    pub entries: Vec<DlnaBrowseEntry>,
}

#[derive(Clone)]
struct ContentDirectoryService {
    service_type: String,
    control_url: Url,
}

pub async fn discover_devices_with_callback<F>(
    app: &tauri::AppHandle,
    timeout_secs: u64,
    mut on_device: F,
) -> Result<Vec<DlnaDevice>, String>
where
    F: FnMut(DlnaDevice),
{
    log::info!(
        "SSDP scan started: st={}, timeout={}s",
        SSDP_ST_MEDIA_SERVER,
        timeout_secs.max(1)
    );
    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("Failed to bind UDP socket: {}", e))?;
    let local_addr = socket
        .local_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    log::debug!("SSDP socket bound on {}", local_addr);

    let target: SocketAddr = SSDP_MULTICAST_ADDR
        .parse()
        .map_err(|e| format!("Invalid SSDP multicast address: {}", e))?;

    let request = [
        "M-SEARCH * HTTP/1.1",
        &format!("HOST: {}", SSDP_MULTICAST_ADDR),
        "MAN: \"ssdp:discover\"",
        "MX: 2",
        &format!("ST: {}", SSDP_ST_MEDIA_SERVER),
        "",
        "",
    ]
    .join("\r\n");

    socket
        .send_to(request.as_bytes(), target)
        .await
        .map_err(|e| format!("Failed to send SSDP discovery packet: {}", e))?;
    log::info!(
        "SSDP M-SEARCH sent to {} (mx=2, st={})",
        SSDP_MULTICAST_ADDR,
        SSDP_ST_MEDIA_SERVER
    );

    let deadline = Instant::now() + Duration::from_secs(timeout_secs.max(1));
    let mut found = Vec::new();
    let mut dedup = HashSet::new();
    let mut buffer = [0u8; 8192];
    let mut response_count = 0usize;
    let mut ignored_no_location = 0usize;
    let mut duplicate_count = 0usize;

    while Instant::now() < deadline {
        let remain = deadline.saturating_duration_since(Instant::now());
        let recv = time::timeout(remain, socket.recv_from(&mut buffer)).await;
        let Ok(Ok((len, addr))) = recv else {
            break;
        };
        response_count += 1;
        log::debug!("SSDP response received: bytes={}, from={}", len, addr);

        let packet = String::from_utf8_lossy(&buffer[..len]).to_string();
        let headers = parse_ssdp_headers(&packet);

        let Some(location) = headers.get("location").cloned() else {
            ignored_no_location += 1;
            log::debug!("SSDP response ignored: missing LOCATION header");
            continue;
        };

        let usn = headers
            .get("usn")
            .cloned()
            .unwrap_or_else(|| location.clone());

        let key = format!("{}|{}", usn, location);
        if dedup.contains(&key) {
            duplicate_count += 1;
            log::debug!("SSDP duplicate ignored: usn={}, location={}", usn, location);
            continue;
        }
        dedup.insert(key);

        let server = headers.get("server").cloned();
        let st = headers
            .get("st")
            .cloned()
            .unwrap_or_else(|| SSDP_ST_MEDIA_SERVER.to_string());
        log::info!(
            "SSDP device discovered: usn={}, st={}, location={}, server={}",
            usn,
            st,
            location,
            server.clone().unwrap_or_else(|| "-".to_string())
        );
        let device = DlnaDevice {
            usn,
            location,
            friendly_name: None,
            server,
            st,
        };
        on_device(device.clone());
        found.push(device);
    }

    let client = crate::network::proxy::configure_client_builder(
        app,
        reqwest::Client::builder().timeout(Duration::from_secs(2)),
    )?
    .build()
        .map_err(|e| format!("Failed to create DLNA discovery HTTP client: {}", e))?;

    log::info!(
        "SSDP scan finished: discovered={}, responses={}, duplicates={}, ignored_no_location={}",
        found.len(),
        response_count,
        duplicate_count,
        ignored_no_location
    );

    for device in &mut found {
        let friendly_name = fetch_device_friendly_name(&client, &device.location).await;
        if let Some(name) = friendly_name.as_ref() {
            log::info!(
                "DLNA device name resolved: usn={}, friendly_name={}",
                device.usn,
                name
            );
        }
        device.friendly_name = friendly_name;
    }

    Ok(found)
}

pub async fn browse_directory(
    app: &tauri::AppHandle,
    connection: &NetworkConnectionRecord,
    object_id: &str,
) -> Result<DlnaBrowseResult, String> {
    let base_url = Url::parse(connection.base_url.trim())
        .map_err(|e| format!("Invalid DLNA device URL: {}", e))?;

    let service = fetch_content_directory_service(app, &base_url).await?;
    let result_xml = soap_browse(app, &service, object_id).await?;

    parse_browse_result(object_id, &result_xml)
}

fn parse_ssdp_headers(response: &str) -> HashMap<String, String> {
    response
        .lines()
        .skip(1)
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let (k, v) = trimmed.split_once(':')?;
            Some((k.trim().to_lowercase(), v.trim().to_string()))
        })
        .collect()
}

async fn fetch_content_directory_service(
    app: &tauri::AppHandle,
    base_url: &Url,
) -> Result<ContentDirectoryService, String> {
    let client = crate::network::proxy::configure_client_builder(
        app,
        reqwest::Client::builder().timeout(Duration::from_secs(10)),
    )?
    .build()
        .map_err(|e| e.to_string())?;

    let body = client
        .get(base_url.clone())
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("Failed to fetch DLNA device description: {}", e))?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let doc = Document::parse(&body).map_err(|e| format!("Invalid DLNA description XML: {}", e))?;

    let service_node = doc
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name().eq_ignore_ascii_case("serviceType")
                && node
                    .text()
                    .map(|txt| txt.to_ascii_lowercase().contains("contentdirectory"))
                    .unwrap_or(false)
        })
        .and_then(|service_type_node| {
            service_type_node.ancestors().find(|node| {
                node.is_element() && node.tag_name().name().eq_ignore_ascii_case("service")
            })
        })
        .ok_or_else(|| "DLNA device has no ContentDirectory service".to_string())?;

    let service_type = child_text(service_node, "serviceType")
        .ok_or_else(|| "Missing ContentDirectory serviceType".to_string())?;
    let control_url_raw = child_text(service_node, "controlURL")
        .ok_or_else(|| "Missing ContentDirectory controlURL".to_string())?;
    let control_url = base_url
        .join(control_url_raw.trim())
        .map_err(|e| format!("Invalid DLNA control URL: {}", e))?;

    Ok(ContentDirectoryService {
        service_type,
        control_url,
    })
}

async fn fetch_device_friendly_name(client: &reqwest::Client, location: &str) -> Option<String> {
    let response = client.get(location).send().await.ok()?;
    let response = response.error_for_status().ok()?;
    let body = response.text().await.ok()?;
    let doc = Document::parse(&body).ok()?;
    find_doc_text(&doc, "friendlyName")
}

fn find_doc_text(doc: &Document<'_>, name: &str) -> Option<String> {
    doc.descendants()
        .find(|child| child.is_element() && child.tag_name().name().eq_ignore_ascii_case(name))
        .and_then(|child| child.text())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn child_text(node: roxmltree::Node<'_, '_>, child_name: &str) -> Option<String> {
    node.children()
        .find(|child| {
            child.is_element() && child.tag_name().name().eq_ignore_ascii_case(child_name)
        })
        .and_then(|child| child.text())
        .map(|txt| txt.trim().to_string())
        .filter(|txt| !txt.is_empty())
}

async fn soap_browse(
    app: &tauri::AppHandle,
    service: &ContentDirectoryService,
    object_id: &str,
) -> Result<String, String> {
    let object_id = if object_id.trim().is_empty() {
        "0"
    } else {
        object_id.trim()
    };

    let envelope = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Browse xmlns:u="{service_type}">
      <ObjectID>{object_id}</ObjectID>
      <BrowseFlag>BrowseDirectChildren</BrowseFlag>
      <Filter>*</Filter>
      <StartingIndex>0</StartingIndex>
      <RequestedCount>0</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Browse>
  </s:Body>
</s:Envelope>"#,
        service_type = service.service_type,
        object_id = xml_escape(object_id),
    );

    let client = crate::network::proxy::configure_client_builder(
        app,
        reqwest::Client::builder().timeout(Duration::from_secs(15)),
    )?
    .build()
        .map_err(|e| e.to_string())?;

    let response_text = client
        .post(service.control_url.clone())
        .header("SOAPAction", format!("\"{}#Browse\"", service.service_type))
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .body(envelope)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("DLNA browse failed: {}", e))?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let doc = Document::parse(&response_text)
        .map_err(|e| format!("Invalid DLNA SOAP response: {}", e))?;
    let result_xml = doc
        .descendants()
        .find(|node| node.is_element() && node.tag_name().name().eq_ignore_ascii_case("Result"))
        .and_then(|node| node.text())
        .ok_or_else(|| "DLNA browse response missing Result".to_string())?
        .to_string();

    Ok(result_xml)
}

fn parse_browse_result(object_id: &str, didl_xml: &str) -> Result<DlnaBrowseResult, String> {
    if didl_xml.trim().is_empty() {
        return Ok(DlnaBrowseResult {
            path: normalize_object_id(object_id),
            entries: Vec::new(),
        });
    }

    let doc = Document::parse(didl_xml).map_err(|e| format!("Invalid DIDL-Lite XML: {}", e))?;
    let mut entries: Vec<DlnaBrowseEntry> = Vec::new();

    for node in doc.root_element().children().filter(|n| n.is_element()) {
        let name = node.tag_name().name();
        if name.eq_ignore_ascii_case("container") {
            let id = node.attribute("id").unwrap_or("").trim();
            if id.is_empty() {
                continue;
            }
            let title = find_descendant_text(node, "title").unwrap_or_else(|| id.to_string());
            entries.push(DlnaBrowseEntry {
                name: title,
                path: id.to_string(),
                is_dir: true,
                size: None,
                modified_at: find_descendant_text(node, "date"),
            });
        } else if name.eq_ignore_ascii_case("item") {
            let title = find_descendant_text(node, "title").unwrap_or_else(|| "item".to_string());
            let res_node = node.children().find(|child| {
                child.is_element() && child.tag_name().name().eq_ignore_ascii_case("res")
            });
            let Some(res) = res_node else {
                continue;
            };
            let Some(url_text) = res.text().map(|v| v.trim()).filter(|v| !v.is_empty()) else {
                continue;
            };
            let size = res.attribute("size").and_then(|v| v.parse::<u64>().ok());
            entries.push(DlnaBrowseEntry {
                name: title,
                path: url_text.to_string(),
                is_dir: false,
                size,
                modified_at: find_descendant_text(node, "date"),
            });
        }
    }

    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
    });

    Ok(DlnaBrowseResult {
        path: normalize_object_id(object_id),
        entries,
    })
}

fn find_descendant_text(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    node.descendants()
        .find(|child| child.is_element() && child.tag_name().name().eq_ignore_ascii_case(name))
        .and_then(|child| child.text())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn normalize_object_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}
