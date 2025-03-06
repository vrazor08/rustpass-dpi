# RustPass DPI - DPI Bypass Tool

**RustPass DPI** is a Rust-based tool for bypassing Deep Packet Inspection (DPI) on Linux systems. Inspired by [byedpi](https://github.com/hufrea/byedpi), RustPass DPI functions as a local SOCKS4 proxy server, enabling users to circumvent network restrictions and censorship.

## Features

- **SOCKS4 Proxy Server**: Provides a local SOCKS4 proxy for routing traffic.
- **UDP Bypass**: Utilizes `nfqueue` and raw sockets for handling UDP traffic.
- **Network Namespace Support**: Allows isolation of UDP bypassing.
- **Customizable Parameters**: Offers various options to fine-tune bypass behavior.

------

## Arguments describing
```sh
rustpass-dpi --help
```
```
rustpass-dpi 0.1.1
Bypass dpi written in rust inspired by byedpi and zapret.

Rustpass-dpi supports bypassing tls using socks4 proxy and udp using nfqueue and network namespace(if need)

USAGE:
    rustpass-dpi [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help
            Prints help information

    -V, --version
            Prints version information


OPTIONS:
    -r, --run-app <run-app>
            Experimental. Run app with rustpass-dpi. It makes sense only with --netns option. To use this option you
            need to set suid bit. If you use this option you don't to run rustpass-dpi with sudo

SUBCOMMANDS:
    help    Prints this message or the help of the given subcommand(s)
    tcp     Use to specify tcp desync options
    udp     Use to specify udp desync options and network namespace

```

```sh
rustpass-dpi tcp --help
```
```
rustpass-dpi-tcp 0.1.1
Use to specify tcp desync options

Warning: If you use options that expect a list of args, such as: --split, you need to put a dot at the end if the next
arg is a udp subcommand, for example: --split 2 -1 10 . udp -N ns1

USAGE:
    rustpass-dpi tcp [OPTIONS] <proxy-addr> [SUBCOMMAND]

FLAGS:
    -h, --help
            Prints help information

    -V, --version
            Prints version information


OPTIONS:
    -b, --buf-size <buf-size>
            TCP buf size [default: 16384]

    -D, --disoob <disoob>...
            Disorder with oob data positions. Can be single number or list of numbers separated by space: -D 2 -1 10 or
            many --disoob arguments: -D 2 -D -1 -D 10
    -d, --disorder <disorder>
            disorder position

    -f, --fake <fake>...
            Split with send fake packets. Can be single number or list of numbers separated by space: -f 2 -1 10 or many
            --fake arguments: -f 2 -f -1 -f 10
    -F, --fake-ttl <fake-ttl>
            TTL for fake packets.

            If you get something like this when connecting: Secure Connection Failed Error code:
            SSL_ERROR_PROTOCOL_VERSION_ALERT decreasing fake-ttl may help [default: 6]
    -o, --oob-data <oob-data>
            Byte sent outside the main stream [default: 97]

    -s, --split <split>...
            Split positions. Can be single number or list of numbers separated by space: -s 2 -1 10 or many --split
            arguments: -s 2 -s -1 -s 10
    -S, --splitoob <splitoob>...
            Split with oob data positions. Can be single number or list of numbers separated by space: -S 2 -1 10 or
            many --splitoob arguments: -S 2 -S -1 -S 10
    -t, --timeout <timeout>
            TCP timeout in secs


ARGS:
    <proxy-addr>
            listen addr in ip:port format


SUBCOMMANDS:
    help    Prints this message or the help of the given subcommand(s)
    udp     Use to specify udp desync options and network namespace

```

```sh
rustpass-dpi udp --help
```
```
rustpass-dpi-udp 0.1.1
Use to specify udp desync options and network namespace

Warning: for all of these options you need to be a root

USAGE:
    rustpass-dpi udp [OPTIONS] [SUBCOMMAND]

FLAGS:
    -h, --help
            Prints help information

    -V, --version
            Prints version information


OPTIONS:
    -F, --fake-ttl <fake-ttl>
            TTL for udp fake packets [default: 6]

    -m, --mark <mark>
            Mark for outgoing udp fake packets. Must be the same as in ./udp-bypass-helper.sh BYPASS_MARK env if use
            [default: 12345]
    -N, --netns <netns>
            Experimental. Run rustpass-dpi in a named, persistent network namespace

    -n, --nfqueue-num <nfqueue-num>
            Nfqueue num for sending udp fake packets Must be the same as in ./udp-bypass-helper.sh QUEUE_NUM env if use
            [default: 0]

SUBCOMMANDS:
    help    Prints this message or the help of the given subcommand(s)
    tcp     Use to specify tcp desync options

```
------

## Installation

### Prerequisites

- **libnetfilter_queue**: Required for UDP desynchronization.

On Debian-based systems, install `libnetfilter_queue` with:
```sh
sudo apt-get install libnetfilter-queue-dev
```

pacman:
```sh
sudo pacman -S libnetfilter_queue
```

### Building RustPass DPI

Clone the repository and build the project using Cargo:
```sh
git clone https://github.com/vrazor08/rustpass-dpi.git
cd rustpass-dpi
cargo install --path .
```

To build without UDP desynchronization support:
```sh
cargo install --path . --no-default-features
```
> [!IMPORTANT]
> Building without default features disables UDP desync functionality.

## Usage

RustPass DPI runs a local SOCKS4 proxy server. Below are some usage examples:

```sh
rustpass-dpi 127.0.0.1:6969 tcp -s 1 -f -1 -b 663
```

To enable UDP desynchronization (requires root privileges):

```sh
sudo rustpass-dpi tcp 127.0.0.1:6969 -b 663 -s 1 -f -1 . udp -m 12345 -n 0
```

## UDP Bypassing

UDP bypassing is implemented using `nfqueue` and fake UDP packets sent via raw sockets. To utilize UDP desynchronization:

1. **Run RustPass DPI as Root with udp subcommand**:
```sh
sudo rustpass-dpi tcp <args> udp <args>
```

2. **Set Up iptables Rule**:
Replace `<interface>`, `<mark>`, and `<nfqueue_num>` with appropriate values.
```sh
sudo iptables -I OUTPUT -o <interface> -p udp -m mark ! --mark <mark> -j NFQUEUE --queue-num <nfqueue_num>
```

This rule directs matching UDP packets to the specified NFQUEUE, where RustPass DPI can process them.

3. **Fake Packet Handling**:
For each UDP packet sent, a corresponding fake packet will be dispatched to aid in bypassing DPI.

### UDP Bypassing with Network Namespace

Isolating UDP bypassing within a network namespace can prevent interference with other applications.

1. **Create and Manage Network Namespace**:

Use `udp-bypass-helper.sh` to create, set up, or delete a network namespace.

```sh
./udp-bypass-helper.sh --help
```

2. **Run RustPass DPI in Namespace**:

```sh
sudo rustpass-dpi udp --netns <namespace>
```

*Alternatively:*

```sh
sudo ip netns exec <namespace> rustpass-dpi <args>
```

3. **Run Applications within Namespace**:

To run an application within the network namespace:

```sh
sudo ip netns exec <namespace> <application>
```

**Recommended**: Use tools like [Firejail](https://github.com/netblue30/firejail/) for better environment handling and dirs mount.

**Example**:

```sh
firejail --netns=<namespace> --name=discord_app discord --proxy-server="socks4://127.0.0.1:6969"
```

*Note*: Firejail may have limitations with certain application formats like Snap or Flatpak.

Or compile with --features suid, then set executable owner as root and set suid bit:
```sh
cargo install --path . --features suid
sudo chown root:root rustpass-dpi
sudo chmod 4755 rustpass-dpi
```
And then run app in --run-app option:
```sh
rustpass-dpi -r "discord --proxy-server='socks4://127.0.0.1:6969'" tcp 127.0.0.1:6969 -s 1 -f -1 -b 663 udp --netns ns1 --mark 12345 --nfqueue-num 0
```

## License

This project is licensed under the [MIT License](https://github.com/vrazor08/rustpass-dpi/blob/master/LICENSE).
