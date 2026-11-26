<?php

declare(strict_types=1);

namespace App;

interface Shape
{
    public function area(): int;
}

interface Other
{
}

abstract class Box
{
    abstract public function size(): int;
}

final class Factory
{
    public function shape(): Shape
    {
        return new class implements Shape
        {
            public function area(): int
            {
                return 1;
            }
        };
    }

    public function box(): Box
    {
        return new class extends Box
        {
            public function size(): int
            {
                return 1;
            }
        };
    }

    public function maker(): \Closure
    {
        return static fn (): Shape => new class implements Shape
        {
            public function area(): int
            {
                return 2;
            }
        };
    }

    public function assigned(): Shape
    {
        $shape = new class implements Shape
        {
            public function area(): int
            {
                return 3;
            }
        };

        return $shape;
    }

    public function wrong(): Other
    {
        return new class implements Shape
        {
            public function area(): int
            {
                return 4;
            }
        };
    }

    public function plain(): Shape
    {
        return new class {};
    }
}
