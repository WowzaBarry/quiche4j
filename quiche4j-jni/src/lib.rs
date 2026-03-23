extern crate jni;

mod wt;

use env_logger::{Builder, Target};
use jni::objects::{JByteArray, JClass, JObject, JObjectArray, JString, JValue, ReleaseMode};
use jni::sys::{jboolean, jbyteArray, jint, jlong, jobject, jobjectArray};
use jni::JNIEnv;
use quiche::{h3, Config, Connection, ConnectionId, Error, Header, RecvInfo, StreamIter, Type};
use quiche::h3::NameValue;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::slice;

use wt::WtServer;

type JNIResult<T> = Result<T, jni::errors::Error>;

static ARRAY_LIST_CLASS: &str = "java/util/ArrayList";
static HTTP3_HEADER_CLASS: &str = "io/quiche4j/http3/Http3Header";
static LOG_FILTER_ENV: &str = "QUICHE4J_JNI_LOG";

fn error_to_c(e: Error) -> jint {
    match e {
        Error::Done => -1,
        Error::BufferTooShort => -2,
        Error::UnknownVersion => -3,
        Error::InvalidFrame => -4,
        Error::InvalidPacket => -5,
        Error::InvalidState => -6,
        Error::InvalidStreamState(_) => -7,
        Error::InvalidTransportParam => -8,
        Error::CryptoFail => -9,
        Error::TlsFail => -10,
        Error::FlowControl => -11,
        Error::StreamLimit => -12,
        Error::FinalSize => -13,
        Error::CongestionControl => -14,
        Error::StreamStopped(_) => -15,
        Error::StreamReset(_) => -16,
        Error::IdLimit => -17,
        Error::OutOfIdentifiers => -18,
        Error::KeyUpdate => -19,
        Error::CryptoBufferExceeded => -20,
        Error::InvalidAckRange => -21,
        Error::OptimisticAckDetected => -22,
        Error::InvalidDcidInitialization => -23,
    }
}

fn h3_error_code(error: h3::Error) -> i32 {
    match error {
        h3::Error::Done => -1,
        h3::Error::BufferTooShort => -2,
        h3::Error::InternalError => -3,
        h3::Error::ExcessiveLoad => -4,
        h3::Error::IdError => -5,
        h3::Error::StreamCreationError => -6,
        h3::Error::ClosedCriticalStream => -7,
        h3::Error::MissingSettings => -8,
        h3::Error::FrameUnexpected => -9,
        h3::Error::FrameError => -10,
        h3::Error::QpackDecompressionFailed => -11,
        h3::Error::TransportError(_) => -12,
        h3::Error::StreamBlocked => -13,
        h3::Error::SettingsError => -14,
        h3::Error::RequestRejected => -15,
        h3::Error::RequestCancelled => -16,
        h3::Error::RequestIncomplete => -17,
        h3::Error::MessageError => -18,
        h3::Error::ConnectError => -19,
        h3::Error::VersionFallback => -20,
    }
}

