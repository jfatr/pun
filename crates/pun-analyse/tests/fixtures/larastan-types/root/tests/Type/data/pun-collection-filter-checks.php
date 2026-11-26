<?php

namespace PunCollectionFilterChecks;

use function PHPStan\Testing\assertType;

final class Calls
{
    /**
     * @param array<string, mixed> $values
     * @param array<int, int|string|null> $ids
     */
    public function filters(array $values, array $ids): void
    {
        assertType('Illuminate\Support\Collection<string, bool|float|int|string>', collect($values)->filter(static fn (mixed $value): bool => is_scalar($value)));
        assertType('Illuminate\Support\Collection<string, array<mixed, mixed>>', collect($values)->filter(static fn (mixed $value): bool => is_array($value)));
        assertType('array<mixed, mixed>|null', collect($values)->filter(static fn (mixed $value): bool => is_array($value))->last());
        assertType('Illuminate\Support\Collection<int, int|string>', collect($ids)->filter(static fn (int|string|null $id): bool => is_scalar($id)));
        assertType('Illuminate\Support\Collection<int, null>', collect($ids)->reject(static fn (int|string|null $id): bool => is_scalar($id)));
    }
}
