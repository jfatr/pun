<?php

declare(strict_types=1);

namespace FloatShapeKeys;

final class Prices
{
    /** @return array{quantity: float} */
    public function row(int $n): array
    {
        return ['quantity' => $n];
    }

    /** @return list<array{quantity: float}> */
    public function rows(int $n): array
    {
        return [['quantity' => $n + 1]];
    }

    /** @return array{quantity: float, label?: string} */
    public function optional(int $n): array
    {
        return ['quantity' => $n];
    }

    /** @return array{quantity: float} */
    public function text(string $n): array
    {
        return ['quantity' => $n];
    }

    /** @return array{quantity: int} */
    public function narrower(float $n): array
    {
        return ['quantity' => $n];
    }

    /** @return array{quantity: float, label: string} */
    public function missing(int $n): array
    {
        return ['quantity' => $n];
    }
}
