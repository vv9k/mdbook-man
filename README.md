# mdbook-man

[![Build Status](https://github.com/vv9k/mdbook-man/workflows/CI/badge.svg)](https://github.com/vv9k/mdbook-man/actions?query=workflow%3A%22CI%22)

Generate man pages from mdBooks!


## Usage

To use **mdbook-man** you'll first need to install it with:
```console
$ cargo install mdbook-man
```

And add the following to your `book.toml`:
```toml
[output.man]
```

The man page will be printed to standard output when running `mdbook build`.


## Configuration

This are the available configuration parameters:

```toml
[output.man]
output-dir = "/some/path/to/a/directory"  # this directory doesn't have to exist, it will be created automatically

split-chapters = true # By default all chapters will be joined as one single man page, 
                      # to override this set this parameter to true.
```

## Example

You can check out [the Rust Programming Language book](https://doc.rust-lang.org/book/) compiled as a man page [here](https://vv9k.github.io/mdbook-man/book.man).

```console
$ wget https://vv9k.github.io/mdbook-man/book.man
$ man -l book.man
```

## License
[MIT](https://github.com/vv9k/mdbook-man/blob/master/LICENSE)
