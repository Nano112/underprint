<?php

declare(strict_types=1);

namespace Underprint;

use FFI;
use FFI\CData;
use JsonException;

final class Native
{
    private const ABI_VERSION = 1;

    private const OK = 0;
    private const NOT_DETECTED = 1;

    private static ?self $instance = null;

    private CData $context;

    private function __construct(
        private readonly FFI $ffi,
        string $modelsDirectory,
        array $runtimeConfiguration,
    ) {
        if ($this->ffi->up_abi_version() !== self::ABI_VERSION) {
            throw new NativeException(2, 'Underprint ABI version mismatch');
        }

        $configuration = json_encode(
            [...$runtimeConfiguration, 'models_dir' => $modelsDirectory],
            JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES,
        );
        [$view, $buffer] = $this->view($configuration);
        $context = $this->ffi->new('up_context *');
        $status = $this->ffi->up_context_create($view, FFI::addr($context));
        unset($buffer);
        if ($status !== self::OK) {
            throw new NativeException($status, 'Underprint context initialization failed');
        }
        $this->context = $context;

        $capabilities = $this->capabilities();
        if (($capabilities['ready'] ?? false) !== true) {
            $reason = (string) ($capabilities['unavailable_reason'] ?? 'native profile unavailable');
            $this->ffi->up_context_free($this->context);
            throw new NativeException(4, $reason);
        }
    }

    /**
     * @param array{
     *   intra_threads?: int,
     *   cpu_arena?: bool,
     *   memory_pattern?: bool,
     *   prepacking?: bool
     * } $runtimeConfiguration
     */
    public static function load(
        string $modelsDirectory,
        ?string $libraryPath = null,
        array $runtimeConfiguration = [],
    ): self {
        if (self::$instance !== null) {
            return self::$instance;
        }
        if (!self::isAvailable()) {
            throw new NativeException(4, 'PHP FFI is not enabled');
        }

        $libraryPath ??= getenv('UNDERPRINT_LIBRARY_PATH') ?: self::defaultLibraryPath();
        if (!is_file($libraryPath) || !is_readable($libraryPath)) {
            throw new NativeException(4, 'Underprint native library is unavailable');
        }
        $declarations = file_get_contents(dirname(__DIR__) . '/underprint.ffi.h');
        if ($declarations === false) {
            throw new NativeException(10, 'Underprint FFI declarations are unavailable');
        }

        return self::$instance = new self(
            FFI::cdef($declarations, $libraryPath),
            $modelsDirectory,
            $runtimeConfiguration,
        );
    }

    public static function isAvailable(): bool
    {
        if (!extension_loaded('ffi')) {
            return false;
        }
        $setting = strtolower((string) ini_get('ffi.enable'));

        return !in_array($setting, ['', '0', 'false', 'off'], true);
    }

    public static function isLoaded(): bool
    {
        return self::$instance !== null;
    }

    public function version(): string
    {
        return $this->copy($this->ffi->up_version());
    }

    /** @return array<string, mixed> */
    public function capabilities(): array
    {
        [$status, $document] = $this->callResult(
            fn (CData $out): int => $this->ffi->up_context_capabilities($this->context, $out),
        );
        $this->assertStatus($status, $document);

        return $document;
    }

    public function detect(string $image, string $profile = 'trustmark-q-bch5@1'): DetectionResult
    {
        [$imageView, $imageBuffer] = $this->view($image);
        [$optionsView, $optionsBuffer] = $this->view(json_encode(
            ['profile' => $profile],
            JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES,
        ));
        [$status, $document] = $this->callResult(
            fn (CData $out): int => $this->ffi->up_detect(
                $this->context,
                $imageView,
                $optionsView,
                $out,
            ),
        );
        unset($imageBuffer, $optionsBuffer);
        if (!in_array($status, [self::OK, self::NOT_DETECTED], true)) {
            $this->assertStatus($status, $document);
        }

        return new DetectionResult($status === self::OK, $document);
    }

    public function embed(
        string $image,
        string $payload,
        string $profile = 'trustmark-q-bch5@1',
    ): EmbeddingResult {
        [$imageView, $imageBuffer] = $this->view($image);
        [$optionsView, $optionsBuffer] = $this->view(json_encode(
            ['profile' => $profile, 'payload' => $payload],
            JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES,
        ));
        $result = $this->ffi->new('up_result *');
        $status = $this->ffi->up_embed(
            $this->context,
            $imageView,
            $optionsView,
            FFI::addr($result),
        );
        unset($imageBuffer, $optionsBuffer);

        try {
            $document = $this->decodeDocument($result);
            $this->assertStatus($status, $document);
            $output = $this->copy($this->ffi->up_result_output($result));
        } finally {
            if (!FFI::isNull($result)) {
                $this->ffi->up_result_free($result);
            }
        }

        return new EmbeddingResult($output, $document);
    }

    public function close(): void
    {
        if (isset($this->context) && !FFI::isNull($this->context)) {
            $this->ffi->up_context_free($this->context);
            unset($this->context);
        }
        self::$instance = null;
    }

    public function __destruct()
    {
        $this->close();
    }

    /**
     * @param callable(CData): int $operation
     * @return array{int, array<string, mixed>}
     */
    private function callResult(callable $operation): array
    {
        $result = $this->ffi->new('up_result *');
        $status = $operation(FFI::addr($result));
        try {
            return [$status, $this->decodeDocument($result)];
        } finally {
            if (!FFI::isNull($result)) {
                $this->ffi->up_result_free($result);
            }
        }
    }

    /** @return array<string, mixed> */
    private function decodeDocument(CData $result): array
    {
        if (FFI::isNull($result)) {
            return [];
        }
        $json = $this->copy($this->ffi->up_result_json($result));
        if ($json === '') {
            return [];
        }
        try {
            $document = json_decode($json, true, 512, JSON_THROW_ON_ERROR);
        } catch (JsonException) {
            throw new NativeException(10, 'Underprint returned malformed result JSON');
        }

        return is_array($document) ? $document : [];
    }

    /** @param array<string, mixed> $document */
    private function assertStatus(int $status, array $document): void
    {
        if ($status === self::OK) {
            return;
        }
        $message = (string) ($document['message'] ?? 'Underprint operation failed');
        throw new NativeException($status, $message);
    }

    /** @return array{CData, CData|null} */
    private function view(string $bytes): array
    {
        $view = $this->ffi->new('up_bytes_view');
        $length = strlen($bytes);
        $view->len = $length;
        if ($length === 0) {
            $view->data = null;

            return [$view, null];
        }

        $buffer = $this->ffi->new("uint8_t[$length]");
        FFI::memcpy($buffer, $bytes, $length);
        $view->data = FFI::addr($buffer[0]);

        return [$view, $buffer];
    }

    private function copy(CData $view): string
    {
        if ($view->len === 0 || FFI::isNull($view->data)) {
            return '';
        }

        return FFI::string($view->data, $view->len);
    }

    private static function defaultLibraryPath(): string
    {
        $extension = match (PHP_OS_FAMILY) {
            'Darwin' => 'dylib',
            'Windows' => 'dll',
            default => 'so',
        };
        $prefix = PHP_OS_FAMILY === 'Windows' ? '' : 'lib';

        return dirname(__DIR__, 3) . "/target/minimal-release/{$prefix}underprint.{$extension}";
    }
}