fn socket_addr_from_jni(env: &mut JNIEnv, addr_bytes: jbyteArray, port: jint) -> SocketAddr {
    let addr_ref = unsafe { JByteArray::from_raw(addr_bytes) };
    let bytes: Vec<u8> = env.convert_byte_array(&addr_ref).unwrap();
    // Don't let JByteArray drop delete the local ref
    std::mem::forget(addr_ref);
    let ip = if bytes.len() == 4 {
        IpAddr::V4(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]))
    } else {
        let arr: [u8; 16] = bytes[..16].try_into().unwrap();
        IpAddr::V6(Ipv6Addr::from(arr))
    };
    SocketAddr::new(ip, port as u16)
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1init_1logger() {
    let mut builder = Builder::from_env(LOG_FILTER_ENV);
    builder.target(Target::Stdout);
    builder.init();
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1new(
    _env: JNIEnv,
    _class: JClass,
    version: jint,
) -> jlong {
    let config = Config::new(version as u32).unwrap();
    Box::into_raw(Box::new(config)) as jlong
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1load_1cert_1chain_1from_1pem_1file(
    mut env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    path: JString,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    let path_str: String = env.get_string(&path).unwrap().into();
    config.load_cert_chain_from_pem_file(&path_str).unwrap();
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1load_1priv_1key_1from_1pem_1file(
    mut env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    path: JString,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    let path_str: String = env.get_string(&path).unwrap().into();
    config.load_priv_key_from_pem_file(&path_str).unwrap();
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1verify_1peer(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jboolean,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.verify_peer(v != 0);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1grease(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jboolean,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.grease(v != 0);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1log_1keys(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.log_keys();
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1enable_1early_1data(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.enable_early_data();
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1application_1protos(
    mut env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    protos: jbyteArray,
) -> jint {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    let protos_ref = unsafe { JByteArray::from_raw(protos) };
    let protos_bytes: Vec<u8> = env.convert_byte_array(&protos_ref).unwrap();
    match config.set_application_protos_wire_format(&protos_bytes[..]) {
        Ok(_) => 0 as jint,
        Err(e) => error_to_c(e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1max_1idle_1timeout(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_max_idle_timeout(v as u64);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1max_1udp_1payload_1size(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_max_send_udp_payload_size(v as usize);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1initial_1max_1data(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_initial_max_data(v as u64);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1initial_1max_1stream_1data_1bidi_1local(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_initial_max_stream_data_bidi_local(v as u64);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1initial_1max_1stream_1data_1bidi_1remote(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_initial_max_stream_data_bidi_remote(v as u64);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1initial_1max_1stream_1data_1uni(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_initial_max_stream_data_uni(v as u64);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1initial_1max_1streams_1bidi(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_initial_max_streams_bidi(v as u64);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1initial_1max_1streams_1uni(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_initial_max_streams_uni(v as u64);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1ack_1delay_1exponent(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_ack_delay_exponent(v as u64);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1max_1ack_1delay(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jlong,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_max_ack_delay(v as u64);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1disable_1active_1migration(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jboolean,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.set_disable_active_migration(v != 0);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1set_1cc_1algorithm_1name(
    mut env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    name: JString,
) -> jint {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    let name_str: String = env.get_string(&name).unwrap().into();
    match config.set_cc_algorithm_name(&name_str) {
        Ok(_) => 0 as jint,
        Err(e) => error_to_c(e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1config_1enable_1hystart(
    _env: JNIEnv,
    _class: JClass,
    config_ptr: jlong,
    v: jboolean,
) {
    let config = unsafe { &mut *(config_ptr as *mut Config) };
    config.enable_hystart(v != 0);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1accept(
    mut env: JNIEnv,
    _class: JClass,
    scid_java: jbyteArray,
    odcid_java: jbyteArray,
    config_ptr: jlong,
    local_addr_bytes: jbyteArray,
    local_port: jint,
    peer_addr_bytes: jbyteArray,
    peer_port: jint,
) -> jlong {
    let mut config = unsafe { &mut *(config_ptr as *mut Config) };
    let scid_ref = unsafe { JByteArray::from_raw(scid_java) };
    let scid: Vec<u8> = env.convert_byte_array(&scid_ref).unwrap();
    std::mem::forget(scid_ref);

    let odcid_ref = unsafe { JByteArray::from_raw(odcid_java) };
    let odcid: Option<ConnectionId> = if odcid_java.is_null() {
        None
    } else {
        let buf = env.convert_byte_array(&odcid_ref).unwrap();
        match buf.len() {
            0 => None,
            _ => Some(ConnectionId::from_vec(buf)),
        }
    };
    std::mem::forget(odcid_ref);

    let local = socket_addr_from_jni(&mut env, local_addr_bytes, local_port);
    let peer = socket_addr_from_jni(&mut env, peer_addr_bytes, peer_port);

    let scid = ConnectionId::from_vec(scid);
    match quiche::accept(&scid, odcid.as_ref(), local, peer, &mut config) {
        Ok(conn) => Box::into_raw(Box::new(conn)) as jlong,
        Err(e) => error_to_c(e) as jlong,
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1connect(
    mut env: JNIEnv,
    _class: JClass,
    domain: JString,
    conn_id: jbyteArray,
    config_ptr: jlong,
    local_addr_bytes: jbyteArray,
    local_port: jint,
    peer_addr_bytes: jbyteArray,
    peer_port: jint,
) -> jlong {
    let domain_str: Option<String> = if domain.is_null() {
        None
    } else {
        Some(env.get_string(&domain).unwrap().into())
    };
    let mut config = unsafe { &mut *(config_ptr as *mut Config) };
    let conn_id_ref = unsafe { JByteArray::from_raw(conn_id) };
    let scid: Vec<u8> = env.convert_byte_array(&conn_id_ref).unwrap();
    std::mem::forget(conn_id_ref);

    let local = socket_addr_from_jni(&mut env, local_addr_bytes, local_port);
    let peer = socket_addr_from_jni(&mut env, peer_addr_bytes, peer_port);

    let scid = ConnectionId::from_vec(scid);
    match quiche::connect(domain_str.as_deref(), &scid, local, peer, &mut config) {
        Ok(conn) => Box::into_raw(Box::new(conn)) as jlong,
        Err(e) => error_to_c(e) as jlong,
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1negotiate_1version(
    mut env: JNIEnv,
    _class: JClass,
    java_scid: jbyteArray,
    java_dcid: jbyteArray,
    java_buf: jbyteArray,
) -> jint {
    let scid_ref = unsafe { JByteArray::from_raw(java_scid) };
    let scid = env.convert_byte_array(&scid_ref).unwrap();
    std::mem::forget(scid_ref);

    let dcid_ref = unsafe { JByteArray::from_raw(java_dcid) };
    let dcid = env.convert_byte_array(&dcid_ref).unwrap();
    std::mem::forget(dcid_ref);

    let buf_ref = unsafe { JByteArray::from_raw(java_buf) };
    let buf_len = env.get_array_length(&buf_ref).unwrap() as usize;
    let mut buf = vec![0u8; buf_len];

    let scid = ConnectionId::from_ref(&scid[..]);
    let dcid = ConnectionId::from_ref(&dcid[..]);
    let len = quiche::negotiate_version(&scid, &dcid, &mut buf);
    match len {
        Ok(v) => {
            let byte_buf =
                unsafe { slice::from_raw_parts(buf.as_ptr() as *const i8, v) };
            env.set_byte_array_region(&buf_ref, 0, byte_buf).unwrap();
            v as jint
        }
        Err(e) => error_to_c(e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1version_1is_1supported(
    _env: JNIEnv,
    _class: JClass,
    version: jint,
) -> jboolean {
    quiche::version_is_supported(version as u32) as jboolean
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1retry(
    mut env: JNIEnv,
    _class: JClass,
    java_scid: jbyteArray,
    java_dcid: jbyteArray,
    java_new_scid: jbyteArray,
    java_token: jbyteArray,
    version: jint,
    java_buf: jbyteArray,
) -> jint {
    let scid_ref = unsafe { JByteArray::from_raw(java_scid) };
    let scid = env.convert_byte_array(&scid_ref).unwrap();
    std::mem::forget(scid_ref);

    let dcid_ref = unsafe { JByteArray::from_raw(java_dcid) };
    let dcid = env.convert_byte_array(&dcid_ref).unwrap();
    std::mem::forget(dcid_ref);

    let new_scid_ref = unsafe { JByteArray::from_raw(java_new_scid) };
    let new_scid = env.convert_byte_array(&new_scid_ref).unwrap();
    std::mem::forget(new_scid_ref);

    let token_ref = unsafe { JByteArray::from_raw(java_token) };
    let token = env.convert_byte_array(&token_ref).unwrap();
    std::mem::forget(token_ref);

    let buf_ref = unsafe { JByteArray::from_raw(java_buf) };
    let buf_len = env.get_array_length(&buf_ref).unwrap() as usize;
    let mut buf = vec![0u8; buf_len];

    let scid = ConnectionId::from_ref(&scid[..]);
    let dcid = ConnectionId::from_ref(&dcid[..]);
    let new_scid = ConnectionId::from_ref(&new_scid[..]);
    let len = quiche::retry(&scid, &dcid, &new_scid, &token[..], version as u32, &mut buf);
    match len {
        Ok(v) => {
            let byte_buf =
                unsafe { slice::from_raw_parts(buf.as_ptr() as *const i8, v) };
            env.set_byte_array_region(&buf_ref, 0, byte_buf).unwrap();
            v as jint
        }
        Err(e) => error_to_c(e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1recv(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    java_buf: jbyteArray,
    from_addr_bytes: jbyteArray,
    from_port: jint,
    to_addr_bytes: jbyteArray,
    to_port: jint,
) -> jint {
    let conn = unsafe { &mut *(ptr as *mut Connection) };
    let buf_ref = unsafe { JByteArray::from_raw(java_buf) };
    let mut buf = env.convert_byte_array(&buf_ref).unwrap();
    std::mem::forget(buf_ref);

    let from = socket_addr_from_jni(&mut env, from_addr_bytes, from_port);
    let to = socket_addr_from_jni(&mut env, to_addr_bytes, to_port);

    let recv_info = RecvInfo { from, to };
    match conn.recv(&mut buf, recv_info) {
        Ok(v) => v as jint,
        Err(e) => error_to_c(e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1send(
    mut env: JNIEnv,
    _class: JClass,
    ptr: jlong,
    java_buf: jbyteArray,
) -> jint {
    let conn = unsafe { &mut *(ptr as *mut Connection) };
    let buf_ref = unsafe { JByteArray::from_raw(java_buf) };
    let buf_len = env.get_array_length(&buf_ref).unwrap() as usize;
    let mut buf = vec![0u8; buf_len];

    let sent_len = conn.send(&mut buf);
    match sent_len {
        Ok((v, _send_info)) => {
            let byte_buf =
                unsafe { slice::from_raw_parts(buf.as_ptr() as *const i8, v) };
            env.set_byte_array_region(&buf_ref, 0, byte_buf).unwrap();
            v as jint
        }
        Err(e) => error_to_c(e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1close(
    mut env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
    app: jboolean,
    error: jlong,
    reason: jbyteArray,
) -> jint {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let reason_ref = unsafe { JByteArray::from_raw(reason) };
    let reason_bytes = env.convert_byte_array(&reason_ref).unwrap();
    match conn.close(app != 0, error as u64, &reason_bytes[..]) {
        Ok(_) => 0 as jint,
        Err(e) => error_to_c(e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1timeout_1as_1nanos(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
) -> jlong {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    match conn.timeout() {
        Some(timeout) => timeout.as_nanos() as jlong,
        None => std::u64::MAX as jlong,
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1timeout_1as_1millis(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
) -> jlong {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    match conn.timeout() {
        Some(timeout) => timeout.as_millis() as jlong,
        None => std::u64::MAX as jlong,
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1on_1timeout(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
) {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    conn.on_timeout();
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1is_1established(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
) -> jboolean {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    conn.is_established() as jboolean
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1is_1in_1early_1data(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
) -> jboolean {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    conn.is_in_early_data() as jboolean
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1is_1closed(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
) -> jboolean {
    let conn = unsafe { &*(conn_ptr as *mut Connection) };
    conn.is_closed() as jboolean
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1stats(
    mut env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
    holder: jobject,
) {
    let conn = unsafe { &*(conn_ptr as *mut Connection) };
    let holder = unsafe { JObject::from_raw(holder) };
    let stats = conn.stats();

    // rtt, cwnd, delivery_rate moved to PathStats
    let path = conn.path_stats().next();
    let (rtt_ms, cwnd, delivery_rate) = match path {
        Some(p) => (p.rtt.as_millis() as jlong, p.cwnd as jint, p.delivery_rate as jlong),
        None => (0 as jlong, 0 as jint, 0 as jlong),
    };

    env.call_method(
        &holder,
        "setRecv",
        "(I)V",
        &[JValue::Int(stats.recv as jint)],
    )
    .unwrap();
    env.call_method(
        &holder,
        "setSent",
        "(I)V",
        &[JValue::Int(stats.sent as jint)],
    )
    .unwrap();
    env.call_method(
        &holder,
        "setLost",
        "(I)V",
        &[JValue::Int(stats.lost as jint)],
    )
    .unwrap();
    env.call_method(
        &holder,
        "setRtt",
        "(J)V",
        &[JValue::Long(rtt_ms)],
    )
    .unwrap();
    env.call_method(
        &holder,
        "setCwnd",
        "(I)V",
        &[JValue::Int(cwnd)],
    )
    .unwrap();
    env.call_method(
        &holder,
        "setDeliveryRate",
        "(J)V",
        &[JValue::Long(delivery_rate)],
    )
    .unwrap();
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1free(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
) {
    unsafe { Box::from_raw(conn_ptr as *mut Connection) };
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1stream_1recv(
    mut env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
    stream_id: jlong,
    java_buf: jbyteArray,
) -> jint {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let buf_ref = unsafe { JByteArray::from_raw(java_buf) };
    let buf_len = env.get_array_length(&buf_ref).unwrap() as usize;
    let mut buf = vec![0u8; buf_len];
    match conn.stream_recv(stream_id as u64, &mut buf) {
        Ok((out_len, _out_fin)) => {
            env.set_byte_array_region(&buf_ref, 0, unsafe {
                slice::from_raw_parts(buf.as_ptr() as *const i8, out_len)
            })
            .unwrap();
            out_len as i32
        }
        Err(e) => error_to_c(e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1stream_1send(
    mut env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
    stream_id: jlong,
    java_buf: jbyteArray,
    fin: jboolean,
) -> jint {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let buf_ref = unsafe { JByteArray::from_raw(java_buf) };
    let buf_len = env.get_array_length(&buf_ref).unwrap() as usize;
    let mut buf = vec![0u8; buf_len];
    env.get_byte_array_region(&buf_ref, 0, unsafe {
        slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut i8, buf_len)
    })
    .unwrap();

    let sent_len = conn.stream_send(stream_id as u64, &buf, fin != 0);
    match sent_len {
        Ok(v) => v as jint,
        Err(e) => error_to_c(e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1stream_1shutdown(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
    stream_id: jlong,
    direction: jint,
    err: jlong,
) {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let dir = match direction {
        0 => quiche::Shutdown::Read,
        _ => quiche::Shutdown::Write,
    };
    match conn.stream_shutdown(stream_id as u64, dir, err as u64) {
        Ok(_) => (),
        Err(Error::Done) => (),
        Err(e) => panic!("{:?}", e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1stream_1capacity(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
    stream_id: jlong,
) -> jint {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    match conn.stream_capacity(stream_id as u64) {
        Ok(v) => v as jint,
        Err(e) => error_to_c(e),
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1stream_1finished(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
    stream_id: jlong,
) -> jboolean {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    conn.stream_finished(stream_id as u64) as jboolean
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1readable(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
) -> jlong {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    Box::into_raw(Box::new(conn.readable())) as jlong
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1conn_1writable(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
) -> jlong {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    Box::into_raw(Box::new(conn.writable())) as jlong
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1stream_1iter_1next(
    _env: JNIEnv,
    _class: JClass,
    stream_iter_ptr: jlong,
) -> jlong {
    let stream_iter = unsafe { &mut *(stream_iter_ptr as *mut StreamIter) };
    match stream_iter.next() {
        Some(stream_id) => stream_id as jlong,
        None => error_to_c(Error::Done) as jlong,
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1stream_1iter_1free(
    _env: JNIEnv,
    _class: JClass,
    stream_iter_ptr: jlong,
) {
    unsafe { Box::from_raw(stream_iter_ptr as *mut StreamIter) };
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1config_1new(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    let h3_config = h3::Config::new().unwrap();
    Box::into_raw(Box::new(h3_config)) as jlong
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1conn_1new_1with_1transport(
    _env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
    h3_config_ptr: jlong,
) -> jlong {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let h3_config = unsafe { &mut *(h3_config_ptr as *mut h3::Config) };
    let h3_conn = h3::Connection::with_transport(conn, h3_config).unwrap();
    Box::into_raw(Box::new(h3_conn)) as jlong
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1conn_1free(
    _env: JNIEnv,
    _class: JClass,
    h3_conn_ptr: jlong,
) {
    unsafe { Box::from_raw(h3_conn_ptr as *mut h3::Connection) };
}

fn headers_from_java(env: &mut JNIEnv, headers: jobjectArray) -> JNIResult<Vec<h3::Header>> {
    let headers_ref = unsafe { JObjectArray::from_raw(headers) };
    let len = env.get_array_length(&headers_ref)? as i32;
    let mut buf = Vec::<h3::Header>::with_capacity(len as usize);
    for i in 0..len {
        let jobj = env.get_object_array_element(&headers_ref, i)?;
        let name = env
            .call_method(&jobj, "name", "()Ljava/lang/String;", &[])?
            .l()?;
        let value = env
            .call_method(&jobj, "value", "()Ljava/lang/String;", &[])?
            .l()?;
        let name_jstr: JString = name.into();
        let value_jstr: JString = value.into();
        let name_str: String = env.get_string(&name_jstr)?.into();
        let value_str: String = env.get_string(&value_jstr)?.into();
        buf.push(h3::Header::new(name_str.as_bytes(), value_str.as_bytes()));
    }
    std::mem::forget(headers_ref);
    Ok(buf)
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1send_1request(
    mut env: JNIEnv,
    _class: JClass,
    h3_ptr: jlong,
    conn_ptr: jlong,
    headers: jobjectArray,
    fin: jboolean,
) {
    let h3_conn = unsafe { &mut *(h3_ptr as *mut h3::Connection) };
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let req = headers_from_java(&mut env, headers).unwrap();
    h3_conn.send_request(conn, &req, fin != 0).unwrap();
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1send_1response(
    mut env: JNIEnv,
    _class: JClass,
    h3_ptr: jlong,
    conn_ptr: jlong,
    stream_id: jlong,
    headers: jobjectArray,
    fin: jboolean,
) -> jint {
    let h3_conn = unsafe { &mut *(h3_ptr as *mut h3::Connection) };
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let req = headers_from_java(&mut env, headers).unwrap();
    match h3_conn.send_response(conn, stream_id as u64, &req, fin != 0) {
        Ok(_) => 0 as jint,
        Err(e) => h3_error_code(e) as jint,
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1send_1body(
    mut env: JNIEnv,
    _class: JClass,
    h3_ptr: jlong,
    conn_ptr: jlong,
    stream_id: jlong,
    java_body: jbyteArray,
    fin: jboolean,
) -> jlong {
    let h3_conn = unsafe { &mut *(h3_ptr as *mut h3::Connection) };
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let body_ref = unsafe { JByteArray::from_raw(java_body) };
    let body: Vec<u8> = env.convert_byte_array(&body_ref).unwrap();
    match h3_conn.send_body(conn, stream_id as u64, &body[..], fin != 0) {
        Ok(v) => v as jlong,
        Err(e) => h3_error_code(e) as jlong,
    }
}

fn call_on_headers(
    env: &mut JNIEnv,
    listener: &JObject,
    stream_id: u64,
    headers: Vec<h3::Header>,
    has_body: bool,
) -> JNIResult<()> {
    let holder = env.new_object(ARRAY_LIST_CLASS, "()V", &[])?;
    for header in &headers {
        let name_str = std::str::from_utf8(header.name()).unwrap_or("");
        let val_str = std::str::from_utf8(header.value()).unwrap_or("");
        let name_jstr = env.new_string(name_str)?;
        let val_jstr = env.new_string(val_str)?;
        let elem = env.new_object(
            HTTP3_HEADER_CLASS,
            "(Ljava/lang/String;Ljava/lang/String;)V",
            &[
                JValue::Object(&name_jstr),
                JValue::Object(&val_jstr),
            ],
        )?;
        env.call_method(&holder, "add", "(Ljava/lang/Object;)Z", &[JValue::Object(&elem)])?;
    }
    env.call_method(
        listener,
        "onHeaders",
        "(JLjava/util/List;Z)V",
        &[
            JValue::Long(stream_id as jlong),
            JValue::Object(&holder),
            JValue::Bool(has_body as jboolean),
        ],
    )?;
    Ok(())
}

fn call_on_data(env: &mut JNIEnv, handler: &JObject, stream_id: u64) -> JNIResult<()> {
    env.call_method(
        handler,
        "onData",
        "(J)V",
        &[JValue::Long(stream_id as jlong)],
    )?;
    Ok(())
}

fn call_on_finished(env: &mut JNIEnv, handler: &JObject, stream_id: u64) -> JNIResult<()> {
    env.call_method(
        handler,
        "onFinished",
        "(J)V",
        &[JValue::Long(stream_id as jlong)],
    )?;
    Ok(())
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1conn_1poll(
    mut env: JNIEnv,
    _class: JClass,
    h3_conn_ptr: jlong,
    conn_ptr: jlong,
    listener: jobject,
) -> jlong {
    let h3_conn = unsafe { &mut *(h3_conn_ptr as *mut h3::Connection) };
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let listener_ref = unsafe { JObject::from_raw(listener) };
    match h3_conn.poll(conn) {
        Ok((stream_id, h3::Event::Headers { list, more_frames })) => {
            call_on_headers(&mut env, &listener_ref, stream_id, list, more_frames).unwrap();
            stream_id as jlong
        }
        Ok((stream_id, h3::Event::Data)) => {
            call_on_data(&mut env, &listener_ref, stream_id).unwrap();
            stream_id as jlong
        }
        Ok((stream_id, h3::Event::Finished)) => {
            call_on_finished(&mut env, &listener_ref, stream_id).unwrap();
            stream_id as jlong
        }
        Ok((_stream_id, h3::Event::Reset(_))) => {
            // Stream was reset by peer, treat as done
            h3_error_code(h3::Error::Done) as jlong
        }
        Ok((_stream_id, h3::Event::PriorityUpdate)) => {
            // Priority update, ignore
            h3_error_code(h3::Error::Done) as jlong
        }
        Ok((_stream_id, h3::Event::GoAway)) => {
            // GoAway received, treat as done
            h3_error_code(h3::Error::Done) as jlong
        }
        Err(e) => h3_error_code(e) as jlong,
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1recv_1body(
    mut env: JNIEnv,
    _class: JClass,
    h3_conn_ptr: jlong,
    conn_ptr: jlong,
    stream_id: jlong,
    java_buf: jbyteArray,
) -> jint {
    let h3_conn = unsafe { &mut *(h3_conn_ptr as *mut h3::Connection) };
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let buf_ref = unsafe { JByteArray::from_raw(java_buf) };
    let buf_len = env.get_array_length(&buf_ref).unwrap() as usize;
    let mut buf = vec![0u8; buf_len];

    let body_len = h3_conn.recv_body(conn, stream_id as u64, &mut buf);
    match body_len {
        Ok(v) => {
            let byte_buf =
                unsafe { slice::from_raw_parts(buf.as_ptr() as *const i8, v) };
            env.set_byte_array_region(&buf_ref, 0, byte_buf).unwrap();
            v as jint
        }
        Err(e) => h3_error_code(e) as jint,
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_Native_quiche_1header_1from_1slice(
    mut env: JNIEnv,
    _class: JClass,
    java_buf: jbyteArray,
    dcid_len: jint,
    holder: jobject,
) {
    let buf_ref = unsafe { JByteArray::from_raw(java_buf) };
    let mut buf: Vec<u8> = env.convert_byte_array(&buf_ref).unwrap();
    let holder = unsafe { JObject::from_raw(holder) };
    let hdr = Header::from_slice(&mut buf, dcid_len as usize).unwrap();
    let ty_java = match hdr.ty {
        Type::Initial => 1,
        Type::Retry => 2,
        Type::Handshake => 3,
        Type::ZeroRTT => 4,
        Type::Short => 5,
        Type::VersionNegotiation => 6,
    };
    env.call_method(
        &holder,
        "setPacketType",
        "(I)V",
        &[JValue::Int(ty_java as jint)],
    )
    .unwrap();
    env.call_method(
        &holder,
        "setVersion",
        "(I)V",
        &[JValue::Int(hdr.version as jint)],
    )
    .unwrap();

    // hdr.dcid and hdr.scid are now ConnectionId, use .as_ref() for byte slice
    let dcid_bytes = env.byte_array_from_slice(hdr.dcid.as_ref()).unwrap();
    env.call_method(
        &holder,
        "setDestinationConnectionId",
        "([B)V",
        &[JValue::Object(&dcid_bytes)],
    )
    .unwrap();
    let scid_bytes = env.byte_array_from_slice(hdr.scid.as_ref()).unwrap();
    env.call_method(
        &holder,
        "setSourceConnectionId",
        "([B)V",
        &[JValue::Object(&scid_bytes)],
    )
    .unwrap();
    match hdr.token {
        Some(token) => {
            let token_bytes = env.byte_array_from_slice(&token).unwrap();
            env.call_method(
                &holder,
                "setToken",
                "([B)V",
                &[JValue::Object(&token_bytes)],
            )
            .unwrap();
        }
        None => {}
    }
    match hdr.versions {
        Some(versions) => {
            let versions_java = env.new_int_array(versions.len() as i32).unwrap();
            env.call_method(
                &holder,
                "setVersions",
                "([I)V",
                &[JValue::Object(&versions_java)],
            )
            .unwrap();
        }
        None => {}
    }
}

// --- H3 Config extensions for WebTransport ---

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1config_1free(
    _env: JNIEnv,
    _class: JClass,
    h3_config_ptr: jlong,
) {
    unsafe { Box::from_raw(h3_config_ptr as *mut h3::Config) };
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1config_1enable_1extended_1connect(
    _env: JNIEnv,
    _class: JClass,
    h3_config_ptr: jlong,
    enabled: jboolean,
) {
    let h3_config = unsafe { &mut *(h3_config_ptr as *mut h3::Config) };
    h3_config.enable_extended_connect(enabled != 0);
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1config_1set_1additional_1settings(
    mut env: JNIEnv,
    _class: JClass,
    h3_config_ptr: jlong,
    java_keys: jbyteArray,
    java_values: jbyteArray,
) -> jint {
    let h3_config = unsafe { &mut *(h3_config_ptr as *mut h3::Config) };
    let keys_ref = unsafe { JByteArray::from_raw(java_keys) };
    let vals_ref = unsafe { JByteArray::from_raw(java_values) };
    let keys_bytes = env.convert_byte_array(&keys_ref).unwrap();
    let vals_bytes = env.convert_byte_array(&vals_ref).unwrap();
    std::mem::forget(keys_ref);
    std::mem::forget(vals_ref);

    // Keys and values are packed as big-endian u64 arrays
    let count = keys_bytes.len() / 8;
    let mut settings = Vec::with_capacity(count);
    for i in 0..count {
        let off = i * 8;
        let key = u64::from_be_bytes(keys_bytes[off..off + 8].try_into().unwrap());
        let val = u64::from_be_bytes(vals_bytes[off..off + 8].try_into().unwrap());
        settings.push((key, val));
    }
    match h3_config.set_additional_settings(settings) {
        Ok(_) => 0 as jint,
        Err(_) => -1 as jint,
    }
}

#[no_mangle]
#[warn(unused_variables)]
pub extern "system" fn Java_io_quiche4j_http3_Http3Native_quiche_1h3_1extended_1connect_1enabled_1by_1peer(
    _env: JNIEnv,
    _class: JClass,
    h3_conn_ptr: jlong,
) -> jboolean {
    let h3_conn = unsafe { &*(h3_conn_ptr as *mut h3::Connection) };
    h3_conn.extended_connect_enabled_by_peer() as jboolean
}

// --- WebTransport JNI bindings ---

#[no_mangle]
pub extern "system" fn Java_io_quiche4j_http3_WtNative_wt_1server_1new(
    _env: JNIEnv,
    _class: JClass,
) -> jlong {
    let wt = WtServer::new();
    Box::into_raw(Box::new(wt)) as jlong
}

#[no_mangle]
pub extern "system" fn Java_io_quiche4j_http3_WtNative_wt_1server_1free(
    _env: JNIEnv,
    _class: JClass,
    wt_ptr: jlong,
) {
    unsafe { Box::from_raw(wt_ptr as *mut WtServer) };
}

#[no_mangle]
pub extern "system" fn Java_io_quiche4j_http3_WtNative_wt_1server_1poll(
    mut env: JNIEnv,
    _class: JClass,
    wt_ptr: jlong,
    h3_conn_ptr: jlong,
    conn_ptr: jlong,
    listener: jobject,
) -> jint {
    let wt = unsafe { &mut *(wt_ptr as *mut WtServer) };
    let h3_conn = unsafe { &mut *(h3_conn_ptr as *mut h3::Connection) };
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let listener_ref = unsafe { JObject::from_raw(listener) };

    let events = wt.poll(conn, h3_conn);
    let count = events.len() as jint;

    for evt in events {
        match evt {
            wt::WtEvent::NewSession { session_id, path } => {
                let path_jstr = env.new_string(&path).unwrap();
                let _ = env.call_method(
                    &listener_ref,
                    "onNewSession",
                    "(JLjava/lang/String;)V",
                    &[
                        JValue::Long(session_id as jlong),
                        JValue::Object(&path_jstr),
                    ],
                );
            }
            wt::WtEvent::NewStream { session_id, stream_id, bidi } => {
                let _ = env.call_method(
                    &listener_ref,
                    "onNewStream",
                    "(JJZ)V",
                    &[
                        JValue::Long(session_id as jlong),
                        JValue::Long(stream_id as jlong),
                        JValue::Bool(bidi as jboolean),
                    ],
                );
            }
            wt::WtEvent::Data { stream_id } => {
                call_on_data(&mut env, &listener_ref, stream_id).unwrap();
            }
            wt::WtEvent::Finished { stream_id } => {
                call_on_finished(&mut env, &listener_ref, stream_id).unwrap();
            }
            wt::WtEvent::H3Headers { stream_id, headers, has_body } => {
                call_on_headers(&mut env, &listener_ref, stream_id, headers, has_body).unwrap();
            }
            wt::WtEvent::H3Data { stream_id } => {
                call_on_data(&mut env, &listener_ref, stream_id).unwrap();
            }
            wt::WtEvent::H3Finished { stream_id } => {
                call_on_finished(&mut env, &listener_ref, stream_id).unwrap();
            }
        }
    }

    count
}

#[no_mangle]
pub extern "system" fn Java_io_quiche4j_http3_WtNative_wt_1server_1open_1uni_1stream(
    _env: JNIEnv,
    _class: JClass,
    wt_ptr: jlong,
    conn_ptr: jlong,
    session_id: jlong,
) -> jlong {
    let wt = unsafe { &mut *(wt_ptr as *mut WtServer) };
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    match wt.open_uni_stream(conn, session_id as u64) {
        Ok(stream_id) => stream_id as jlong,
        Err(_) => -1 as jlong,
    }
}

#[no_mangle]
pub extern "system" fn Java_io_quiche4j_http3_WtNative_wt_1server_1open_1bidi_1stream(
    _env: JNIEnv,
    _class: JClass,
    wt_ptr: jlong,
    conn_ptr: jlong,
    session_id: jlong,
) -> jlong {
    let wt = unsafe { &mut *(wt_ptr as *mut WtServer) };
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    match wt.open_bidi_stream(conn, session_id as u64) {
        Ok(stream_id) => stream_id as jlong,
        Err(_) => -1 as jlong,
    }
}

#[no_mangle]
pub extern "system" fn Java_io_quiche4j_http3_WtNative_wt_1server_1stream_1send(
    mut env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
    stream_id: jlong,
    java_buf: jbyteArray,
    fin: jboolean,
) -> jlong {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let buf_ref = unsafe { JByteArray::from_raw(java_buf) };
    let buf: Vec<u8> = env.convert_byte_array(&buf_ref).unwrap();
    std::mem::forget(buf_ref);
    match conn.stream_send(stream_id as u64, &buf, fin != 0) {
        Ok(v) => v as jlong,
        Err(e) => error_to_c(e) as jlong,
    }
}

#[no_mangle]
pub extern "system" fn Java_io_quiche4j_http3_WtNative_wt_1server_1stream_1recv(
    mut env: JNIEnv,
    _class: JClass,
    conn_ptr: jlong,
    stream_id: jlong,
    java_buf: jbyteArray,
) -> jint {
    let conn = unsafe { &mut *(conn_ptr as *mut Connection) };
    let buf_ref = unsafe { JByteArray::from_raw(java_buf) };
    let buf_len = env.get_array_length(&buf_ref).unwrap() as usize;
    let mut buf = vec![0u8; buf_len];
    match conn.stream_recv(stream_id as u64, &mut buf) {
        Ok((out_len, _fin)) => {
            env.set_byte_array_region(&buf_ref, 0, unsafe {
                slice::from_raw_parts(buf.as_ptr() as *const i8, out_len)
            })
            .unwrap();
            out_len as jint
        }
        Err(e) => error_to_c(e),
    }
}
