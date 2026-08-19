typedef struct up_context up_context;
typedef struct up_result up_result;
typedef unsigned char uint8_t;
typedef unsigned int uint32_t;
typedef unsigned long long size_t;

typedef struct {
    const uint8_t *data;
    size_t len;
} up_bytes_view;

uint32_t up_abi_version(void);
up_bytes_view up_version(void);
int up_context_create(up_bytes_view config_json, up_context **out);
int up_context_capabilities(up_context *context, up_result **out);
int up_detect(up_context *context, up_bytes_view image, up_bytes_view options_json, up_result **out);
int up_embed(up_context *context, up_bytes_view image, up_bytes_view options_json, up_result **out);
int up_verify(up_context *context, up_bytes_view image, up_bytes_view evidence, up_bytes_view options_json, up_result **out);
up_bytes_view up_result_json(up_result *result);
up_bytes_view up_result_output(up_result *result);
void up_result_free(up_result *result);
void up_context_free(up_context *context);
