<?php

declare(strict_types=1);

namespace Underprint;

final readonly class DetectionResult
{
    /** @param array<string, mixed> $document */
    public function __construct(
        public bool $present,
        public array $document,
    ) {}
}
