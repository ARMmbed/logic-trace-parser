# SPIF-Parser

This tool parses an export from Saleae Logic software and tries to interpret it as a SPIF communication.
```
spif-parser 0.1.0
Wilfried Chauveau <wilfried.chauveau@arm.com>


USAGE:
    spif-parser [FLAGS] [OPTIONS] [file]

FLAGS:
    -h, --help       Prints help information
    -v               Sets the level of verbosity
    -V, --version    Prints version information

OPTIONS:
        --clk <clk>                            Channel used for the clock [default: 3]
        --cs <cs>                              Channel used for the chip select. [default: 0]
    -l, --cs_active_level <cs_active_level>    Chip select active level [default: Low]  [possible values: High, Low]
    -f, --freq <freq>                          Sample frequency [default: 1.]
        --miso <miso>                          Channel used for miso [default: 1]
    -m, --mode <mode>                          Spi mode [default: 0]  [possible values: 0, 1, 2, 3]
        --mosi <mosi>                          Channel used for mosi [default: 2]

ARGS:
    <file>    Input file. If not provided, stdin will be used.
```
## How to use :

`spif-parser trace_sample_on_change.bin | rg -v '(StatusRegister|WriteEnable)' | less`
