#include "underprint.h"

#include <stdio.h>
#include <string.h>

static int contains(const uint8_t *haystack, size_t length, const char *needle) {
    size_t needle_length = strlen(needle);
    if (needle_length > length) {
        return 0;
    }
    for (size_t offset = 0; offset <= length - needle_length; offset++) {
        if (memcmp(haystack + offset, needle, needle_length) == 0) {
            return 1;
        }
    }
    return 0;
}

int main(void) {
    if (up_abi_version() != 1) {
        return 1;
    }

    up_context *context = NULL;
    up_bytes_view empty = {NULL, 0};
    if (up_context_create(empty, &context) != UP_OK || context == NULL) {
        return 2;
    }

    up_result *result = NULL;
    if (up_context_capabilities(context, &result) != UP_OK || result == NULL) {
        return 3;
    }

    up_bytes_view json = up_result_json(result);
    if (json.data == NULL || json.len == 0 || !contains(json.data, json.len, "abi_version")) {
        return 4;
    }

    up_result_free(result);
    up_result_free(result);
    up_context_free(context);
    up_context_free(context);
    puts("Underprint C ABI smoke test passed");
    return 0;
}
