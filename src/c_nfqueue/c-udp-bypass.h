#ifndef C_UDP_BYPASS
#define C_UDP_BYPASS

#include <stdint.h>
#include <stdlib.h>

#define bail(msg) perror(msg); return -1
#define close_bail(fd, msg) close(fd); bail(msg)

const int yes = 1;
typedef enum {
  /// A level lower than all log levels.
  Off,
  /// Corresponds to the `Error` log level.
  Error,
  /// Corresponds to the `Warn` log level.
  Warn,
  /// Corresponds to the `Info` log level.
  Info,
  /// Corresponds to the `Debug` log level.
  Debug,
  /// Corresponds to the `Trace` log level.
  Trace,
} LevelFilter;

#define STRINGIFY(x) #x
#define TOSTRING(x) STRINGIFY(x)
#define log_msg "[" __FILE__ ":" TOSTRING(__LINE__) "] "

struct bypass_data {
  int mark;
  uint16_t queue_num;
  uint8_t fake_ttl;
  char* fake_pkt_payload;
  size_t fake_pkt_payload_len;
  int log_level;
};

int init_nfq(struct bypass_data *cb_data, struct nfq_handle **h, struct nfq_q_handle **qh);
void run_nfq(struct nfq_handle *h, char *buf, size_t buf_size);
void destroy_nfq(struct nfq_handle *h, struct nfq_q_handle *qh);

#endif
