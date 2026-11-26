<?php

namespace PunContainerBindings;

use function PHPStan\Testing\assertType;

final class Calls
{
    public function insideTransaction(): bool
    {
        assertType('Illuminate\Database\DatabaseTransactionsManager', resolve('db.transactions'));
        assertType('Illuminate\Database\DatabaseTransactionsManager', app('db.transactions'));
        assertType('Illuminate\Log\LogManager', resolve('log'));
        assertType('Illuminate\Pipeline\Pipeline', app('pipeline'));
        assertType('Illuminate\Database\Migrations\Migrator', app('migrator'));
        assertType('Illuminate\Cache\MemcachedConnector', app('memcached.connector'));

        return resolve('db.transactions')->callbackApplicableTransactions()->isNotEmpty();
    }
}
