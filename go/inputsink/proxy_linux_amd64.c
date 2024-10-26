#include "proxy_linux_amd64.h"

int write_events(const struct libevdev_uinput *uinput, size_t len,
                 event_t events[len]) {
  for (int i = 0; i < len; i++) {
    int ret = libevdev_uinput_write_event(uinput, events[i].type,
                                          events[i].code, events[i].value);
    if (ret != 0) {
      return ret;
    }
  }
  return 0;
}
