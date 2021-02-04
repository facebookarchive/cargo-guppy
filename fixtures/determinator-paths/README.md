# determinator paths fixtures

This fixture is used to test that path matching works correctly across platforms.

* `git-diff.out`: The output of `git diff -z --name-only f9ddae14671073f9fe847f8c6190de596f87a119^ f9ddae14671073f9fe847f8c6190de596f87a119`, identical on Windows and Linux.
* `guppy-win.json`: `cargo metadata` output on Windows.
* `guppy-linux.json`: `cargo metadata` output on Linux.
