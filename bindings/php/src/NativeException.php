<?php

declare(strict_types=1);

namespace Underprint;

use RuntimeException;

final class NativeException extends RuntimeException
{
    public function __construct(
        public readonly int $status,
        string $message,
    ) {
        parent::__construct($message, $status);
    }
}
