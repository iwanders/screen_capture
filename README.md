# screen_capture

This is a fork of my [displaylight_rs](https://github.com/iwanders/displaylight_rs) project that contains just the `screen_capture` crate.
That project was one of my earliest Rust projects and the `screen_capture` crate was reused in many projects that required
capturing output from computer games and the like.
In this fork I'm cleaning it up a bit and adding compatibility with the [image](https://github.com/image-rs/image) crate.

- Screen capture takes a snapshot of the screen and keeps it in shared memory.
  - Uses X11's shared memory extension [Xshm](https://en.wikipedia.org/wiki/MIT-SHM) on Linux.
  - Uses the [Desktop Duplication API](https://docs.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api) on Windows (with help of [windows-rs][windows-rs]).

Both on Windows and Linux this makes it almost a zero overhead capture system, assuming the BGR image buffer can be used.
The capturer returns a `Box<dyn ImageBGR>`, which points directly at the framebuffer.
Both Linux and Windows use `BGR` as color order, with a padding byte between the individual pixels, which makes one pixel exactly 4 bytes.
This data type does implement `GenericImageView<Pixel=image::Rgba>`, but accessing through that does require the channels to be swapped
individually. Copying the `Box<dyn ImageBGR>` makes a copy of the framebuffer, but does not do color space conversion
(it copies to `RasterImageBGR` under the hood).

To convert it to a normal `image::RgbaImage`, the `to_rgba()` method can be called on the `dyn ImageBGR` object.
This performs a color space conversion as well as creating an owned copy of the image.
There is some [hand written simd](./src/simd.rs) to do this conversion in a fast way.
It loads 8 BGRA pixels into one SIMD vector (256), then performs a single shuffle operation with a fixed mask,
then an OR operation to ensure alpha channel is fully opaque, after which the RGBA pixels are stored back to memory.
This fast routine does require compiling this crate with avx2, so if you do need the color conversion be sure to enable that.
If avx2 is not available, it falls back to a simple implementation.


## Development
Building the Windows binaries from Linux:
```
rustup target add x86_64-pc-windows-gnu
cargo build --target x86_64-pc-windows-gnu --example main
```

## License
License is `MIT OR Apache-2.0`.

[rust]: https://www.rust-lang.org/
[windows-rs]: https://github.com/microsoft/windows-rs
