#include "wrapper.h"

x264_t *x264_encoder_open_any(x264_param_t *param) {
    return x264_encoder_open(param);
}
