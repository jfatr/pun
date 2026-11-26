<?php

declare(strict_types=1);

namespace NarrowedMixedGenerics;

/** @template T */
final class Box
{
    /** @param T $value */
    public function __construct(public mixed $value) {}
}

final class Boxes
{
    /** @return Box<int> */
    public function narrowed(mixed $value): Box
    {
        if ($value) {
            return new Box($value);
        }

        return new Box(0);
    }

    /** @return Box<int> */
    public function notNull(mixed $value): Box
    {
        if ($value !== null) {
            return new Box($value);
        }

        return new Box(0);
    }

    /** @return Box<int> */
    public function plain(mixed $value): Box
    {
        return new Box($value);
    }

    /** @return Box<int> */
    public function text(string $value): Box
    {
        return new Box($value);
    }
}
