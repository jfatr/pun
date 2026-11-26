<?php

final class LocalArray
{
    public function copied(array $raw, string $file): bool
    {
        $paths = $raw;

        foreach ($paths as $path) {
            if ($file === $path) {
                return true;
            }
        }

        return false;
    }

    public function mapped(array $raw, string $file): bool
    {
        $paths = array_map(fn (string $path): string => rtrim($path, '/'), $raw);

        foreach ($paths as $path) {
            if ($path == $file) {
                return false;
            }
        }

        return true;
    }

    public function untyped($raw, string $file): bool
    {
        $paths = $raw;

        foreach ($paths as $path) {
            if ($file === $path) {
                return true;
            }
        }

        return false;
    }
}
-----
<?php

final class LocalArray
{
    public function copied(array $raw, string $file): bool
    {
        $paths = $raw;
        return in_array($file, $paths, true);
    }

    public function mapped(array $raw, string $file): bool
    {
        $paths = array_map(fn (string $path): string => rtrim($path, '/'), $raw);
        return !in_array($file, $paths);
    }

    public function untyped($raw, string $file): bool
    {
        $paths = $raw;

        foreach ($paths as $path) {
            if ($file === $path) {
                return true;
            }
        }

        return false;
    }
}
