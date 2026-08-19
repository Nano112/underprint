<?php

declare(strict_types=1);

require dirname(__DIR__).'/vendor/autoload.php';

use Underprint\Native;

$root = dirname(__DIR__, 3);
$iterations = max(1, (int) (getenv('UNDERPRINT_SOAK_ITERATIONS') ?: 500));
$image = file_get_contents($root.'/tests/golden/trustmark-q-bch5-v1/protected.png');
if ($image === false) {
    throw new RuntimeException('Golden image is unavailable');
}

$before = memory_get_usage(true);
$native = Native::load($root.'/models');
for ($index = 0; $index < $iterations; $index++) {
    $result = $native->detect($image);
    if (! $result->present) {
        throw new RuntimeException("Detection failed at iteration {$index}");
    }
    if ($index % 50 === 0) {
        gc_collect_cycles();
    }
}
$after = memory_get_usage(true);
$growth = $after - $before;
$native->close();

if ($growth > 16 * 1024 * 1024) {
    throw new RuntimeException("PHP managed memory grew by {$growth} bytes");
}

printf("PHP FFI soak passed: %d detections, managed growth %+d bytes\n", $iterations, $growth);
