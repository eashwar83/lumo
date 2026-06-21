use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::time;
use url::Url;

use crate::store::network_connection_store::NetworkConnectionRecord;

const MDNS_MULTICAST_ADDR: &str = "224.0.0.251:5353";
const MDNS_MULTICAST_IPV4: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const SMB_SERVICE_NAME: &str = "_smb._tcp.local";
const DNS_TYPE_A: u16 = 1;
const DNS_TYPE_PTR: u16 = 12;
const DNS_TYPE_TXT: u16 = 16;
const DNS_TYPE_AAAA: u16 = 28;
const DNS_TYPE_SRV: u16 = 33;
const DNS_CLASS_IN: u16 = 0x0001;
const DNS_CLASS_IN_QU: u16 = 0x8001;

const SMB_PATH_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'[')
    .add(b']')
    .add(b'\\')
    .add(b'^')
    .add(b'|');
const SMB_USERINFO_ENCODE_SET: &AsciiSet = &SMB_PATH_ENCODE_SET
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'@');

#[derive(Clone)]
pub struct SmbDevice {
    pub instance_name: String,
    pub location: String,
    pub friendly_name: Option<String>,
    pub server: Option<String>,
    pub service_type: String,
}

#[derive(Clone)]
pub struct SmbBrowseEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
}

pub struct SmbBrowseResult {
    pub path: String,
    pub entries: Vec<SmbBrowseEntry>,
}

#[repr(C)]
struct SoiaSmbBrowseEntry {
    name: *mut c_char,
    is_dir: c_int,
    size: u64,
    has_size: c_int,
    modified_at: i64,
    has_modified_at: c_int,
}

#[repr(C)]
struct SoiaSmbBrowseResult {
    entries: *mut SoiaSmbBrowseEntry,
    entry_count: usize,
    error: *mut c_char,
}

#[repr(C)]
struct SoiaSmbReadResult {
    data: *mut u8,
    data_len: usize,
    file_size: u64,
    has_file_size: c_int,
    error: *mut c_char,
}

#[repr(C)]
struct SoiaSmbOpenResult {
    file: *mut SoiaSmbFile,
    file_size: u64,
    has_file_size: c_int,
    error: *mut c_char,
}

#[repr(C)]
struct SoiaSmbFile {
    _private: [u8; 0],
}

#[link(name = "soia_utils")]
unsafe extern "C" {
    fn soia_smb_browse_directory(
        server: *const c_char,
        share: *const c_char,
        path: *const c_char,
        domain: *const c_char,
        username: *const c_char,
        password: *const c_char,
        result: *mut SoiaSmbBrowseResult,
    ) -> c_int;
    fn soia_smb_browse_result_free(result: *mut SoiaSmbBrowseResult);
    fn soia_smb_read_result_free(result: *mut SoiaSmbReadResult);
    fn soia_smb_file_open(uri: *const c_char, result: *mut SoiaSmbOpenResult) -> c_int;
    fn soia_smb_open_result_free(result: *mut SoiaSmbOpenResult);
    fn soia_smb_file_read_at(
        file: *mut SoiaSmbFile,
        offset: u64,
        length: usize,
        result: *mut SoiaSmbReadResult,
    ) -> c_int;
    fn soia_smb_file_close(file: *mut SoiaSmbFile);
}

struct ParsedSmbConnection {
    server: String,
    share: Option<String>,
    root_path: String,
    domain: String,
    username: String,
    password: String,
}

struct ResolvedSmbTarget {
    share: String,
    target_path: String,
    display_path: String,
}

#[derive(Clone, Default)]
struct SmbService {
    instance_name: String,
    target: Option<String>,
    port: Option<u16>,
    addresses: Vec<IpAddr>,
    txt: Vec<String>,
    source_addr: Option<IpAddr>,
}

#[derive(Debug)]
enum MdnsRecordData {
    Ptr(String),
    Srv { port: u16, target: String },
    Txt(Vec<String>),
    A(Ipv4Addr),
    Aaaa,
    Other,
}

