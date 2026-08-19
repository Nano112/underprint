<?php

declare(strict_types=1);

namespace Underprint;

final readonly class EmbeddingResult
{
    /** @param array<string, mixed> $document */
    public function __construct(
        public string $image,
        public array $document,
    ) {}
}
