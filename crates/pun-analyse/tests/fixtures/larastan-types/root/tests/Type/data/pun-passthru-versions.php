<?php

namespace PunPassthruVersions;

use Illuminate\Database\Eloquent\Model;

use function PHPStan\Testing\assertType;

final class Order extends Model
{
}

final class Calls
{
    public function open(string $number): bool
    {
        assertType('Illuminate\Support\Collection', Order::query()->insertOrIgnoreReturning([['number' => $number]], ['id']));
        assertType('int<0, max>', Order::query()->insertOrIgnore([['number' => $number]]));

        return Order::query()->insertOrIgnoreReturning([['number' => $number]], ['id'])->all() !== [];
    }
}
