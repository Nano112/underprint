<?php

declare(strict_types=1);

use Underprint\Native;

require dirname(__DIR__) . '/bindings/php/vendor/autoload.php';

/** @return non-empty-string */
function requiredEnvironment(string $name): string
{
    $value = getenv($name);
    if (!is_string($value) || $value === '') {
        fwrite(STDERR, "{$name} is required\n");
        exit(2);
    }

    return $value;
}

/** @return array{mean_ms: float, median_ms: float, p95_ms: float, min_ms: float, max_ms: float} */
function statistics(array $samples): array
{
    sort($samples, SORT_NUMERIC);
    $count = count($samples);

    return [
        'mean_ms' => array_sum($samples) / $count,
        'median_ms' => $samples[intdiv($count, 2)],
        'p95_ms' => $samples[(int) ceil($count * 0.95) - 1],
        'min_ms' => $samples[0],
        'max_ms' => $samples[$count - 1],
    ];
}

function currentRssMib(): ?float
{
    $output = [];
    $status = 0;
    exec('ps -o rss= -p ' . getmypid(), $output, $status);
    if ($status !== 0 || !isset($output[0])) {
        return null;
    }

    return round(((int) trim($output[0])) / 1024, 2);
}

function positiveIntegerEnvironment(string $name, int $default): int
{
    $raw = getenv($name);
    $value = is_string($raw) && $raw !== '' ? filter_var($raw, FILTER_VALIDATE_INT) : $default;
    if (!is_int($value) || $value < 1) {
        fwrite(STDERR, "{$name} must be a positive integer\n");
        exit(2);
    }

    return $value;
}

$models = requiredEnvironment('UNDERPRINT_MODELS_DIR');
$library = requiredEnvironment('UNDERPRINT_LIBRARY_PATH');
$inputPath = requiredEnvironment('UNDERPRINT_BENCH_INPUT');
$protectedPath = requiredEnvironment('UNDERPRINT_BENCH_PROTECTED');
$token = getenv('UNDERPRINT_BENCH_TOKEN') ?: '1011011110011000111111000000011111011111011100000110110110111';
$warmups = positiveIntegerEnvironment('UNDERPRINT_BENCH_WARMUPS', 3);
$detectIterations = positiveIntegerEnvironment('UNDERPRINT_BENCH_DETECT_ITERATIONS', 30);
$embedIterations = positiveIntegerEnvironment('UNDERPRINT_BENCH_EMBED_ITERATIONS', 10);
$configurationJson = getenv('UNDERPRINT_BENCH_RUNTIME') ?: '{}';
$configuration = json_decode($configurationJson, true, 512, JSON_THROW_ON_ERROR);
if (!is_array($configuration)) {
    throw new RuntimeException('UNDERPRINT_BENCH_RUNTIME must be a JSON object');
}

$input = file_get_contents($inputPath);
$protected = file_get_contents($protectedPath);
if (!is_string($input) || !is_string($protected)) {
    throw new RuntimeException('benchmark inputs must be readable files');
}

$loadStart = hrtime(true);
$native = Native::load($models, $library, $configuration);
$loadMs = (hrtime(true) - $loadStart) / 1e6;
$capabilities = $native->capabilities();
$memory = ['after_load_mib' => currentRssMib()];

for ($index = 0; $index < $warmups; $index++) {
    $detection = $native->detect($protected);
    if (!$detection->present) {
        throw new RuntimeException('warmup image did not contain a detectable payload');
    }
}
$memory['after_detect_mib'] = currentRssMib();

$detectSamples = [];
for ($index = 0; $index < $detectIterations; $index++) {
    $start = hrtime(true);
    $detection = $native->detect($protected);
    $detectSamples[] = (hrtime(true) - $start) / 1e6;
    if (!$detection->present || ($detection->document['detections'][0]['payload'] ?? null) !== $token) {
        throw new RuntimeException('detection returned an unexpected payload');
    }
}

for ($index = 0; $index < $warmups; $index++) {
    $embedding = $native->embed($input, $token);
}
$memory['after_embed_mib'] = currentRssMib();

$embedSamples = [];
for ($index = 0; $index < $embedIterations; $index++) {
    $start = hrtime(true);
    $embedding = $native->embed($input, $token);
    $embedSamples[] = (hrtime(true) - $start) / 1e6;
    if (($embedding->document['payload'] ?? null) !== $token) {
        throw new RuntimeException('embedding returned an unexpected payload');
    }
}

$result = [
    'schema' => 'underprint.benchmark/v1',
    'underprint_version' => $native->version(),
    'machine' => php_uname(),
    'php_version' => PHP_VERSION,
    'runtime' => $capabilities['runtime'] ?? $configuration,
    'iterations' => [
        'warmups' => $warmups,
        'detect' => $detectIterations,
        'embed' => $embedIterations,
    ],
    'input' => [
        'bytes' => strlen($input),
        'sha256' => hash('sha256', $input),
    ],
    'protected' => [
        'bytes' => strlen($protected),
        'sha256' => hash('sha256', $protected),
    ],
    'load_ms' => $loadMs,
    'memory' => $memory,
    'detect' => statistics($detectSamples),
    'embed' => statistics($embedSamples),
    'last_embedding' => [
        'bytes' => strlen($embedding->image),
        'sha256' => hash('sha256', $embedding->image),
        'strength' => $embedding->document['selected_strength'] ?? null,
    ],
];

echo json_encode($result, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
