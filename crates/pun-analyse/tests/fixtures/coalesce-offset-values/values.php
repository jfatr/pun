<?php

declare(strict_types=1);

namespace CoalesceOffsetValues;

use function PHPStan\dumpType;

final class Box
{
    public function __construct(public string $id) {}
}

final class Boxes
{
    /**
     * @param  list<Box>  $boxes
     * @return array<string, Box>
     */
    public function firstByName(array $boxes): array
    {
        $first = [];

        foreach ($boxes as $box) {
            $first[strtolower($box->id)] ??= $box;
        }

        dumpType($first);

        return $first;
    }

    /**
     * @param  list<Box>  $boxes
     * @return list<string>
     */
    public function owners(array $boxes): array
    {
        $owners = [];
        $result = [];

        foreach ($boxes as $box) {
            $result[] = $owners[spl_object_id($box)] ??= $box->id;
        }

        dumpType($owners);
        dumpType($result);

        return $result;
    }

    public function once(Box $box): void
    {
        $seen = [];
        dumpType($seen[$box->id] ??= $box);
        dumpType($seen);
    }
}
