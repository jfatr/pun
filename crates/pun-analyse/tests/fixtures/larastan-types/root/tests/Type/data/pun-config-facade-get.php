<?php

namespace PunConfigFacadeGet;

use Illuminate\Config\Repository;
use Illuminate\Support\Facades\Config;
use function PHPStan\Testing\assertType;

function test(Repository $config): void
{
    assertType('mixed', Config::get('test.foo'));
    assertType('mixed', Config::get('test.bar'));
    assertType('mixed', Config::get('test.foo', 5));
    assertType("'bar'", $config->get('test.foo'));
    assertType("'bar'", config('test.foo'));
    assertType("'bar'", config()->get('test.foo'));
}

function guards(): ?string
{
    $foo = Config::get('test.foo');

    if (! is_string($foo) || $foo === '') {
        return null;
    }

    return $foo;
}

function literal(): bool
{
    return is_string(config('test.foo'));
}