#[derive(Debug)]
struct MdnsRecord {
    name: String,
    data: MdnsRecordData,
}

pub async fn discover_devices_with_callback<F>(
    timeout_secs: u64,
    mut on_device: F,
) -> Result<Vec<SmbDevice>, String>
where
    F: FnMut(SmbDevice),
{
    let timeout_secs = timeout_secs.max(1);
    log::info!(
        "SMB mDNS scan started: service={}, timeout={}s",
        SMB_SERVICE_NAME,
        timeout_secs
    );

    let (socket, receives_multicast) = create_mdns_socket().await?;
    let target: SocketAddr = MDNS_MULTICAST_ADDR
        .parse()
        .map_err(|e| format!("Invalid mDNS multicast address: {}", e))?;

    send_query(
        &socket,
        target,
        &[(SMB_SERVICE_NAME, DNS_TYPE_PTR)],
        !receives_multicast,
    )
    .await?;
    log::info!(
        "SMB mDNS PTR query sent to {} (multicast_listener={})",
        MDNS_MULTICAST_ADDR,
        receives_multicast
    );

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut buffer = [0u8; 9000];
    let mut services: HashMap<String, SmbService> = HashMap::new();
    let mut queried_services = HashSet::new();
    let mut queried_hosts = HashSet::new();
    let mut emitted_locations = HashSet::new();
    let mut packets = 0usize;

    while Instant::now() < deadline {
        let remain = deadline.saturating_duration_since(Instant::now());
        let recv = time::timeout(remain, socket.recv_from(&mut buffer)).await;
        let Ok(Ok((len, addr))) = recv else {
            break;
        };
        packets += 1;

        let records = match parse_mdns_records(&buffer[..len]) {
            Ok(records) => records,
            Err(error) => {
                log::debug!("Malformed SMB mDNS response ignored: {}", error);
                continue;
            }
        };
        let mut service_queries = Vec::new();
        let mut host_queries = Vec::new();

        for record in records {
            match record.data {
                MdnsRecordData::Ptr(instance) => {
                    if !record.name.eq_ignore_ascii_case(SMB_SERVICE_NAME) {
                        continue;
                    }
                    let key = normalize_dns_name(&instance);
                    let service = services.entry(key.clone()).or_insert_with(|| SmbService {
                        instance_name: instance.clone(),
                        source_addr: Some(addr.ip()),
                        ..Default::default()
                    });
                    if service.source_addr.is_none() {
                        service.source_addr = Some(addr.ip());
                    }
                    if queried_services.insert(key) {
                        service_queries.push((instance, DNS_TYPE_SRV));
                        service_queries.push((service.instance_name.clone(), DNS_TYPE_TXT));
                    }
                }
                MdnsRecordData::Srv { port, target } => {
                    let key = normalize_dns_name(&record.name);
                    let service = services.entry(key).or_insert_with(|| SmbService {
                        instance_name: record.name.clone(),
                        source_addr: Some(addr.ip()),
                        ..Default::default()
                    });
                    service.port = Some(port);
                    service.target = Some(target.clone());
                    if queried_hosts.insert(normalize_dns_name(&target)) {
                        host_queries.push((target.clone(), DNS_TYPE_A));
                        host_queries.push((target, DNS_TYPE_AAAA));
                    }
                }
                MdnsRecordData::Txt(items) => {
                    let key = normalize_dns_name(&record.name);
                    let service = services.entry(key).or_insert_with(|| SmbService {
                        instance_name: record.name.clone(),
                        source_addr: Some(addr.ip()),
                        ..Default::default()
                    });
                    service.txt = items;
                }
                MdnsRecordData::A(address) => {
                    let hostname = normalize_dns_name(&record.name);
                    for service in services.values_mut() {
                        if service
                            .target
                            .as_ref()
                            .map(|target| normalize_dns_name(target) == hostname)
                            .unwrap_or(false)
                            && !service.addresses.contains(&IpAddr::V4(address))
                        {
                            service.addresses.push(IpAddr::V4(address));
                        }
                    }
                }
                MdnsRecordData::Aaaa | MdnsRecordData::Other => {}
            }
        }

        if !service_queries.is_empty() {
            let queries = service_queries
                .iter()
                .map(|(name, record_type)| (name.as_str(), *record_type))
                .collect::<Vec<_>>();
            send_query(&socket, target, &queries, !receives_multicast).await?;
        }
        if !host_queries.is_empty() {
            let queries = host_queries
                .iter()
                .filter(|(name, _)| !name.is_empty())
                .map(|(name, record_type)| (name.as_str(), *record_type))
                .collect::<Vec<_>>();
            if !queries.is_empty() {
                send_query(&socket, target, &queries, !receives_multicast).await?;
            }
        }

        for service in services.values() {
            let Some(device) = service_to_device(service.clone()) else {
                continue;
            };
            if emitted_locations.insert(device.location.clone()) {
                log::info!(
                    "SMB mDNS device discovered: name={}, location={}, friendly_name={}, server={}",
                    device.instance_name,
                    device.location,
                    device.friendly_name.as_deref().unwrap_or(""),
                    device.server.as_deref().unwrap_or("")
                );
                on_device(device);
            }
        }
    }

    let mut devices = services
        .into_values()
        .filter_map(service_to_device)
        .collect::<Vec<_>>();
    devices.sort_by(|a, b| {
        a.friendly_name
            .as_deref()
            .unwrap_or(&a.instance_name)
            .cmp(b.friendly_name.as_deref().unwrap_or(&b.instance_name))
    });
    for device in &devices {
        log::info!(
            "SMB mDNS device discovered: name={}, location={}, friendly_name={}, server={}",
            device.instance_name,
            device.location,
            device.friendly_name.as_deref().unwrap_or(""),
            device.server.as_deref().unwrap_or("")
        );
    }
    log::info!(
        "SMB mDNS scan finished: discovered={}, packets={}",
        devices.len(),
        packets
    );
    Ok(devices)
}

