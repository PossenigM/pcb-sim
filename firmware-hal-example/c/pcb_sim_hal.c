/* pcb_sim_hal.c — reference C HAL implementation skeleton.
 *
 * TODO: depends on msgpack-c. Add to your build:
 *   sudo apt install libmsgpack-c-dev
 *   gcc -lmsgpackc your_firmware.c pcb_sim_hal.c -o firmware
 *
 * Implementation notes:
 *   - Single-threaded blocking model.
 *   - Maintain a request_id counter.
 *   - For each request: serialize, write framed message, read frames in
 *     a loop until one with matching id arrives. Queue any events seen
 *     in the meantime.
 *   - Frame format: 4-byte LE length, then MessagePack body. See
 *     docs/wire-protocol.md.
 */

#include "pcb_sim_hal.h"

/* TODO: include msgpack-c headers. */
/* #include <msgpack.h> */

#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

static int g_fd = -1;
static int64_t g_next_id = 1;

pcb_sim_result_t pcb_sim_connect(const char *socket_path) {
    /* TODO:
     *   1. socket(AF_UNIX, SOCK_STREAM, 0)
     *   2. connect to socket_path
     *   3. send hello message (protocol_version=1)
     *   4. read response, verify hello_ack
     */
    (void)socket_path;
    return PCB_SIM_ERR_IO;
}

void pcb_sim_disconnect(void) {
    if (g_fd >= 0) {
        close(g_fd);
        g_fd = -1;
    }
}

pcb_sim_result_t pcb_sim_i2c_write(const char *bus, uint8_t address,
                           const uint8_t *data, size_t len) {
    /* TODO: build MessagePack i2c_write, send, await i2c_ack. */
    (void)bus; (void)address; (void)data; (void)len;
    return PCB_SIM_ERR_IO;
}

pcb_sim_result_t pcb_sim_i2c_read(const char *bus, uint8_t address,
                          uint8_t *buf, size_t len) {
    /* TODO: build i2c_read, send, await i2c_data, copy to buf. */
    (void)bus; (void)address; (void)buf; (void)len;
    return PCB_SIM_ERR_IO;
}

pcb_sim_result_t pcb_sim_i2c_write_read(const char *bus, uint8_t address,
                                const uint8_t *write_data, size_t write_len,
                                uint8_t *read_buf, size_t read_len) {
    /* TODO. */
    (void)bus; (void)address;
    (void)write_data; (void)write_len;
    (void)read_buf; (void)read_len;
    return PCB_SIM_ERR_IO;
}

pcb_sim_result_t pcb_sim_gpio_write(const char *pin, pcb_sim_pin_value_t value) {
    /* TODO. */
    (void)pin; (void)value;
    return PCB_SIM_ERR_IO;
}

pcb_sim_result_t pcb_sim_gpio_read(const char *pin, pcb_sim_pin_value_t *out_value) {
    /* TODO. */
    (void)pin; (void)out_value;
    return PCB_SIM_ERR_IO;
}

int pcb_sim_poll_event(pcb_sim_event_t *out_event, int timeout_ms) {
    /* TODO:
     *   - First check the in-process event queue (built up by previous
     *     reads while waiting for request responses).
     *   - Otherwise select() on g_fd with timeout_ms.
     *   - Read any pending frames; classify each as response (ignore
     *     for poll) or event (return).
     */
    (void)out_event; (void)timeout_ms;
    return -1;
}

/* TODO: helpers
 *   static int write_all(int fd, const void *buf, size_t len);
 *   static int read_exact(int fd, void *buf, size_t len);
 *   static int send_frame(const uint8_t *body, size_t len);
 *   static int recv_frame(uint8_t **out_body, size_t *out_len);
 */
