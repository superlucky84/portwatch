//! 루프백 TLS 확인으로 받은 인증서(DER)를 읽는다. docs/DESIGN.md "도메인 연결" 4.

use x509_parser::extensions::GeneralName;
use x509_parser::prelude::{FromDer, X509Certificate};
use x509_parser::x509::X509Name;

use crate::snapshot::Cert;

/// 만료가 가깝다고 경고하는 기간(밀리초).
pub const EXPIRES_SOON_MS: u64 = 14 * 24 * 60 * 60 * 1000;

/// 인증서를 읽어 `name`으로 접속했을 때의 경고를 붙인다.
pub fn describe(der: &[u8], name: &str, now_ms: u64) -> Cert {
    let cert = match X509Certificate::from_der(der) {
        Ok((_, cert)) => cert,
        Err(_) => return failed("인증서 형식을 읽지 못했습니다."),
    };
    let sans: Vec<String> = cert
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|ext| {
            ext.value
                .general_names
                .iter()
                .filter_map(|general| match general {
                    GeneralName::DNSName(dns) => Some(dns.to_ascii_lowercase()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let issuer_org = first_attr(cert.issuer(), |n| n.iter_organization());
    let issuer = issuer_org
        .clone()
        .or_else(|| first_attr(cert.issuer(), |n| n.iter_common_name()))
        .unwrap_or_default();
    let not_after_ms = u64::try_from(cert.validity().not_after.timestamp())
        .ok()
        .map(|secs| secs * 1000);

    let mut warnings = Vec::new();
    match not_after_ms {
        Some(end) if end <= now_ms => warnings.push("인증서가 만료됐습니다.".to_owned()),
        Some(end) if end - now_ms <= EXPIRES_SOON_MS => {
            warnings.push("인증서가 14일 안에 만료됩니다.".to_owned())
        }
        _ => {}
    }
    if !sans.iter().any(|san| san_matches(san, name)) {
        warnings.push(format!("인증서에 이 주소 이름({name})이 없습니다."));
    }

    Cert {
        subject: first_attr(cert.subject(), |n| n.iter_common_name()).unwrap_or_default(),
        sans,
        mkcert: issuer_org.is_some_and(|org| org.starts_with("mkcert")),
        issuer,
        not_after_ms,
        warnings,
        error: None,
    }
}

/// 읽지 못한 경우의 값.
pub fn failed(reason: &str) -> Cert {
    Cert {
        error: Some(reason.to_owned()),
        ..Cert::default()
    }
}

/// SAN 이름이 접속 이름과 맞는지. `*.example.test`는 한 단계 하위 이름과만 맞는다.
pub fn san_matches(san: &str, name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    match san.strip_prefix("*.") {
        Some(base) => name
            .split_once('.')
            .is_some_and(|(label, rest)| !label.is_empty() && rest == base),
        None => san == name,
    }
}

fn first_attr<'a, I>(name: &'a X509Name<'a>, iter: impl Fn(&'a X509Name<'a>) -> I) -> Option<String>
where
    I: Iterator<Item = &'a x509_parser::x509::AttributeTypeAndValue<'a>>,
{
    iter(name)
        .find_map(|attr| attr.as_str().ok())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{date_time_ymd, CertificateParams, DistinguishedName, DnType, KeyPair};

    const DAY_MS: u64 = 24 * 60 * 60 * 1000;
    // 2026-11-19 00:00:00 UTC
    const NOW_MS: u64 = 1_795_046_400_000;

    fn make(names: &[&str], issuer_org: &str, until: (i32, u8, u8)) -> Vec<u8> {
        let ca_key = KeyPair::generate().unwrap();
        let mut ca = CertificateParams::new(Vec::<String>::new()).unwrap();
        ca.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let mut ca_name = DistinguishedName::new();
        ca_name.push(DnType::OrganizationName, issuer_org);
        ca_name.push(DnType::CommonName, format!("{issuer_org} root"));
        ca.distinguished_name = ca_name;
        let issuer = rcgen::Issuer::new(ca, ca_key);

        let key = KeyPair::generate().unwrap();
        let mut params =
            CertificateParams::new(names.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>())
                .unwrap();
        let mut subject = DistinguishedName::new();
        subject.push(DnType::CommonName, names[0]);
        params.distinguished_name = subject;
        params.not_before = date_time_ymd(2026, 1, 1);
        params.not_after = date_time_ymd(until.0, until.1, until.2);
        params.signed_by(&key, &issuer).unwrap().der().to_vec()
    }

    #[test]
    fn reads_an_mkcert_style_certificate() {
        let der = make(
            &["shop.example.test"],
            "mkcert development CA",
            (2028, 2, 19),
        );
        let cert = describe(&der, "shop.example.test", NOW_MS);
        assert_eq!(cert.subject, "shop.example.test");
        assert_eq!(cert.sans, ["shop.example.test"]);
        assert_eq!(cert.issuer, "mkcert development CA");
        assert!(cert.mkcert);
        assert_eq!(cert.not_after_ms, Some(1_834_531_200_000));
        assert!(cert.warnings.is_empty(), "{:?}", cert.warnings);
        assert_eq!(cert.error, None);
    }

    #[test]
    fn warns_about_expiry_and_names() {
        let soon = make(&["*.example.test"], "Example Test CA", (2026, 11, 30));
        let cert = describe(&soon, "shop.example.test", NOW_MS);
        assert!(!cert.mkcert);
        assert_eq!(cert.warnings, ["인증서가 14일 안에 만료됩니다."]);

        let cert = describe(&soon, "a.shop.example.test", NOW_MS);
        assert_eq!(
            cert.warnings,
            [
                "인증서가 14일 안에 만료됩니다.",
                "인증서에 이 주소 이름(a.shop.example.test)이 없습니다."
            ]
        );

        let expired = make(&["shop.example.test"], "Example Test CA", (2026, 6, 1));
        let cert = describe(&expired, "shop.example.test", NOW_MS + DAY_MS);
        assert_eq!(cert.warnings, ["인증서가 만료됐습니다."]);
    }

    #[test]
    fn broken_der_is_an_error() {
        let cert = describe(b"not a certificate", "shop.example.test", NOW_MS);
        assert_eq!(
            cert.error.as_deref(),
            Some("인증서 형식을 읽지 못했습니다.")
        );
        assert!(cert.sans.is_empty());
    }

    #[test]
    fn wildcards_cover_one_label() {
        assert!(san_matches("*.example.test", "shop.example.test"));
        assert!(san_matches("*.example.test", "SHOP.example.test"));
        assert!(!san_matches("*.example.test", "example.test"));
        assert!(!san_matches("*.example.test", "a.b.example.test"));
        assert!(san_matches("localhost", "localhost"));
    }
}
