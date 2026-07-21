//! Política de Membrana: listen defaults e ordenação de seeds.

use libp2p::Multiaddr;
use mycelium_core::Membrane;

/// Listen padrão conforme membrana (sem STUN / sem UPnP).
pub fn default_listen_addrs(membrane: Membrane, has_global_ip6: bool) -> Vec<Multiaddr> {
    let parse = |s: &str| s.parse::<Multiaddr>().expect("multiaddr estático");
    match membrane {
        Membrane::Folha => {
            let mut v = vec![parse("/ip4/127.0.0.1/tcp/0")];
            if has_global_ip6 {
                v.insert(0, parse("/ip6/::/tcp/0"));
                v.insert(0, parse("/ip6/::/udp/0/quic-v1"));
            }
            v.push(parse("/ip4/127.0.0.1/udp/0/quic-v1"));
            v
        }
        Membrane::Floresta => vec![
            parse("/ip6/::/udp/0/quic-v1"),
            parse("/ip6/::/tcp/0"),
            // IPv4 só loopback — floresta não finge NAT
            parse("/ip4/127.0.0.1/udp/0/quic-v1"),
            parse("/ip4/127.0.0.1/tcp/0"),
        ],
        Membrane::Raiz | Membrane::Esporocarp => vec![
            parse("/ip6/::/udp/0/quic-v1"),
            parse("/ip6/::/tcp/0"),
            parse("/ip4/0.0.0.0/udp/0/quic-v1"),
            parse("/ip4/0.0.0.0/tcp/0"),
        ],
    }
}

/// Prioridade de dial para seed (menor = primeiro). `None` = legado sem flag.
/// Retorna `None` se o seed não deve ser dialado (folha remota).
pub fn seed_dial_rank(local: Membrane, remote: Option<Membrane>) -> Option<u8> {
    if matches!(remote, Some(Membrane::Folha)) {
        return None;
    }
    let rank = match (local, remote) {
        // Folha: esporocarp primeiro (ponte), depois floresta, raiz, legado
        (Membrane::Folha, Some(Membrane::Esporocarp)) => 0,
        (Membrane::Folha, Some(Membrane::Floresta)) => 1,
        (Membrane::Folha, Some(Membrane::Raiz)) => 2,
        (Membrane::Folha, None) => 3,
        // Demais: floresta primeiro
        (_, Some(Membrane::Floresta)) => 0,
        (_, Some(Membrane::Esporocarp)) => 1,
        (_, Some(Membrane::Raiz)) => 2,
        (_, None) => 3,
        (_, Some(Membrane::Folha)) => 255, // unreachable
    };
    Some(rank)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mycelium_core::diagnose_membrane;

    #[test]
    fn diagnose_matrix() {
        assert_eq!(
            diagnose_membrane(false, None, true, None, false),
            Membrane::Esporocarp
        );
        assert_eq!(
            diagnose_membrane(true, Some("1.2.3.4"), false, None, false),
            Membrane::Floresta
        );
        assert_eq!(
            diagnose_membrane(false, Some("1.2.3.4"), false, None, false),
            Membrane::Raiz
        );
        assert_eq!(
            diagnose_membrane(false, None, false, None, false),
            Membrane::Folha
        );
        assert_eq!(
            diagnose_membrane(true, None, false, Some(Membrane::Raiz), false),
            Membrane::Raiz
        );
        assert_eq!(
            diagnose_membrane(true, None, false, None, true),
            Membrane::Esporocarp
        );
    }

    #[test]
    fn folha_skips_remote_folha() {
        assert!(seed_dial_rank(Membrane::Floresta, Some(Membrane::Folha)).is_none());
        assert_eq!(
            seed_dial_rank(Membrane::Folha, Some(Membrane::Esporocarp)),
            Some(0)
        );
    }

    #[test]
    fn folha_listen_is_loopback_v4() {
        let addrs = default_listen_addrs(Membrane::Folha, false);
        assert!(addrs.iter().all(|a| {
            let s = a.to_string();
            s.contains("127.0.0.1") || s.contains("/ip6/")
        }));
        assert!(addrs.iter().any(|a| a.to_string().contains("127.0.0.1")));
        assert!(!addrs.iter().any(|a| a.to_string().contains("0.0.0.0")));
    }
}
