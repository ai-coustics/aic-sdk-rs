/* Bindgen input wrapper.
 *
 * `aic.h` is vendored verbatim from the SDK release and is checked against it by the
 * `check-header` CI job, so it must not be edited. This wrapper includes it and adds the one
 * symbol that ships in `libaic` but is intentionally absent from the public header. Pointing
 * bindgen here lets every linking mode (static, dynamic, runtime) get the declaration uniformly.
 */
#include <stdint.h>
#include "aic.h"

/* Sets the SDK wrapper ID. Present in the library, omitted from the public `aic.h`. */
void aic_set_sdk_wrapper_id(uint32_t id);
