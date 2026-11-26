<?php

final class InlineVar
{
    /** @param list<string> $raw */
    public function named(array $raw, string $file): bool
    {
        /** @var list<string> $paths */
        $paths = array_map(fn (string $path): string => rtrim($path, '/'), $raw);

        foreach ($paths as $path) {
            if ($file === $path || str_starts_with($file, $path . '/')) {
                return true;
            }
        }

        return false;
    }

    /** @param list<string> $raw */
    public function unnamed(array $raw, string $file): bool
    {
        /** @var list<string> */
        $paths = $raw;

        foreach ($paths as $path) {
            if ($file === $path || str_starts_with($file, $path)) {
                return true;
            }
        }

        return false;
    }

    /** @param list<string> $raw */
    public function otherVariable(array $raw, string $file): bool
    {
        /** @var list<string> $other */
        $paths = $raw;

        foreach ($paths as $path) {
            if ($file === $path || str_starts_with($file, $path)) {
                return true;
            }
        }

        return false;
    }

    /** @param list<string> $raw */
    public function untagged(array $raw, string $file): bool
    {
        $paths = array_map(fn (string $path): string => rtrim($path, '/'), $raw);

        foreach ($paths as $path) {
            if ($file === $path || str_starts_with($file, $path . '/')) {
                return true;
            }
        }

        return false;
    }
}
-----
<?php

final class InlineVar
{
    /** @param list<string> $raw */
    public function named(array $raw, string $file): bool
    {
        /** @var list<string> $paths */
        $paths = array_map(fn (string $path): string => rtrim($path, '/'), $raw);

        foreach ($paths as $path) {
            if ($file === $path || str_starts_with($file, $path . '/')) {
                return true;
            }
        }

        return false;
    }

    /** @param list<string> $raw */
    public function unnamed(array $raw, string $file): bool
    {
        /** @var list<string> */
        $paths = $raw;

        foreach ($paths as $path) {
            if ($file === $path || str_starts_with($file, $path)) {
                return true;
            }
        }

        return false;
    }

    /** @param list<string> $raw */
    public function otherVariable(array $raw, string $file): bool
    {
        /** @var list<string> $other */
        $paths = $raw;
        return array_any($paths, fn($path) => $file === $path || str_starts_with($file, $path));
    }

    /** @param list<string> $raw */
    public function untagged(array $raw, string $file): bool
    {
        $paths = array_map(fn (string $path): string => rtrim($path, '/'), $raw);
        return array_any($paths, fn($path) => $file === $path || str_starts_with($file, $path . '/'));
    }
}
