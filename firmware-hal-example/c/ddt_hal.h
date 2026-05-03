/* pcb_sim_hal.h — reference C HAL for pcb-sim.
 *
 * Single-threaded, blocking. Intended as a reference; production HALs
 * may want async I/O, callback-driven event handling, or transport
 * abstractions for running on real hardware.
 *
 * Wire protocol: see ../../docs/wire-protocol.md.
 */

#ifndef PCB_SIM_HAL_H
#define PCB_SIM_HAL_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    PCB_SIM_PIN_LOW  = 0,
    PCB_SIM_PIN_HIGH = 1,
    PCB_SIM_PIN_Z    = 2,
} pcb_sim_pin_value_t;

typedef enum {
    PCB_SIM_OK            = 0,
    PCB_SIM_ERR_IO        = -1,
    PCB_SIM_ERR_PROTOCOL  = -2,
    PCB_SIM_ERR_NACK      = -3,
    PCB_SIM_ERR_TIMEOUT   = -4,
} pcb_sim_result_t;

/* Connect and perform the hello handshake. Returns PCB_SIM_OK on success. */
pcb_sim_result_t pcb_sim_connect(const char *socket_path);

/* Disconnect cleanly. */
void pcb_sim_disconnect(void);

/* I2C operations. */
pcb_sim_result_t pcb_sim_i2c_write(const char *bus, uint8_t address,
                           const uint8_t *data, size_t len);
pcb_sim_result_t pcb_sim_i2c_read (const char *bus, uint8_t address,
                           uint8_t *buf, size_t len);
pcb_sim_result_t pcb_sim_i2c_write_read(const char *bus, uint8_t address,
                                const uint8_t *write_data, size_t write_len,
                                uint8_t *read_buf, size_t read_len);

/* GPIO operations. */
pcb_sim_result_t pcb_sim_gpio_write(const char *pin, pcb_sim_pin_value_t value);
pcb_sim_result_t pcb_sim_gpio_read (const char *pin, pcb_sim_pin_value_t *out_value);

/* Event polling. Returns 1 if an event was retrieved, 0 if no event,
 * negative on error. */
typedef enum {
    PCB_SIM_EVENT_GPIO,
    PCB_SIM_EVENT_UART_RX,
    PCB_SIM_EVENT_UART_OVERFLOW,
    PCB_SIM_EVENT_SIM,
} pcb_sim_event_kind_t;

typedef struct {
    pcb_sim_event_kind_t kind;
    uint64_t sim_time_us;
    /* TODO: union of per-event payloads. */
} pcb_sim_event_t;

int pcb_sim_poll_event(pcb_sim_event_t *out_event, int timeout_ms);

#ifdef __cplusplus
}
#endif

#endif /* PCB_SIM_HAL_H */