pub async fn list_directory(
    connection: &NetworkConnectionRecord,
    path: &str,
) -> Result<SmbBrowseResult, String> {
    let connection = parse_smb_connection(connection)?;
    let normalized_path = normalize_path(path);
    let target = resolve_smb_target(&connection, &normalized_path)?;

    tauri::async_runtime::spawn_blocking(move || browse_directory_blocking(connection, target))
        .await
        .map_err(|e| format!("SMB browse task failed: {}", e))?
}

pub fn build_playback_url(
    connection: &NetworkConnectionRecord,
    file_path: &str,
) -> Result<String, String> {
    let connection = parse_smb_connection(connection)?;
    let normalized_path = normalize_path(file_path);
    let target = resolve_smb_target(&connection, &normalized_path)?;
    if target.share.is_empty() {
        return Err("SMB share is required for playback".into());
    }
    let path = encode_smb_path(&target.target_path);
    let share = encode_smb_segment(&target.share);
    let suffix = if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", path)
    };
    Ok(format!(
        "smb://{}/{}{}",
        connection.server, share, suffix
    ))
}

pub(crate) fn playback_url_with_credentials(
    playback_url: &str,
    username: &str,
    password: &str,
) -> Result<String, String> {
    let username = username.trim();
    if username.is_empty() {
        return Ok(playback_url.to_string());
    }
    let rest = playback_url
        .strip_prefix("smb://")
        .ok_or_else(|| "SMB playback URL must start with smb://".to_string())?;
    let (authority, suffix) = rest
        .split_once('/')
        .map(|(authority, suffix)| (authority, format!("/{suffix}")))
        .unwrap_or((rest, "/".to_string()));
    let authority = authority.rsplit_once('@').map(|(_, value)| value).unwrap_or(authority);
    let (domain, username) = username
        .split_once(';')
        .map(|(domain, user)| (domain.trim(), user.trim()))
        .unwrap_or(("", username));
    let domain = if domain.is_empty() {
        String::new()
    } else {
        format!("{};", encode_smb_userinfo(domain))
    };
    Ok(format!(
        "smb://{}{}:{}@{}{}",
        domain,
        encode_smb_userinfo(username),
        encode_smb_userinfo(password),
        authority,
        suffix
    ))
}

