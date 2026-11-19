//! 루프백 TLS 확인. docs/DESIGN.md "도메인 연결" 4.
//!
//! 루프백 주소에만 연결해 핸드셰이크만 하고 인증서를 받는다. 인증서는 검증하지 않고(읽기만 한다) 데이터는 보내지 않는다.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portwatch_core::cert;
use portwatch_core::{Scheme, Snapshot};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, ClientConnection, DigitallySignedStruct, SignatureScheme};

/// 연결과 핸드셰이크 제한 시간.
pub const TIMEOUT: Duration = Duration::from_secs(1);
/// 같은 (주소, 포트, 이름) 결과를 다시 쓰는 시간.
pub const CACHE_TTL: Duration = Duration::from_secs(60);

/// 인증서 읽기. 루프백 주소만 받는다.
pub trait TlsProbe: Sync {
    fn peer_certificate(&self, ip: IpAddr, port: u16, name: &str) -> Result<Vec<u8>, String>;
}

/// TLS 확인을 하지 않는다. 결과는 "확인하지 않음" 오류다.
pub struct NoTls;

impl TlsProbe for NoTls {
    fn peer_certificate(&self, _: IpAddr, _: u16, _: &str) -> Result<Vec<u8>, String> {
        Err("인증서를 확인하지 않았습니다.".to_owned())
    }
}

/// rustls로 실제 연결한다.
pub struct RustlsProbe;

impl TlsProbe for RustlsProbe {
    fn peer_certificate(&self, ip: IpAddr, port: u16, name: &str) -> Result<Vec<u8>, String> {
        if !ip.is_loopback() {
            return Err("루프백 주소만 확인합니다.".to_owned());
        }
        handshake(SocketAddr::new(ip, port), name)
            .map_err(|error| format!("인증서를 읽지 못했습니다: {error}"))
    }
}

/// (주소, 포트, 이름) → (읽은 시각, 결과). 앱 전체에서 같이 쓴다.
pub type TlsCache = Mutex<HashMap<(IpAddr, u16, String), (Instant, Result<Vec<u8>, String>)>>;

/// 결과를 [`CACHE_TTL`] 동안 다시 쓰는 감싸개.
pub struct CachedProbe<'a> {
    inner: &'a dyn TlsProbe,
    cache: &'a TlsCache,
}

impl<'a> CachedProbe<'a> {
    pub fn new(inner: &'a dyn TlsProbe, cache: &'a TlsCache) -> Self {
        Self { inner, cache }
    }
}

impl TlsProbe for CachedProbe<'_> {
    fn peer_certificate(&self, ip: IpAddr, port: u16, name: &str) -> Result<Vec<u8>, String> {
        let key = (ip, port, name.to_owned());
        if let Ok(cache) = self.cache.lock() {
            if let Some((at, result)) = cache.get(&key) {
                if at.elapsed() < CACHE_TTL {
                    return result.clone();
                }
            }
        }
        let result = self.inner.peer_certificate(ip, port, name);
        if let Ok(mut cache) = self.cache.lock() {
            cache.retain(|_, (at, _)| at.elapsed() < CACHE_TTL);
            cache.insert(key, (Instant::now(), result.clone()));
        }
        result
    }
}

/// https 주소마다 인증서를 읽어 `tls`를 채운다.
pub fn fill_certificates(snapshot: &mut Snapshot, probe: &dyn TlsProbe, now_ms: u64) {
    // 포트를 루프백 IPv6에만 열었으면 [::1], 그 밖에는 127.0.0.1로 연결한다.
    let mut v6_only: HashMap<u16, bool> = HashMap::new();
    for listener in snapshot.entries.iter().flat_map(|e| &e.listeners) {
        let only = v6_only.entry(listener.port).or_insert(true);
        *only &= listener.address == "::1";
    }
    for domain in snapshot
        .entries
        .iter_mut()
        .flat_map(|e| e.domains.iter_mut())
        .filter(|d| d.scheme == Scheme::Https)
    {
        let ip = if v6_only.get(&domain.port).copied().unwrap_or(false) {
            IpAddr::V6(Ipv6Addr::LOCALHOST)
        } else {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        };
        domain.tls = Some(
            match probe.peer_certificate(ip, domain.port, &domain.name) {
                Ok(der) => cert::describe(&der, &domain.name, now_ms),
                Err(error) => cert::failed(&error),
            },
        );
    }
}

