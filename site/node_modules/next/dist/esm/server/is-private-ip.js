import ipaddr from 'next/dist/compiled/ipaddr.js';
export function isPrivateIp(ip) {
    if (ip.startsWith('[') && ip.endsWith(']')) {
        ip = ip.slice(1, -1);
    }
    if (!ipaddr.isValid(ip)) {
        return false;
    }
    try {
        let addr = ipaddr.parse(ip);
        if (addr.kind() === 'ipv6' && addr.isIPv4MappedAddress()) {
            addr = addr.toIPv4Address();
        }
        const range = addr.range();
        return range !== 'unicast';
    } catch (e) {
        return false;
    }
}

//# sourceMappingURL=is-private-ip.js.map