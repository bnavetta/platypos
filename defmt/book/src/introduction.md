<p align="center"><img src="assets/knurling_logo_light_text.svg"></p>

# `defmt`

> `defmt` ("de format", short for "deferred formatting") is a highly efficient logging framework that targets resource-constrained devices, like microcontrollers.

## Features

- `println!`-like formatting
- Multiple logging levels: `error`, `info`, `warn`, `debug`, `trace`
- Compile-time `RUST_LOG`-like filtering of logs: include/omit logging levels with module-level granularity
- Timestamped logs

## Current limitations

- Output object format must be ELF
- Custom linking (linker script) is required
- Single, global logger instance (but using multiple channels is possible)

## Intended use

In its current iteration `defmt` mainly targets tiny embedded devices that have no mean to display information to the developer, e.g. a screen.
In this scenario logs need to be transferred to a second machine, usually a PC/laptop, before they can be displayed to the developer/end-user.

`defmt` operating principles, however, are applicable to beefier machines and could be use to improve the logging performance of x86 web server applications and the like.

## Operating principle

`defmt` achieves high performance using deferred formatting and string compression.

Deferred formatting means that formatting is not done on the machine that's logging data but on a second machine.
That is, instead of formatting `255u8` into `"255"` and sending the string, the single-byte binary data is sent to a second machine, the *host*, and the formatting happens there.

`defmt`'s string compression consists of building a table of string literals, like `"Hello, world"` or `"The answer is {:?}"`, at compile time.
At runtime the logging machine sends *indices* instead of complete strings.

## Support

`defmt` is part of the [Knurling] project, [Ferrous Systems]' effort at
improving tooling used to develop for embedded systems.

If you think that our work is useful, consider sponsoring it via [GitHub
Sponsors].

<iframe src="https://github.com/sponsors/knurling-rs/card" height=250em width=100%; title="Sponsor knurling-rs" style="border: 0; display:block; margin:auto" id="iframe"></iframe>

[Knurling]: https://knurling.ferrous-systems.com/
[Ferrous Systems]: https://ferrous-systems.com/
[GitHub Sponsors]: https://github.com/sponsors/knurling-rs


<!-- git commit & date are injected in this block -->
<div style="font-size: 0.75em;">
  <center>
    <code>
      {{ #include ../version.md }}
    </code>
  </center>
</div>