fn handshake(address: SocketAddr, name: &str) -> io::Result<Vec<u8>> {
    let invalid = |error: String| io::Error::new(io::ErrorKind::InvalidInput, error);
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|e| invalid(e.to_string()))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(ReadOnly(provider)))
        .with_no_client_auth();
    let server_name = ServerName::try_from(name.to_owned()).map_err(|e| invalid(e.to_string()))?;
    let mut connection =
        ClientConnection::new(Arc::new(config), server_name).map_err(|e| invalid(e.to_string()))?;

    let started = Instant::now();
    let mut socket = TcpStream::connect_timeout(&address, TIMEOUT)?;
    socket.set_read_timeout(Some(TIMEOUT))?;
    socket.set_write_timeout(Some(TIMEOUT))?;
    while connection.is_handshaking() {
        if started.elapsed() > TIMEOUT * 2 {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "시간 초과"));
        }
        connection.complete_io(&mut socket)?;
    }
    let der = connection
        .peer_certificates()
        .and_then(|chain| chain.first())
        .map(|cert| cert.to_vec())
        .ok_or_else(|| io::Error::other("인증서가 없습니다"))?;
    connection.send_close_notify();
    let _ = connection.complete_io(&mut socket);
    Ok(der)
}

/// 인증서를 믿을지 정하지 않고 읽기만 한다. 서명 확인은 rustls 기본대로 한다.
#[derive(Debug)]
struct ReadOnly(Arc<CryptoProvider>);

impl ServerCertVerifier for ReadOnly {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::pki_types::PrivateKeyDer;
    use rustls::{ServerConfig, ServerConnection};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn reads_the_certificate_of_a_loopback_tls_server() {
        let certified =
            rcgen::generate_simple_self_signed(vec!["shop.example.test".into()]).unwrap();
        let cert_der = certified.cert.der().clone();
        let key = PrivateKeyDer::Pkcs8(certified.signing_key.serialize_der().into());
        let config =
            ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .unwrap()
                .with_no_client_auth()
                .with_single_cert(vec![cert_der.clone()], key)
                .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut connection = ServerConnection::new(Arc::new(config)).unwrap();
            while connection.is_handshaking() {
                if connection.complete_io(&mut socket).is_err() {
                    break;
                }
            }
            connection.server_name().map(str::to_owned)
        });

        let der = RustlsProbe
            .peer_certificate(IpAddr::V4(Ipv4Addr::LOCALHOST), port, "shop.example.test")
            .unwrap();
        assert_eq!(der, cert_der.to_vec());
        assert_eq!(
            server.join().unwrap().as_deref(),
            Some("shop.example.test"),
            "SNI로 이름을 보낸다"
        );
    }

    #[test]
    fn refuses_non_loopback_and_reports_closed_ports() {
        let error = RustlsProbe
            .peer_certificate("192.0.2.1".parse().unwrap(), 443, "shop.example.test")
            .unwrap_err();
        assert_eq!(error, "루프백 주소만 확인합니다.");

        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let error = RustlsProbe
            .peer_certificate(IpAddr::V4(Ipv4Addr::LOCALHOST), port, "shop.example.test")
            .unwrap_err();
        assert!(error.starts_with("인증서를 읽지 못했습니다: "), "{error}");
    }

    struct Counting(AtomicUsize);

    impl TlsProbe for Counting {
        fn peer_certificate(&self, _: IpAddr, port: u16, _: &str) -> Result<Vec<u8>, String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(vec![port as u8])
        }
    }

    #[test]
    fn cache_reuses_results_per_address_port_and_name() {
        let counting = Counting(AtomicUsize::new(0));
        let cache = TlsCache::default();
        let probe = CachedProbe::new(&counting, &cache);
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert_eq!(probe.peer_certificate(ip, 443, "a.test"), Ok(vec![187]));
        assert_eq!(probe.peer_certificate(ip, 443, "a.test"), Ok(vec![187]));
        assert_eq!(probe.peer_certificate(ip, 443, "b.test"), Ok(vec![187]));
        assert_eq!(counting.0.load(Ordering::SeqCst), 2);
    }
}