pub struct SmbReadRangeResult {
    pub data: Vec<u8>,
}

pub struct SmbPlaybackFile {
    file: *mut SoiaSmbFile,
    file_size: Option<u64>,
}

unsafe impl Send for SmbPlaybackFile {}

impl SmbPlaybackFile {
    pub fn file_size(&self) -> Option<u64> {
        self.file_size
    }

    pub fn read_range(&mut self, offset: u64, length: usize) -> Result<SmbReadRangeResult, String> {
        if self.file.is_null() {
            return Err("SMB file is closed".to_string());
        }
        let mut result = SoiaSmbReadResult {
            data: std::ptr::null_mut(),
            data_len: 0,
            file_size: 0,
            has_file_size: 0,
            error: std::ptr::null_mut(),
        };
        let status = unsafe {
            soia_smb_file_read_at(self.file, offset, length, &mut result)
        };
        read_result_from_ffi(status, result)
    }
}

impl Drop for SmbPlaybackFile {
    fn drop(&mut self) {
        if !self.file.is_null() {
            unsafe {
                soia_smb_file_close(self.file);
            }
            self.file = std::ptr::null_mut();
        }
    }
}

pub async fn open_playback_url(playback_url: String) -> Result<SmbPlaybackFile, String> {
    tauri::async_runtime::spawn_blocking(move || open_playback_url_blocking(&playback_url))
        .await
        .map_err(|e| format!("SMB open task failed: {}", e))?
}

fn open_playback_url_blocking(playback_url: &str) -> Result<SmbPlaybackFile, String> {
    let uri = cstring("SMB URI", playback_url)?;
    let mut result = SoiaSmbOpenResult {
        file: std::ptr::null_mut(),
        file_size: 0,
        has_file_size: 0,
        error: std::ptr::null_mut(),
    };
    let status = unsafe { soia_smb_file_open(uri.as_ptr(), &mut result) };
    if status != 0 || result.file.is_null() {
        let error = c_string_lossy(result.error).unwrap_or_else(|| "SMB open failed".to_string());
        unsafe {
            soia_smb_open_result_free(&mut result);
        }
        return Err(error);
    }
    let file = result.file;
    let file_size = (result.has_file_size != 0).then_some(result.file_size);
    unsafe {
        soia_smb_open_result_free(&mut result);
    }
    Ok(SmbPlaybackFile { file, file_size })
}

fn read_result_from_ffi(
    status: c_int,
    mut result: SoiaSmbReadResult,
) -> Result<SmbReadRangeResult, String> {
    if status != 0 {
        let error = c_string_lossy(result.error).unwrap_or_else(|| "SMB read failed".to_string());
        unsafe {
            soia_smb_read_result_free(&mut result);
        }
        return Err(error);
    }

    let data = if result.data.is_null() || result.data_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(result.data, result.data_len).to_vec() }
    };
    unsafe {
        soia_smb_read_result_free(&mut result);
    }
    Ok(SmbReadRangeResult { data })
}

