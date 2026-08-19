<?php

declare(strict_types=1);

require dirname(__DIR__) . '/vendor/autoload.php';

use Underprint\Native;

$inputPath = getenv('UNDERPRINT_TEST_IMAGE');
if ($inputPath === false || !is_file($inputPath)) {
    fwrite(STDERR, "UNDERPRINT_TEST_IMAGE must name a readable PNG/JPEG/WebP fixture\n");
    exit(2);
}

$source = file_get_contents($inputPath);
if ($source === false) {
    throw new RuntimeException('Unable to read test image');
}

$models = getenv('UNDERPRINT_MODELS_DIR') ?: dirname(__DIR__, 3) . '/models';
$token = '1011011110011000111111000000011111011111011100000110110110111';
$native = Native::load($models);
$embedding = $native->embed($source, $token);
$detection = $native->detect($embedding->image);

assert($embedding->image !== '');
assert(($embedding->document['self_verified'] ?? false) === true);
assert($detection->present);
assert(($detection->document['detections'][0]['payload'] ?? null) === $token);

echo "Underprint PHP FFI binary round trip passed\n";
