# SPIF-Parser

This tool parses an export from Saleae Logic software and tries to interpret it as a SPIF communication.

## How to use :

`bat trace_sample_on_change.bin | cargo run --color=always 2>&1 | rg -v '(StatusRegister|WriteEnable)' | less`
