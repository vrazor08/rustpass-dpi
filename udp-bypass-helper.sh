#!/usr/bin/bash

set -e
if [ -f "/tmp/IP_FORWARD_BEFORE" ]; then
  IP_FORWARD_BEFORE=$(cat /tmp/IP_FORWARD_BEFORE)
else
  IP_FORWARD_BEFORE=$(sysctl net.ipv4.ip_forward --values)
fi

prepare_ns() {
  local mark=$1
  local queue_num=$2
  ip netns add ns1
  ip netns exec ns1 ip link set lo up
  ip link add br0 type bridge
  ip link set br0 up
  ip addr add 17.0.0.1/24 dev br0
  ip link add veth0 type veth peer name ceth0
  ip link set veth0 master br0
  ip link set veth0 up
  ip link set ceth0 netns ns1
  ip netns exec ns1 ip link set ceth0 up
  ip netns exec ns1 ip addr add 17.0.0.10/24 dev ceth0
  ip netns exec ns1 ip route add default via 17.0.0.1
  sysctl -w net.ipv4.ip_forward=1
  iptables -t nat -A POSTROUTING -s 17.0.0.0/24 ! -o br0 -j MASQUERADE
  ip netns exec ns1 iptables -I OUTPUT -o ceth0 -p udp -m mark ! --mark $mark -j NFQUEUE --queue-num $queue_num
  echo $IP_FORWARD_BEFORE > /tmp/IP_FORWARD_BEFORE
}

# FIXME: uncorrect deleting
del_ns() {
  set +e
  ip netns pids ns1 | xargs kill
  set -e
  ip link set veth0 down
  ip link delete veth0
  ip netns exec ns1 ip link set lo down
  ip netns delete ns1
  ip link set br0 down
  ip link delete br0
  sysctl -w net.ipv4.ip_forward=$IP_FORWARD_BEFORE
}

if [ $(id -u) -ne 0 ]; then
  echo "Please run this script as root or using sudo!"
  exit 1
fi

case $1 in
  "help" | "--help" | "-h")
    echo "--prepare-ns - (env required BYPASS_MARK(num), QUEUE_NUM(num)) prepare network namespace for bypassing handling udp"
    echo "--del-ns - delete network namespace"
    echo "help for this message"
    echo -e "\n\nShell in network namespace doesn't include all env vars which basic shell has."
    echo "It's import because apps in this shell don't see micro and headphones."
    echo "To fix it in piperwire you need to set env XDG_RUNTIME_DIR: export XDG_RUNTIME_DIR=/run/user/\$(id -u)"
    ;;
  "--prepare-ns" | "-p")
    if [[ ! -v BYPASS_MARK ]]; then
      echo "env BYPASS_MARK(num) must be set"
      exit 1
    elif [[ ! -v QUEUE_NUM ]]; then
      echo "env QUEUE_NUM(num) must be set"
      exit 1
    else
      prepare_ns "$BYPASS_MARK" "$QUEUE_NUM"
    fi
    echo "ns1 was created successful"
    ;;
  "--del-ns" | "-d")
    del_ns
    echo "ns1 was deleted successful"
    ;;
  *)
    echo "Usage: $0 <command>"
    echo "$0 help - for more info"
    exit 1
    ;;
esac
