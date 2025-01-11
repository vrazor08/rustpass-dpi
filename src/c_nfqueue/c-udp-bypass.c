#include <arpa/inet.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netinet/ip.h>
#include <netinet/udp.h>
#include <unistd.h>

#include <linux/netfilter.h>
#include <libnetfilter_queue/libnetfilter_queue.h>
#include <libnetfilter_queue/libnetfilter_queue_udp.h>
#include <libnetfilter_queue/libnetfilter_queue_ipv4.h>

#include "c-udp-bypass.h"

static uint32_t get_pkt_id (struct nfq_data *tb) {
  int id = 0;
  struct nfqnl_msg_packet_hdr *ph;
  ph = nfq_get_msg_packet_hdr(tb);
  if (ph) id = ntohl(ph->packet_id);
  return id;
}

int send_udp_packet(uint32_t src_ip, uint32_t dst_ip, uint16_t src_port, uint16_t dst_port, struct bypass_data *b_data) {
  int fakefd;
  uint8_t buffer[sizeof(struct iphdr) + sizeof(struct udphdr) + b_data->fake_pkt_payload_len];
  struct iphdr *iph = (struct iphdr *)buffer;
  struct udphdr *udph = (struct udphdr *)(buffer + sizeof(struct iphdr));
  uint8_t *payload = buffer + sizeof(struct iphdr) + sizeof(struct udphdr);
  struct sockaddr_in sin;
  if ((fakefd = socket(AF_INET, SOCK_RAW, IPPROTO_RAW)) < 0) bail("socket");
  if (setsockopt(fakefd, SOL_SOCKET, SO_MARK, &b_data->mark, sizeof(b_data->mark)) < 0)
    close_bail(fakefd, "setsockopt SO_MARK");

  iph->ihl = 5;
  iph->version = 4;
  iph->tos = 0;
  iph->tot_len = htons(sizeof(buffer));
  iph->id = htons(54321);
  iph->frag_off = 0;
  iph->ttl = b_data->fake_ttl;
  iph->protocol = IPPROTO_UDP;
  iph->saddr = src_ip;
  iph->daddr = dst_ip;
  iph->check = 0;
  nfq_ip_set_checksum(iph);

  udph->source = src_port;
  udph->dest = dst_port;
  udph->len = htons(sizeof(struct udphdr) + b_data->fake_pkt_payload_len);
  udph->check = 0;
  memcpy(payload, b_data->fake_pkt_payload, b_data->fake_pkt_payload_len);
  nfq_udp_compute_checksum_ipv4(udph, iph);

  sin.sin_family = AF_INET;
  sin.sin_port = udph->dest;
  sin.sin_addr.s_addr = iph->daddr;

  if (setsockopt(fakefd, IPPROTO_IP, IP_HDRINCL, &yes, sizeof(yes)) < 0) close_bail(fakefd, "setsockopt IP_HDRINCL");
  if (sendto(fakefd, buffer, sizeof(buffer), 0, (struct sockaddr *)&sin, sizeof(sin)) < 0) close_bail(fakefd, "sendto");
  close(fakefd);
  return 0;
}

static int cb(struct nfq_q_handle *qh, struct nfgenmsg *nfmsg, struct nfq_data *nfa, void *data) {
  (void)nfmsg;
  unsigned char *packetData;
  struct bypass_data *cb_data = (struct bypass_data*)data;
  int len = nfq_get_payload(nfa, &packetData);
  if (len >= 0) {
    struct iphdr *ip = (struct iphdr *)packetData;
    if (ip->protocol != IPPROTO_UDP)
      fprintf(stderr, log_msg"Error: it isn't udp packet, maybe there is not iptables rule?\n");
    else {
      struct udphdr *udp = (struct udphdr *)(packetData + (ip->ihl * 4));
      if (send_udp_packet(ip->saddr, ip->daddr, udp->source, udp->dest, cb_data) != 0)
        fprintf(stderr, log_msg"Failed to send UDP packet\n");
      else if (cb_data->log_level == Trace) {
        struct in_addr src_addr, dst_addr;
        src_addr.s_addr = ip->saddr;
        dst_addr.s_addr = ip->daddr;
        char src_ip_str[INET_ADDRSTRLEN], dst_ip_str[INET_ADDRSTRLEN];
        inet_ntop(AF_INET, &src_addr, src_ip_str, INET_ADDRSTRLEN);
        inet_ntop(AF_INET, &dst_addr, dst_ip_str, INET_ADDRSTRLEN);
        printf(log_msg"Sent 64-byte UDP packet from %s:%d to %s:%d\n", src_ip_str, ntohs(udp->source), dst_ip_str, ntohs(udp->dest));
      }
    }
  }
  uint32_t id = get_pkt_id(nfa);
  int ret = nfq_set_verdict(qh, id, NF_ACCEPT, 0, NULL);
  if (cb_data->log_level >= Debug) printf(log_msg"Sent original packet with ret: %d, id: %u\n", ret, id);
  return ret;
}

int init_nfq(struct bypass_data *cb_data, struct nfq_handle **h, struct nfq_q_handle **qh) {
  if (!(*h = nfq_open())) bail("nfq_open");
  // TODO: add ipv6 support
  if (nfq_unbind_pf(*h, AF_INET) < 0) bail("nfq_unbind_pf");
  if (nfq_bind_pf(*h, AF_INET) < 0) bail("nfq_bind_pf");
  if (!(*qh = nfq_create_queue(*h, cb_data->queue_num, &cb, (void*)cb_data))) bail("nfq_create_queue");
  if (nfq_set_mode(*qh, NFQNL_COPY_PACKET, 0xffff) < 0) bail("nfq_set_mode");
  return 0;
}

void run_nfq(struct nfq_handle *h, char *buf, size_t buf_size) {
  int rv;
  int fd = nfq_fd(h);
  setsockopt(fd, SOL_NETLINK, NETLINK_NO_ENOBUFS, &yes, sizeof(int));
  while ((rv = recv(fd, buf, buf_size, 0)) && rv >= 0)
    nfq_handle_packet(h, buf, rv);
}

void destroy_nfq(struct nfq_handle *h, struct nfq_q_handle *qh) {
  if (qh != NULL) nfq_destroy_queue(qh);
  if (h != NULL) nfq_close(h);
}
