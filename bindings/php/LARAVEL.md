# Laravel queue and Octane lifecycle

Create one `Underprint\Native` singleton only in long-lived queue or Octane
workers. Do not resolve it in every PHP-FPM request. Configure the shared
library and model directory explicitly; never derive them from request input.

```php
$this->app->singleton(Underprint\Native::class, fn () => Underprint\Native::load(
    modelsDirectory: config('services.underprint.models'),
    libraryPath: config('services.underprint.library'),
));
```

Call `close()` from the worker termination hook after jobs have stopped. Do not
close the singleton while another coroutine or job is inside a native call.
Underprint calls are blocking, so Octane applications should dispatch model work
to a bounded task-worker pool rather than the event loop. Keep concurrency at or
below the measured native worker count and recycle workers using normal Laravel
memory/job limits.

Run the lifecycle soak against the stripped library and real pinned models:

```bash
UNDERPRINT_LIBRARY_PATH=target/minimal-release/libunderprint.dylib \
UNDERPRINT_SOAK_ITERATIONS=500 \
php bindings/php/tests/soak.php
```

Use `.so` on Linux. The soak reuses one context, copies every result before its
native handle is freed, periodically collects PHP cycles, and fails on missing
detections or more than 16 MiB of PHP-managed growth. Track native RSS
separately at the process supervisor because ONNX session memory is outside
PHP's allocator.