fn browse_directory_blocking(
    connection: ParsedSmbConnection,
    target: ResolvedSmbTarget,
) -> Result<SmbBrowseResult, String> {
    let server = cstring("SMB server", &connection.server)?;
    let share = cstring("SMB share", &target.share)?;
    let path = cstring("SMB path", &target.target_path)?;
    let domain = cstring("SMB domain", &connection.domain)?;
    let username = cstring("SMB username", &connection.username)?;
    let password = cstring("SMB password", &connection.password)?;

    let mut result = SoiaSmbBrowseResult {
        entries: std::ptr::null_mut(),
        entry_count: 0,
        error: std::ptr::null_mut(),
    };

    let status = unsafe {
        soia_smb_browse_directory(
            server.as_ptr(),
            share.as_ptr(),
            path.as_ptr(),
            domain.as_ptr(),
            username.as_ptr(),
            password.as_ptr(),
            &mut result,
        )
    };

    if status != 0 {
        let error = c_string_lossy(result.error)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "SMB browse failed".to_string());
        unsafe {
            soia_smb_browse_result_free(&mut result);
        }
        return Err(error);
    }

    let entries = if result.entry_count == 0 {
        Vec::new()
    } else {
        unsafe {
            std::slice::from_raw_parts(result.entries, result.entry_count)
                .iter()
                .filter_map(|entry| smb_entry_from_ffi(&target.display_path, entry))
                .collect::<Vec<_>>()
        }
    };
    unsafe {
        soia_smb_browse_result_free(&mut result);
    }

    let mut entries = entries;
    entries.sort_by(|left, right| match (left.is_dir, right.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
    });

    Ok(SmbBrowseResult {
        path: target.display_path,
        entries,
    })
}

fn parse_smb_connection(
    connection: &NetworkConnectionRecord,
) -> Result<ParsedSmbConnection, String> {
    let url = Url::parse(connection.base_url.trim())
        .map_err(|e| format!("Invalid SMB URL: {}", e))?;
    if url.scheme() != "smb" {
        return Err("SMB URL must start with smb://".into());
    }

    let mut server = url
        .host_str()
        .ok_or_else(|| "SMB URL host is required".to_string())?
        .to_string();
    if let Some(port) = url.port() {
        server = format!("{}:{}", server, port);
    }

    let segments = url
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .map(|segment| percent_decode_str(segment).decode_utf8_lossy().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let share = segments
        .first()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let root_path = if share.is_some() && segments.len() > 1 {
        normalize_path(&format!("/{}", segments[1..].join("/")))
    } else {
        "/".to_string()
    };

    let mut username = connection.username.trim().to_string();
    let mut password = connection.password.clone();
    if username.is_empty() && !url.username().is_empty() {
        username = percent_decode_str(url.username()).decode_utf8_lossy().to_string();
    }
    if password.is_empty() {
        if let Some(url_password) = url.password() {
            password = percent_decode_str(url_password).decode_utf8_lossy().to_string();
        }
    }
    let mut domain = String::new();
    if let Some((domain_part, user_part)) = username.split_once(';') {
        domain = domain_part.trim().to_string();
        username = user_part.trim().to_string();
    }

    Ok(ParsedSmbConnection {
        server,
        share,
        root_path,
        domain,
        username,
        password,
    })
}

fn resolve_smb_target(
    connection: &ParsedSmbConnection,
    path: &str,
) -> Result<ResolvedSmbTarget, String> {
    let normalized_path = normalize_path(path);
    if let Some(share) = &connection.share {
        let target_path = join_paths(&connection.root_path, &normalized_path);
        let display_path = normalize_path_without_root(&connection.root_path, &target_path);
        return Ok(ResolvedSmbTarget {
            share: share.clone(),
            target_path,
            display_path,
        });
    }

    if normalized_path == "/" {
        return Ok(ResolvedSmbTarget {
            share: String::new(),
            target_path: "/".to_string(),
            display_path: "/".to_string(),
        });
    }

    let mut segments = normalized_path
        .trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty());
    let share = segments
        .next()
        .map(|segment| segment.to_string())
        .ok_or_else(|| "SMB share is required".to_string())?;
    let remaining = segments.collect::<Vec<_>>();
    let target_path = if remaining.is_empty() {
        "/".to_string()
    } else {
        normalize_path(&format!("/{}", remaining.join("/")))
    };

    Ok(ResolvedSmbTarget {
        share,
        target_path,
        display_path: normalized_path,
    })
}

