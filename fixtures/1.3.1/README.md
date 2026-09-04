# Fixture provenance

These fixtures cover the output boundary of Apple's signed `container` 1.3.1 CLI. On September 4, 2026, the CLI was extracted from Apple's signed installer and run on macOS 26.6.2 against an installed 1.2.0 system service. Every read command Bushel uses completed successfully. Its JSON shapes and error text matched the existing sanitized 1.2.0 fixtures, so this directory records those verified shapes with the captured 1.3.1 version output. Some optional fields are omitted to exercise Bushel's tolerant parser.

Because this check used a 1.2.0 service, replace these files with sanitized output from a complete 1.3.1 installation during the physical macOS 27 release gate.
