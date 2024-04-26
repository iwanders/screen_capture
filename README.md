# screen_capture

This is a fork of my [displaylight_rs](https://github.com/iwanders/displaylight_rs) project that contains just the `screen_capture` crate.
That project was one of my earliest Rust projects and the `screen_capture` crate was reused in many projects that required capturing output from computer games and the like. In this fork I'm cleaning it up a bit and adding compatibility with the [image](https://github.com/image-rs/image) crate.

- Screen capture takes a snapshot of the screen and keeps it in shared memory.
  - Uses X11's shared memory extension [Xshm](https://en.wikipedia.org/wiki/MIT-SHM) on Linux.
  - Uses the [Desktop Duplication API](https://docs.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api) on Windows (with help of [windows-rs][windows-rs]).

Both on Windows and Linux this makes it almost a zero overhead capture system. Especially if the image doesn't need to be copied.

## License
License is `MIT OR Apache-2.0`.

[rust]: https://www.rust-lang.org/
[windows-rs]: https://github.com/microsoft/windows-rs