fn smb_entry_from_ffi(current_path: &str, entry: &SoiaSmbBrowseEntry) -> Option<SmbBrowseEntry> {
    let name = c_string_lossy(entry.name)?.trim().to_string();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let path = child_path(current_path, &name);
    Some(SmbBrowseEntry {
        name,
        path,
        is_dir: entry.is_dir != 0,
        size: if entry.has_size != 0 {
            Some(entry.size)
        } else {
            None
        },
        modified_at: if entry.has_modified_at != 0 {
            DateTime::<Utc>::from_timestamp(entry.modified_at, 0).map(|time| time.to_rfc3339())
        } else {
            None
        },
    })
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    let with_leading = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    };
    with_leading.trim_end_matches('/').to_string()
}

fn join_paths(root: &str, path: &str) -> String {
    let root = normalize_path(root);
    let path = normalize_path(path);
    if root == "/" {
        return path;
    }
    if path == "/" {
        return root;
    }
    format!("{}/{}", root.trim_end_matches('/'), path.trim_start_matches('/'))
}

fn child_path(parent: &str, name: &str) -> String {
    let parent = normalize_path(parent);
    if parent == "/" {
        format!("/{}", name)
    } else {
        format!("{}/{}", parent, name)
    }
}

fn normalize_path_without_root(root: &str, path: &str) -> String {
    let root = normalize_path(root);
    let path = normalize_path(path);
    if root == "/" {
        return path;
    }
    if path == root {
        return "/".to_string();
    }
    path.strip_prefix(root.trim_end_matches('/'))
        .map(normalize_path)
        .unwrap_or(path)
}

fn encode_smb_segment(value: &str) -> String {
    utf8_percent_encode(value, SMB_PATH_ENCODE_SET).to_string()
}

fn encode_smb_path(path: &str) -> String {
    normalize_path(path)
        .trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(encode_smb_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_smb_userinfo(value: &str) -> String {
    utf8_percent_encode(value, SMB_USERINFO_ENCODE_SET).to_string()
}

fn cstring(label: &str, value: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("{} contains an embedded NUL byte", label))
}

fn c_string_lossy(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned())
}

async fn send_query(
    socket: &UdpSocket,
    target: SocketAddr,
    questions: &[(&str, u16)],
    request_unicast_response: bool,
) -> Result<(), String> {
    if questions.is_empty() {
        return Ok(());
    }
    let mut packet = Vec::with_capacity(512);
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&(questions.len() as u16).to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    packet.extend_from_slice(&0u16.to_be_bytes());
    for (name, record_type) in questions {
        encode_dns_name(name, &mut packet)?;
        packet.extend_from_slice(&record_type.to_be_bytes());
        let record_class = if request_unicast_response {
            DNS_CLASS_IN_QU
        } else {
            DNS_CLASS_IN
        };
        packet.extend_from_slice(&record_class.to_be_bytes());
    }
    socket
        .send_to(&packet, target)
        .await
        .map_err(|e| format!("Failed to send mDNS query: {}", e))?;
    Ok(())
}

async fn create_mdns_socket() -> Result<(UdpSocket, bool), String> {
    match create_multicast_mdns_socket() {
        Ok(socket) => return Ok((socket, true)),
        Err(error) => {
            log::debug!(
                "Failed to bind SMB mDNS multicast listener, falling back to unicast replies: {}",
                error
            );
        }
    }

    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("Failed to bind fallback mDNS UDP socket: {}", e))?;
    Ok((socket, false))
}

