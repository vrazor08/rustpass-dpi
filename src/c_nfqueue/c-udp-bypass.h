#ifndef C_UDP_BYPASS
#define C_UDP_BYPASS

#include <stdint.h>
#include <stdlib.h>

#define bail(msg) perror(msg); return -1
#define close_bail(fd, msg) close(fd); bail(msg)

const int yes = 1;
struct bypass_data {
  int mark;
  uint16_t queue_num;
  uint8_t fake_ttl;
  char* fake_pkt_payload;
  size_t fake_pkt_payload_len;
};

int init_nfq(struct bypass_data *cb_data, struct nfq_handle **h, struct nfq_q_handle **qh);
void run_nfq(struct nfq_handle *h, char *buf, size_t buf_size);
void destroy_nfq(struct nfq_handle *h, struct nfq_q_handle *qh);

#endif
