<?php

namespace PunVendorRouteMacros;

use Closure;
use Illuminate\Routing\Route;

class RouteMacros
{
    public function register(): void
    {
        Route::macro('punWithHead', $this->routeMacro());
        Route::macro('punWithTitle', $this->titleMacro());
    }

    protected function routeMacro(): Closure
    {
        $macro = function (?string $title = null): Route {
            /** @var Route $this */
            return $this->defaults('title', $title);
        };

        return $macro;
    }

    protected function titleMacro(): Closure
    {
        return function (string $title): Route {
            /** @var Route $this */
            return $this->defaults('title', $title);
        };
    }
}
