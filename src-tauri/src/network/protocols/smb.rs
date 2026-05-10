use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;
use tokio::time;

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

#[derive(Clone)]
pub struct SmbDevice {
    pub instance_name: String,
    pub location: String,
    pub friendly_name: Option<String>,
    pub server: Option<String>,
    pub service_type: String,
}

#[derive(Default)]
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

pub async fn discover_devices(timeout_secs: u64) -> Result<Vec<SmbDevice>, String> {
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
    log::info!(
        "SMB mDNS scan finished: discovered={}, packets={}",
        devices.len(),
        packets
    );
    Ok(devices)
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
