<?php

declare(strict_types=1);

namespace NullsafeNullableChains;

final class Box
{
    public int $n = 0;

    public function count(): int
    {
        return 1;
    }
}

final class Holder
{
    public ?Box $maybe = null;

    public Box $box;

    public function __construct()
    {
        $this->box = new Box();
    }
}

final class Reader
{
    public function chain(?Holder $h): void
    {
        \PHPStan\dumpType($h?->maybe->n);
        \PHPStan\dumpType($h?->maybe->count());
        \PHPStan\dumpType($h?->box->n);
        \PHPStan\dumpType($h?->box->count());
    }

    public function direct(Holder $h): void
    {
        \PHPStan\dumpType($h->maybe->n);
        \PHPStan\dumpType($h->maybe->count());
    }

    public function nullsafeOnBoth(?Holder $h): void
    {
        \PHPStan\dumpType($h?->maybe?->n);
        \PHPStan\dumpType($h?->maybe?->count());
    }
}
