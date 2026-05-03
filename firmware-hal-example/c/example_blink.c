/* example_blink.c — minimal demo firmware using the pcb_sim HAL.
 *
 * Connects to the simulator, toggles PA0 every second, and listens for
 * GPIO events.
 */

#include "pcb_sim_hal.h"
#include <stdio.h>
#include <unistd.h>

int main(int argc, char **argv) {
    const char *socket_path = argc > 1 ? argv[1] : "/tmp/board_sim/mcu1.sock";

    if (pcb_sim_connect(socket_path) != PCB_SIM_OK) {
        fprintf(stderr, "failed to connect to %s\n", socket_path);
        return 1;
    }

    int level = 0;
    for (;;) {
        pcb_sim_gpio_write("PA0", level ? PCB_SIM_PIN_HIGH : PCB_SIM_PIN_LOW);
        level = !level;
        sleep(1);
    }
}
