<?php

namespace PunVendorRouteMacros;

use Illuminate\Support\ServiceProvider;

class RouteMacrosServiceProvider extends ServiceProvider
{
    public function boot(): void
    {
        (new RouteMacros)->register();
    }
}
