<?php

namespace PunReturnedMacros;

use Illuminate\Routing\Route as RoutingRoute;
use Illuminate\Support\Facades\Route;

use function PHPStan\Testing\assertType;

function routes(): RoutingRoute
{
    assertType('Illuminate\Routing\Route', Route::get('/c', static fn (): string => 'c')->punWithHead(title: 'C'));
    assertType('Illuminate\Routing\Route', Route::get('/d', static fn (): string => 'd')->punWithTitle('D'));

    return Route::get('/c', static fn (): string => 'c')->punWithHead(title: 'C')->name('c');
}
