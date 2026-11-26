<?php

declare(strict_types=1);

namespace TraitPropertyTemplates;

/** @template T */
final class Box
{
    /** @param list<T> $items */
    public function __construct(public array $items) {}
}

final class Apple {}

final class Pear {}

/**
 * @template TRow of object
 */
trait HoldsRows
{
    /** @var Box<TRow>|null */
    private ?Box $rows = null;

    /** @param Box<TRow> $rows */
    public function hold(Box $rows): void
    {
        $this->rows = $rows;
    }
}

final class Basket
{
    /** @use HoldsRows<Apple> */
    use HoldsRows;

    /** @param Box<Apple> $apples */
    public function apples(Box $apples): void
    {
        $this->rows = $apples;
    }

    /** @param Box<Pear> $pears */
    public function pears(Box $pears): void
    {
        $this->rows = $pears;
    }
}