fn create_multicast_mdns_socket() -> Result<UdpSocket, String> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| format!("Failed to create mDNS socket: {}", e))?;
    socket
        .set_reuse_address(true)
        .map_err(|e| format!("Failed to set mDNS SO_REUSEADDR: {}", e))?;
    #[cfg(unix)]
    socket
        .set_reuse_port(true)
        .map_err(|e| format!("Failed to set mDNS SO_REUSEPORT: {}", e))?;
    socket
        .bind(&SocketAddr::from(([0, 0, 0, 0], 5353)).into())
        .map_err(|e| format!("Failed to bind mDNS multicast port 5353: {}", e))?;
    socket
        .join_multicast_v4(&MDNS_MULTICAST_IPV4, &Ipv4Addr::UNSPECIFIED)
        .map_err(|e| format!("Failed to join mDNS multicast group: {}", e))?;
    socket
        .set_nonblocking(true)
        .map_err(|e| format!("Failed to set mDNS socket nonblocking: {}", e))?;

    let std_socket: std::net::UdpSocket = socket.into();
    std_socket
        .set_multicast_loop_v4(true)
        .map_err(|e| format!("Failed to enable mDNS multicast loop: {}", e))?;
    std_socket
        .set_multicast_ttl_v4(255)
        .map_err(|e| format!("Failed to set mDNS multicast TTL: {}", e))?;
    UdpSocket::from_std(std_socket).map_err(|e| format!("Failed to wrap mDNS socket: {}", e))
}

fn parse_mdns_records(packet: &[u8]) -> Result<Vec<MdnsRecord>, String> {
    if packet.len() < 12 {
        return Ok(Vec::new());
    }
    let qdcount = read_u16(packet, 4)? as usize;
    let ancount = read_u16(packet, 6)? as usize;
    let nscount = read_u16(packet, 8)? as usize;
    let arcount = read_u16(packet, 10)? as usize;
    let mut offset = 12usize;

    for _ in 0..qdcount {
        let (_, next) = read_dns_name(packet, offset)?;
        offset = next.saturating_add(4);
        if offset > packet.len() {
            return Ok(Vec::new());
        }
    }

    let mut records = Vec::new();
    for _ in 0..(ancount + nscount + arcount) {
        let (name, next) = read_dns_name(packet, offset)?;
        offset = next;
        if offset + 10 > packet.len() {
            break;
        }
        let record_type = read_u16(packet, offset)?;
        offset += 2;
        let _class = read_u16(packet, offset)?;
        offset += 2;
        let _ttl = read_u32(packet, offset)?;
        offset += 4;
        let data_len = read_u16(packet, offset)? as usize;
        offset += 2;
        if offset + data_len > packet.len() {
            break;
        }
        let data_offset = offset;
        let data_end = offset + data_len;
        offset = data_end;

        let data = match record_type {
            DNS_TYPE_PTR => {
                let (value, _) = read_dns_name(packet, data_offset)?;
                MdnsRecordData::Ptr(value)
            }
            DNS_TYPE_SRV => {
                if data_offset + 6 > data_end {
                    MdnsRecordData::Other
                } else {
                    let port = read_u16(packet, data_offset + 4)?;
                    let (target, _) = read_dns_name(packet, data_offset + 6)?;
                    MdnsRecordData::Srv { port, target }
                }
            }
            DNS_TYPE_TXT => MdnsRecordData::Txt(read_txt_record(&packet[data_offset..data_end])),
            DNS_TYPE_A => {
                if data_len == 4 {
                    MdnsRecordData::A(Ipv4Addr::new(
                        packet[data_offset],
                        packet[data_offset + 1],
                        packet[data_offset + 2],
                        packet[data_offset + 3],
                    ))
                } else {
                    MdnsRecordData::Other
                }
            }
            DNS_TYPE_AAAA => MdnsRecordData::Aaaa,
            _ => MdnsRecordData::Other,
        };
        records.push(MdnsRecord { name, data });
    }

    Ok(records)
}

fn service_to_device(service: SmbService) -> Option<SmbDevice> {
    let host = service
        .addresses
        .iter()
        .find(|addr| matches!(addr, IpAddr::V4(_)))
        .or_else(|| service.addresses.first())
        .map(|addr| addr.to_string())
        .or_else(|| service.target.as_deref().map(trim_trailing_dot))
        .or_else(|| service.source_addr.map(|addr| addr.to_string()))?;
    let port = service.port.unwrap_or(445);
    let location = if port == 445 {
        format!("smb://{}/", host)
    } else {
        format!("smb://{}:{}/", host, port)
    };
    let friendly_name = service_label(&service.instance_name);
    let server = service
        .target
        .as_deref()
        .map(trim_trailing_dot)
        .or_else(|| service.txt.iter().find(|item| !item.is_empty()).cloned());

    Some(SmbDevice {
        instance_name: service.instance_name,
        location,
        friendly_name,
        server,
        service_type: SMB_SERVICE_NAME.to_string(),
    })
}

