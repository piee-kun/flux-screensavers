#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct Flux Flux;

struct Flux *flux_new(float width, float height, float physical_width, float physical_height, const char *settings_json_ptr);

void flux_animate(struct Flux *ptr, float timestamp);

void flux_resize(struct Flux *ptr, float logical_width, float logical_height, float physical_width, float physical_height);

void flux_destroy(struct Flux *ptr);
