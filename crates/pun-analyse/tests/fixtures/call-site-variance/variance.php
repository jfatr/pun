<?php

declare(strict_types=1);

namespace CallSiteVariance;

/** @template T */
final class Items
{
    /** @param list<T> $items */
    public function __construct(private array $items = []) {}

    /**
     * @template TMap
     *
     * @param  callable(T): TMap  $callback
     * @return Items<TMap>
     */
    public function map(callable $callback): Items
    {
        return new Items(array_map($callback, $this->items));
    }
}

class Animal {}

final class Dog extends Animal {}

final class Shelter
{
    /** @param Items<int> $ids */
    public function run(Items $ids): void
    {
        $groups = $ids->map(fn (int $id): Items => $this->load());
        \PHPStan\dumpType($groups);
        \PHPStan\dumpType($this->load());
        $this->accept($groups);
        $this->acceptInvariant($groups);
    }

    /** @return Items<covariant Animal> */
    private function load(): Items
    {
        return new Items([new Dog]);
    }

    /** @param Items<Items<covariant Animal>> $groups */
    private function accept(Items $groups): void {}

    /** @param Items<Items<Animal>> $groups */
    private function acceptInvariant(Items $groups): void {}
}
