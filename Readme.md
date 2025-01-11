# RustPass dpi - DPI bypass

Implementation of some DPI bypass methods **only for linux** inspired by [byedpi](https://github.com/hufrea/byedpi). The program is a local SOCKS4 proxy server.
Usage examples:
```sh
rustpass-dpi 127.0.0.1:6969 -s 1 -f -1 -b 663
sudo rustpass-dpi 127.0.0.1:6969 -s 1 -f -1 -b 663 -U
```

------

## Arguments describing
```
Bypass dpi written in rust inspired by byedpi and zapret.

Rustpass-dpi supports bypassing tls using socks4 proxy and udp using nfqueue and network namespace(if need)

USAGE:
    rustpass-dpi [FLAGS] [OPTIONS] <proxy-addr>

FLAGS:
    -h, --help
            Prints help information

    -U, --udp-desync
            Use udp desync. Warning for it you need to run rustpass-dpi as root. You can also run udp-bypass-helper.sh
            for creating new network namespace. It can be useful when you want to desync udp trafic only for some apps.
    -V, --version
            Prints version information


OPTIONS:
    -b, --buf-size <buf-size>
            TCP buf size [default: 16384]

    -D, --disoob <disoob>...
            Disorder with oob data positions. Can be single number or list of numbers separated by space: -D 2 -1 10 or
            many --disoob arguments: -D 2 -D -1 -D 10
    -d, --disorder <disorder>
            disorder position [default: 0]

    -f, --fake <fake>...
            Split with send fake packets. Can be single number or list of numbers separated by space: -f 2 -1 10 or many
            --fake arguments: -f 2 -f -1 -f 10
    -F, --fake-ttl <fake-ttl>
            TTL for fake packets.

            If you get something like this when connecting: Secure Connection Failed Error code:
            SSL_ERROR_PROTOCOL_VERSION_ALERT then decreasing fake-ttl may help [default: 6]
    -m, --mark <mark>
            Mark for outgoing udp fake packets. Must be the same as in ./udp-bypass-helper.sh BYPASS_MARK env if use
            [default: 12345]
    -n, --nfqueue-num <nfqueue-num>
            Nfqueue num for sending udp fake packets Must be the same as in ./udp-bypass-helper.sh QUEUE_NUM env if use
            [default: 0]
    -o, --oob-data <oob-data>
            Byte sent outside the main stream [default: 97]

    -s, --split <split>...
            Split positions. Can be single number or list of numbers separated by space: -s 2 -1 10 or many --split
            arguments: -s 2 -s -1 -s 10
    -S, --splitoob <splitoob>...
            Split with oob data positions. Can be single number or list of numbers separated by space: -S 2 -1 10 or
            many --splitoob arguments: -S 2 -S -1 -S 10
    -t, --timeout <timeout>
            TCP timeout in secs [default: 0]


ARGS:
    <proxy-addr>
            listen addr in ip:port format
```

------

## How to use

rustpass-dpi runs local SOCKS4 proxy but currently implemented only SOCKS4 without SOCKS4a.
It also doesn't support 0x02 SOCKS4 command = establish a TCP/IP port binding.
But it enough for using.

### Udp bypassing

UDP bypassing is implemented using nfqueue and fake udp packets sending using raw sockets
that's why if you want to use UDP desync you need to run rustpass-dpi as root.
For UDP bypassing you need to create some iptable rule:
```sh
iptables -I OUTPUT -o <interface> -p udp -m mark ! --mark <mark for fake udp packets> -j NFQUEUE --queue-num <nfqueue num>
```
And for each UDP packet sent a fake packet will be sent.

#### Udp bypassing in network namespace

Usually everything was good and you don't need it but you can use linux network namespace for rustpass-dpi and run apps in network namespace.
You can run this apps in this network namespace: `sudo ip netns exec ns1 bash`. This command create root
shell(for your user shell run: `sudo ip netns exec ns1 sudo -u <user_name> bash`) that uses network
namespace ns1 and you can run apps from this shell and there udp traffic will desync.
For it you also need to run rastpass-dpi from this shell: `sudo ip netns exec ns1 rustpass-dpi <args>`.
But it isn't recomended way to run apps in network namespace.
Because shell in network namespace doesn't include all env vars which basic shell has.
It also may not work with NetworkManager.

Recomended way to run apps in network namespace is using something like [firejail](https://github.com/netblue30/firejail/)
For example:
```sh
firejail --netns=ns1 --name=desync_discord discord --proxy-server="socks4://127.0.0.1:6969"
```
But firejail also has some limitations for example it can not run snap and flatpak apps.

------

## Building

You need to install `libnetfilter_queue` this is used for udp desync and then build rustpass-dpi:
```sh
cargo build --release
```
Or you can compile with:
```sh
cargo build --release --no-default-features
```
But you can not use udp desync.
