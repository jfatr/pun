<?php

declare(strict_types=1);

namespace App;

use function PHPStan\dumpType;

final class Memory
{
    public function limit(): int
    {
        return ini_parse_quantity(ini_get('memory_limit'));
    }

    /** @param 'memory_limit'|'precision' $either */
    public function types(string $name, string $either, bool $flag): void
    {
        dumpType(ini_get('date.timezone'));
        dumpType(ini_get('max_execution_time'));
        dumpType(ini_get('max_input_time'));
        dumpType(ini_get('default_socket_timeout'));
        dumpType(ini_get('precision'));
        dumpType(ini_get('max_memory_limit'));
        dumpType(ini_get($either));
        dumpType(ini_get($flag ? 'memory_limit' : 'display_errors'));
        dumpType(ini_get('display_errors'));
        dumpType(ini_get($name));
    }
}
