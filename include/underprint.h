#ifndef UNDERPRINT_H
#define UNDERPRINT_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct up_context up_context;
typedef struct up_result up_result;

typedef struct {
    const uint8_t *data;
    size_t len;
} up_bytes_view;

typedef enum {
    UP_OK = 0,
    UP_NOT_DETECTED = 1,
    UP_INVALID_ARGUMENT = 2,
    UP_INVALID_INPUT = 3,
    UP_UNAVAILABLE = 4,
    UP_UNTRUSTED_EVIDENCE = 5,
    UP_RESOURCE_LIMIT = 6,
    UP_INTERNAL = 10
} up_status;

uint32_t up_abi_version(void);
up_bytes_view up_version(void);

up_status up_context_create(up_bytes_view config_json, up_context **out);
up_status up_context_capabilities(up_context *context, up_result **out);

up_status up_detect(
    up_context *context,
    up_bytes_view image,
    up_bytes_view options_json,
    up_result **out
);

up_status up_embed(
    up_context *context,
    up_bytes_view image,
    up_bytes_view options_json,
    up_result **out
);

up_status up_verify(
    up_context *context,
    up_bytes_view image,
    up_bytes_view evidence,
    up_bytes_view options_json,
    up_result **out
);

/* Views borrow memory owned by result and remain valid until up_result_free.
 * Callers must not free the same result concurrently with a view copy. */
up_bytes_view up_result_json(up_result *result);
up_bytes_view up_result_output(up_result *result);

/* Free functions tolerate NULL, invalid, stale, and repeated handles. */
void up_result_free(up_result *result);
void up_context_free(up_context *context);

#ifdef __cplusplus
}
#endif

#endif
