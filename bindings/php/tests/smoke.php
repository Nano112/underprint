<?php

declare(strict_types=1);

require dirname(__DIR__) . '/vendor/autoload.php';

use Underprint\Native;

$models = getenv('UNDERPRINT_MODELS_DIR') ?: dirname(__DIR__, 3) . '/models';
$native = Native::load($models);

assert($native->version() !== '');
assert(($native->capabilities()['ready'] ?? false) === true);

echo "Underprint PHP FFI smoke test passed\n";