fn service_label(instance_name: &str) -> Option<String> {
    let lower = instance_name.to_ascii_lowercase();
    let suffix = format!(".{}", SMB_SERVICE_NAME);
    let value = if lower.ends_with(&suffix) {
        &instance_name[..instance_name.len().saturating_sub(suffix.len())]
    } else {
        instance_name
    };
    let label = trim_trailing_dot(value).trim().to_string();
    if label.is_empty() {
        None
    } else {
        Some(label)
    }
}

fn encode_dns_name(name: &str, packet: &mut Vec<u8>) -> Result<(), String> {
    for label in name.trim_end_matches('.').split('.') {
        let bytes = label.as_bytes();
        if bytes.len() > 63 {
            return Err(format!("mDNS label is too long: {}", label));
        }
        packet.push(bytes.len() as u8);
        packet.extend_from_slice(bytes);
    }
    packet.push(0);
    Ok(())
}

fn read_dns_name(packet: &[u8], offset: usize) -> Result<(String, usize), String> {
    let mut labels = Vec::new();
    let mut cursor = offset;
    let mut next_offset = None;
    let mut jumps = 0usize;

    loop {
        if cursor >= packet.len() {
            return Err("Malformed mDNS packet: name exceeds packet".to_string());
        }
        let len = packet[cursor];
        if len & 0xC0 == 0xC0 {
            if cursor + 1 >= packet.len() {
                return Err("Malformed mDNS packet: truncated pointer".to_string());
            }
            let pointer = (((len & 0x3F) as usize) << 8) | packet[cursor + 1] as usize;
            if next_offset.is_none() {
                next_offset = Some(cursor + 2);
            }
            cursor = pointer;
            jumps += 1;
            if jumps > 16 {
                return Err("Malformed mDNS packet: pointer loop".to_string());
            }
            continue;
        }
        if len == 0 {
            let end = next_offset.unwrap_or(cursor + 1);
            return Ok((labels.join("."), end));
        }
        cursor += 1;
        let end = cursor + len as usize;
        if end > packet.len() {
            return Err("Malformed mDNS packet: truncated label".to_string());
        }
        labels.push(String::from_utf8_lossy(&packet[cursor..end]).to_string());
        cursor = end;
    }
}

fn read_txt_record(data: &[u8]) -> Vec<String> {
    let mut offset = 0usize;
    let mut items = Vec::new();
    while offset < data.len() {
        let len = data[offset] as usize;
        offset += 1;
        if offset + len > data.len() {
            break;
        }
        items.push(String::from_utf8_lossy(&data[offset..offset + len]).to_string());
        offset += len;
    }
    items
}

fn normalize_dns_name(value: &str) -> String {
    trim_trailing_dot(value).to_ascii_lowercase()
}

fn trim_trailing_dot(value: &str) -> String {
    value.trim_end_matches('.').to_string()
}

fn read_u16(packet: &[u8], offset: usize) -> Result<u16, String> {
    if offset + 2 > packet.len() {
        return Err("Malformed mDNS packet: expected u16".to_string());
    }
    Ok(u16::from_be_bytes([packet[offset], packet[offset + 1]]))
}

fn read_u32(packet: &[u8], offset: usize) -> Result<u32, String> {
    if offset + 4 > packet.len() {
        return Err("Malformed mDNS packet: expected u32".to_string());
    }
    Ok(u32::from_be_bytes([
        packet[offset],
        packet[offset + 1],
        packet[offset + 2],
        packet[offset + 3],
    ]))
}
