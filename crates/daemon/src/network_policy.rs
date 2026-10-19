//! Network scope is an additional filter, never a substitute for pinned TLS.
use anyhow::{bail, Context, Result};
use if_addrs::IfAddr;
use std::net::IpAddr;

fn same_subnet(peer: IpAddr, local: IpAddr, mask: IpAddr) -> bool {
    match (peer, local, mask) {
        (IpAddr::V4(p), IpAddr::V4(l), IpAddr::V4(m)) => {
            let m = u32::from(m);
            m != 0 && (!m).wrapping_add(1).is_power_of_two() && u32::from(p) & m == u32::from(l) & m
        }
        (IpAddr::V6(p), IpAddr::V6(l), IpAddr::V6(m)) => {
            let m = u128::from(m);
            m != 0
                && (!m).wrapping_add(1).is_power_of_two()
                && u128::from(p) & m == u128::from(l) & m
        }
        _ => false,
    }
}

pub fn check(peer: IpAddr, same_subnet_only: bool) -> Result<()> {
    if peer.is_unspecified()
        || peer.is_multicast()
        || matches!(peer, IpAddr::V4(ip) if ip.is_broadcast())
    {
        bail!("请输入单台电脑的有效 IP，不能使用广播、组播或未指定地址");
    }
    // Loopback never reaches another host; retained for isolated protocol tests.
    if !same_subnet_only || peer.is_loopback() {
        return Ok(());
    }
    let interfaces = if_addrs::get_if_addrs().context("无法读取网卡，不能验证同子网连接限制")?;
    if interfaces
        .into_iter()
        .filter(|i| i.is_oper_up())
        .any(|i| match i.addr {
            IfAddr::V4(a) => same_subnet(peer, a.ip.into(), a.netmask.into()),
            IfAddr::V6(a) => same_subnet(peer, a.ip.into(), a.netmask.into()),
        })
    {
        return Ok(());
    }
    bail!("对方不在本机网卡的子网内。若确需通过路由或 VPN 连接，请在设置 → 连接与高级关闭“仅同子网连接”并保存；配对认证仍然必需")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ip(value: &str) -> IpAddr {
        value.parse().unwrap()
    }
    #[test]
    fn scope_uses_real_masks_not_the_first_two_octets() {
        assert!(same_subnet(
            ip("192.168.1.5"),
            ip("192.168.1.20"),
            ip("255.255.255.0")
        ));
        assert!(!same_subnet(
            ip("192.168.2.5"),
            ip("192.168.1.20"),
            ip("255.255.255.0")
        ));
        assert!(same_subnet(ip("10.3.9.1"), ip("10.1.2.3"), ip("255.0.0.0")));
        assert!(!same_subnet(ip("10.3.9.1"), ip("10.1.2.3"), ip("0.0.0.0")));
        assert!(!same_subnet(
            ip("10.1.2.4"),
            ip("10.1.2.3"),
            ip("255.0.255.0")
        ));
        assert!(same_subnet(
            ip("fd01::2"),
            ip("fd01::1"),
            ip("ffff:ffff:ffff:ffff::")
        ));
        assert!(!same_subnet(
            ip("fd02::2"),
            ip("fd01::1"),
            ip("ffff:ffff:ffff:ffff::")
        ));
    }
    #[test]
    fn invalid_destinations_are_rejected_even_with_scope_disabled() {
        for address in ["0.0.0.0", "255.255.255.255", "224.0.0.1", "::", "ff02::1"] {
            assert!(check(ip(address), false).is_err());
        }
        assert!(check(ip("127.0.0.1"), true).is_ok());
    }
}
