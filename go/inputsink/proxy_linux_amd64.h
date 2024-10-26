#ifndef PROXY
#define PROXY

#include <libevdev/libevdev-uinput.h>
#include <libevdev/libevdev.h>
#include <linux/input.h>

typedef struct {
  unsigned int type;
  unsigned int code;
  int value;
} event_t;

int write_events(const struct libevdev_uinput *uinput, size_t len,
                 event_t events[len]);

#endif
